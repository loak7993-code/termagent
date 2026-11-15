export type Effort = "auto" | "low" | "medium" | "high" | "max";
export type Risk = "low" | "medium" | "high";

export interface ProviderInfo {
  id: string;
  name: string;
  protocol: string;
  env: string[];
  api: string;
  configured: boolean;
  keySet: boolean;
  local: boolean;
  envSet: boolean;
  models: number;
  reasoningModels: number;
}

export interface ModelInfo {
  id: string;
  name: string;
  reasoning: boolean;
  efforts: string[];
  tool_call: boolean;
  context: number;
  cost_in: number;
  cost_out: number;
}

export interface SessionMeta {
  id: string;
  title: string;
  created: number;
  cwd: string;
  provider: string;
  model: string;
  effort: Effort;
}

export type StoredEvent =
  | { kind: "meta"; id: string; title: string; created: number; cwd: string; provider: string; model: string; effort: Effort }
  | { kind: "title"; title: string }
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string; reasoning?: string; tool_calls?: { id: string; name: string; arguments: string }[] }
  | { kind: "tool"; id: string; name: string; args: unknown; output: string; ok: boolean; risk: Risk }
  | { kind: "usage"; input: number; output: number };

export interface SessionPayload extends SessionMeta {
  events: StoredEvent[] | null;
}

export interface Snapshot {
  version: string;
  configured: boolean;
  provider: string;
  model: string;
  modelName: string | null;
  reasoning: boolean;
  efforts: string[];
  effort: Effort;
  autoApprove: boolean;
  theme: string;
  providers: ProviderInfo[];
  sessions: SessionMeta[];
  session: SessionPayload | null;
  totals: { input: number; output: number };
  registry: { source: string; fetchedAt: number; providers: number; models: number };
  busy: boolean;
}

export interface Approval {
  id: string;
  tool: string;
  summary: string;
  args: unknown;
  risk: Risk;
}

export interface ToolItem {
  kind: "tool";
  id: string;
  name: string;
  summary: string;
  risk: Risk;
  running: boolean;
  ok: boolean;
  output: string;
  args?: unknown;
  started: number;
  elapsed?: number;
}

export type Item =
  | { kind: "user"; id: string; text: string }
  | { kind: "assistant"; id: string; text: string; streaming: boolean }
  | { kind: "reasoning"; id: string; text: string; live: boolean }
  | ToolItem
  | { kind: "error"; id: string; text: string }
  | { kind: "info"; id: string; text: string };

export const EFFORTS: Effort[] = ["auto", "low", "medium", "high", "max"];

declare global {
  interface Window {
    __TERMAGENT__?: { token?: string; port?: number };
  }
}

export const token = window.__TERMAGENT__?.token ?? "";
export const booted = Boolean(token);

export class ApiError extends Error {}

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set("x-termagent-token", token);
  if (init.body) headers.set("content-type", "application/json");
  const res = await fetch(path, { ...init, headers });
  const text = await res.text();
  let data: unknown = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = text;
  }
  if (!res.ok) {
    const message =
      data && typeof data === "object" && "error" in data
        ? String((data as { error: unknown }).error)
        : `request failed (${res.status})`;
    throw new ApiError(message);
  }
  return data as T;
}

export const POPULAR = [
  "anthropic",
  "openai",
  "google",
  "openrouter",
  "opencode",
  "deepseek",
  "groq",
  "mistral",
  "xai",
  "cerebras",
  "moonshotai",
  "zhipuai",
  "lmstudio",
];

export function orderProviders<
  T extends { id: string; name: string; keySet: boolean; envSet: boolean },
>(providers: T[], active?: string): T[] {
  const rank = (entry: T): number => {
    if (entry.id === active) return -1;
    if (entry.keySet) return 0;
    if (entry.envSet) return 1;
    const popular = POPULAR.indexOf(entry.id);
    return popular >= 0 ? 2 : 3;
  };
  return [...providers].sort(
    (a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name),
  );
}

export function uid(): string {
  return Math.random().toString(36).slice(2, 10);
}

export function compact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 10_000) return `${Math.round(n / 1000)}K`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}K`;
  return String(n);
}

export function price(model: ModelInfo | null | undefined): string {
  if (!model) return "cost unknown";
  if (model.cost_in === 0 && model.cost_out === 0) return "free";
  return `$${model.cost_in.toFixed(2)} / $${model.cost_out.toFixed(2)} per Mtok`;
}

export function costOf(usage: { input: number; output: number }, model: ModelInfo | null): number {
  if (!model) return 0;
  return (usage.input / 1_000_000) * model.cost_in + (usage.output / 1_000_000) * model.cost_out;
}

export function effortLabel(effort: Effort): string {
  return effort.charAt(0).toUpperCase() + effort.slice(1);
}

export function timeAgo(unix: number): string {
  const secs = Math.max(1, Math.floor(Date.now() / 1000 - unix));
  if (secs < 60) return "just now";
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86_400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86_400)}d ago`;
}

export function groupDay(unix: number): "Today" | "Yesterday" | "Earlier" {
  const start = new Date();
  start.setHours(0, 0, 0, 0);
  const day = start.getTime() / 1000;
  if (unix >= day) return "Today";
  if (unix >= day - 86_400) return "Yesterday";
  return "Earlier";
}

export function itemsFromEvents(events: StoredEvent[] | null | undefined): Item[] {
  if (!events) return [];
  const items: Item[] = [];
  for (const event of events) {
    if (event.kind === "user") {
      items.push({ kind: "user", id: uid(), text: event.text });
    } else if (event.kind === "assistant") {
      if (event.reasoning && event.reasoning.trim()) {
        items.push({ kind: "reasoning", id: uid(), text: event.reasoning, live: false });
      }
      if (event.text && event.text.trim()) {
        items.push({ kind: "assistant", id: uid(), text: event.text, streaming: false });
      }
    } else if (event.kind === "tool") {
      items.push({
        kind: "tool",
        id: event.id,
        name: event.name,
        summary: summarize(event.args, event.name),
        risk: event.risk,
        running: false,
        ok: event.ok,
        output: event.output,
        args: event.args,
        started: 0,
      });
    }
  }
  return items;
}

export function summarize(args: unknown, fallback: string): string {
  if (args && typeof args === "object") {
    const record = args as Record<string, unknown>;
    for (const key of ["command", "path", "pattern", "url", "content"]) {
      const value = record[key];
      if (typeof value === "string" && value.trim()) {
        return value.length > 160 ? `${value.slice(0, 160)}…` : value;
      }
    }
  }
  return fallback;
}
