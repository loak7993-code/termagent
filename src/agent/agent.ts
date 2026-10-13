// The tool-calling loop. Port of agent.py with 0.2.0 additions:
// per-turn usage/elapsed meta, reasoning forwarding, tool durations.

import { Client, APIError } from "./client.js";
import type { Config } from "./config.js";
import { schemas } from "./tools/index.js";
import { allTools, get, parseArgs } from "./tools/index.js";
import type { ToolContext } from "./tools/base.js";
import type { ChatMessage, ToolCall, TurnMeta } from "./types.js";

export interface AgentCallbacks {
  onText?: (chunk: string) => void;
  onReasoning?: (chunk: string) => void;
  onToolStart?: (id: string, name: string, args: Record<string, unknown>) => void;
  onToolResult?: (
    name: string,
    args: Record<string, unknown>,
    result: string,
    isError: boolean,
    durationMs: number,
  ) => void;
  onAssistantDone?: (text: string) => void;
  onError?: (message: string) => void;
  onMeta?: (meta: TurnMeta) => void;
}

export class Agent {
  cfg: Config;
  ctx: ToolContext;
  client: Client;
  messages: ChatMessage[] = [];
  private cb: AgentCallbacks;

  constructor(cfg: Config, ctx: ToolContext, callbacks: AgentCallbacks = {}) {
    this.cfg = cfg;
    this.ctx = ctx;
    this.cb = callbacks;
    this.client = new Client(cfg.apiKey, cfg.baseUrl, cfg.model, cfg.extraHeaders);
  }

  addSystem(): void {
    const toolNames = allTools().map((t) => t.name).join(", ");
    this.messages.push({
      role: "system",
      content:
        `${this.cfg.system}\n\nEnvironment:\n- working directory: ${this.ctx.cwd}` +
        `\n- platform: detect via tools\nYou have tools: ${toolNames}.`,
    });
  }

  async userTurn(text: string): Promise<void> {
    this.messages.push({ role: "user", content: text });
    await this.runLoop();
  }

  private async runLoop(): Promise<void> {
    const toolSchemas = schemas();
    for (let turn = 0; turn < this.cfg.maxTurns; turn++) {
      const contentParts: string[] = [];
      let toolCalls: ToolCall[] = [];
      const turnStart = Date.now();
      try {
        for await (const delta of this.client.streamChat(this.messages, toolSchemas)) {
          if (delta.reasoning) this.cb.onReasoning?.(delta.reasoning);
          if (delta.content) {
            contentParts.push(delta.content);
            this.cb.onText?.(delta.content);
          }
          if (delta.toolCalls) toolCalls = delta.toolCalls;
        }
      } catch (e) {
        if (e instanceof APIError) this.cb.onError?.(`API error: ${e.message}`);
        else this.cb.onError?.(`request failed: ${(e as Error).message}`);
        return;
      }

      const content = contentParts.join("");
      const assistant: ChatMessage = { role: "assistant", content: content || null };
      if (toolCalls.length > 0) assistant.tool_calls = toolCalls;
      this.messages.push(assistant);
      this.cb.onAssistantDone?.(content);

      const usage = this.client.lastUsage;
      this.cb.onMeta?.({
        promptTokens: usage?.prompt_tokens ?? 0,
        completionTokens: usage?.completion_tokens ?? 0,
        model: usage?.model ?? this.cfg.model,
        elapsedMs: Date.now() - turnStart,
      });

      if (toolCalls.length === 0) return;

      for (const call of toolCalls) {
        const name = call.function.name;
        const args = parseArgs(call.function.arguments);
        this.cb.onToolStart?.(call.id || name, name, args);
        const t0 = Date.now();
        const result = await this.exec(name, args);
        const durationMs = Date.now() - t0;
        this.cb.onToolResult?.(name, args, result.content, result.isError, durationMs);
        this.messages.push({
          role: "tool",
          tool_call_id: call.id || name,
          name,
          content: result.content,
        });
      }
    }
    this.cb.onError?.("max turns reached; stopping to avoid runaway tool use.");
  }

  private async exec(
    name: string,
    args: Record<string, unknown>,
  ): Promise<{ content: string; isError: boolean }> {
    const tool = get(name);
    if (!tool) return { content: `error: unknown tool '${name}'`, isError: true };
    try {
      return await tool.run(args, this.ctx);
    } catch (e) {
      return { content: `error in tool ${name}: ${(e as Error).message}`, isError: true };
    }
  }
}
