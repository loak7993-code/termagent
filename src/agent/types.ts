// Core message and result types shared by the agent, client, tools, and UI.

export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string | null;
  tool_calls?: ToolCall[];
  tool_call_id?: string;
  name?: string;
}

export interface ToolCall {
  id: string;
  type: "function";
  function: { name: string; arguments: string };
}

export interface ToolResult {
  content: string;
  isError: boolean;
}

export interface ToolCallView {
  id: string;
  name: string;
  args: Record<string, unknown>;
  status: "pending" | "running" | "success" | "error";
  result?: string;
  isError?: boolean;
  durationMs?: number;
}

export interface TurnMeta {
  promptTokens: number;
  completionTokens: number;
  model: string;
  elapsedMs: number;
}

export interface Usage {
  prompt_tokens: number;
  completion_tokens: number;
  model?: string;
}

export interface Delta {
  content?: string;
  reasoning?: string;
  toolCalls?: ToolCall[];
}
