// Tool registry + shared types. Port of tools/base.py.

import type { ChatMessage, ToolResult } from "../types.js";

export interface ToolContext {
  cwd: string;
  /** Return true to proceed. Sync or promise; the agent awaits it. */
  approve?: (kind: string, detail: string) => Promise<boolean> | boolean;
  emit?: (line: string) => void;
}

export function createToolContext(init: Partial<ToolContext> & { cwd: string }): ToolContext {
  return { approve: init.approve, emit: init.emit, cwd: init.cwd };
}

export interface ToolParams {
  type: "object";
  properties: Record<string, unknown>;
  required?: string[];
}

export interface Tool {
  name: string;
  description: string;
  params: ToolParams;
  read_only: boolean;
  run: (args: Record<string, unknown>, ctx: ToolContext) => Promise<ToolResult> | ToolResult;
}

export interface ToolSchema {
  type: "function";
  function: { name: string; description: string; parameters: ToolParams };
}

const REGISTRY = new Map<string, Tool>();

export function register(tool: Tool): void {
  REGISTRY.set(tool.name, tool);
}

export function get(name: string): Tool | undefined {
  return REGISTRY.get(name);
}

export function allTools(): Tool[] {
  return [...REGISTRY.values()];
}

export function schemas(): ToolSchema[] {
  return allTools().map((t) => ({
    type: "function",
    function: { name: t.name, description: t.description, parameters: t.params },
  }));
}

export function parseArgs(raw: unknown): Record<string, unknown> {
  if (raw === null || raw === undefined) return {};
  if (typeof raw === "object") return raw as Record<string, unknown>;
  if (typeof raw === "string") {
    const s = raw.trim();
    if (!s) return {};
    try {
      const parsed = JSON.parse(s) as unknown;
      return typeof parsed === "object" && parsed !== null
        ? (parsed as Record<string, unknown>)
        : { _raw: s };
    } catch {
      return { _raw: s };
    }
  }
  return {};
}

export const err = (message: string): ToolResult => ({ content: message, isError: true });
export const ok = (content: string): ToolResult => ({ content, isError: false });
export type { ChatMessage };
