// Observable app store bridging the agent loop to React (useSyncExternalStore).

import type { TurnMeta } from "../agent/types.js";

export type Risk = "low" | "medium" | "high";

export interface ToolCallItem {
  id: string;
  name: string;
  args: Record<string, unknown>;
  status: "pending" | "running" | "success" | "error";
  result?: string;
  isError?: boolean;
  durationMs?: number;
}

export type FeedItem =
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string; streaming: boolean }
  | { kind: "tool"; call: ToolCallItem }
  | { kind: "usage"; meta: TurnMeta }
  | { kind: "thinking"; seconds: number }
  | { kind: "notice"; text: string }
  | { kind: "error"; text: string }
  | { kind: "panel"; title: string; rows: Array<[string, string]> };

export interface ApprovalRequest {
  id: number;
  kind: string;
  detail: string;
  risk: Risk;
  resolve: (approved: boolean) => void;
}

export type OverlayKind = "models" | "themes" | "help" | null;

export interface ModelChoice {
  id: string;
  label: string;
  free: boolean;
}

export interface Snapshot {
  feed: FeedItem[];
  busy: boolean;
  approval: ApprovalRequest | null;
  overlay: OverlayKind;
  model: string;
  modelChoices: ModelChoice[];
  cwd: string;
  themeName: string;
  version: string;
  autoApprove: boolean;
  scrollOffset: number;
  status: string | null;
}

// Port of tui.py risk classification.
const DANGEROUS =
  /\b(rm\s+(-[a-z]*r[a-z]*f|-[a-z]*f[a-z]*r)|mkfs|dd\s+if=|shutdown|reboot|init\s+0|:\(\)\{.*\};:|chmod\s+-R\s+777\s+\/|git\s+push\s+--force|>\s*\/dev\/[sh]d|mkswap|format)\b/;
const REMOTE_SCRIPT = /\b(curl|wget)\b[^|;]*\|\s*(sudo\s+)?(ba)?sh\b/;

export function classifyRisk(kind: string, detail: string): Risk {
  if (kind === "bash") {
    if (DANGEROUS.test(detail) || REMOTE_SCRIPT.test(detail)) return "high";
    return "medium";
  }
  if (kind === "write" || kind === "edit") return "medium";
  return "low";
}

let approvalSeq = 0;

export class Store {
  private listeners = new Set<() => void>();
  private snap: Snapshot;

  feed: FeedItem[] = [];
  busy = false;
  approval: ApprovalRequest | null = null;
  overlay: OverlayKind = null;
  model: string;
  modelChoices: ModelChoice[] = [];
  cwd: string;
  themeName: string;
  version: string;
  autoApprove = false;
  scrollOffset = 0;
  status: string | null = null;

  constructor(init: { model: string; cwd: string; themeName: string; version: string }) {
    this.model = init.model;
    this.cwd = init.cwd;
    this.themeName = init.themeName;
    this.version = init.version;
    this.snap = this.build();
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  getSnapshot = (): Snapshot => this.snap;

  private build(): Snapshot {
    return {
      feed: this.feed,
      busy: this.busy,
      approval: this.approval,
      overlay: this.overlay,
      model: this.model,
      modelChoices: this.modelChoices,
      cwd: this.cwd,
      themeName: this.themeName,
      version: this.version,
      autoApprove: this.autoApprove,
      scrollOffset: this.scrollOffset,
      status: this.status,
    };
  }

  notify(): void {
    this.snap = this.build();
    for (const l of this.listeners) l();
  }

  // ---- feed ------------------------------------------------------------

  push(item: FeedItem): void {
    this.feed = [...this.feed, item];
    this.scrollOffset = 0;
    this.notify();
  }

  pushUser(text: string): void {
    this.push({ kind: "user", text });
  }

  startAssistant(): void {
    this.push({ kind: "assistant", text: "", streaming: true });
  }

  appendAssistant(chunk: string): void {
    const last = this.feed[this.feed.length - 1];
    if (last && last.kind === "assistant" && last.streaming) {
      const updated: FeedItem = { kind: "assistant", text: last.text + chunk, streaming: true };
      this.feed = [...this.feed.slice(0, -1), updated];
    } else {
      this.push({ kind: "assistant", text: chunk, streaming: true });
    }
    this.notify();
  }

  endAssistant(): void {
    const last = this.feed[this.feed.length - 1];
    if (last && last.kind === "assistant" && last.streaming) {
      const updated: FeedItem = { kind: "assistant", text: last.text, streaming: false };
      this.feed = [...this.feed.slice(0, -1), updated];
      this.notify();
    }
  }

  pushTool(call: ToolCallItem): void {
    this.push({ kind: "tool", call });
  }

  updateTool(id: string, patch: Partial<ToolCallItem>): void {
    const idx = this.feed.findIndex(
      (f) => f.kind === "tool" && f.call.id === id,
    );
    if (idx === -1) return;
    const item = this.feed[idx];
    if (!item || item.kind !== "tool") return;
    const updated: FeedItem = { kind: "tool", call: { ...item.call, ...patch } };
    this.feed = [...this.feed.slice(0, idx), updated, ...this.feed.slice(idx + 1)];
    this.notify();
  }

  // ---- approval ----------------------------------------------------------

  requestApproval(kind: string, detail: string): Promise<boolean> {
    if (this.autoApprove) return Promise.resolve(true);
    return new Promise<boolean>((resolve) => {
      approvalSeq += 1;
      this.approval = {
        id: approvalSeq,
        kind,
        detail,
        risk: classifyRisk(kind, detail),
        resolve: (approved) => {
          this.approval = null;
          this.notify();
          resolve(approved);
        },
      };
      this.notify();
    });
  }

  resolveApproval(approved: boolean, always = false): void {
    if (always) this.autoApprove = true;
    this.approval?.resolve(approved);
  }

  // ---- misc --------------------------------------------------------------

  setStatus(text: string | null): void {
    this.status = text;
    this.notify();
  }

  setBusy(busy: boolean): void {
    this.busy = busy;
    this.notify();
  }

  setOverlay(overlay: OverlayKind): void {
    this.overlay = overlay;
    this.notify();
  }

  setModel(model: string): void {
    this.model = model;
    this.notify();
  }

  setCwd(cwd: string): void {
    this.cwd = cwd;
    this.notify();
  }

  setThemeName(name: string): void {
    this.themeName = name;
    this.notify();
  }

  clearFeed(): void {
    this.feed = [];
    this.scrollOffset = 0;
    this.notify();
  }

  scroll(delta: number): void {
    const max = 500;
    const next = Math.min(max, Math.max(0, this.scrollOffset + delta));
    if (next !== this.scrollOffset) {
      this.scrollOffset = next;
      this.notify();
    }
  }
}
