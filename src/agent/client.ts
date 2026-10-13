// OpenAI-compatible streaming client over fetch + SSE. Port of client.py:
// chunked tool-call accumulation, reasoning deltas, usage capture from the
// final chunk, and graceful degradation when a provider rejects
// stream_options.

import type { ChatMessage, Delta, ToolCall, Usage } from "./types.js";

export class APIError extends Error {}

export interface StreamChatOptions {
  temperature?: number;
}

const trim = (s: string, n: number): string => (s.length <= n ? s : s.slice(0, n) + "…");

export class Client {
  apiKey: string;
  baseUrl: string;
  model: string;
  extraHeaders: Record<string, string>;
  timeoutMs: number;
  lastUsage: Usage | null = null;
  private skipStreamOptions = false;

  constructor(
    apiKey: string,
    baseUrl: string,
    model: string,
    extraHeaders: Record<string, string> = {},
    timeoutMs = 120_000,
  ) {
    this.apiKey = apiKey;
    this.baseUrl = baseUrl.replace(/\/+$/, "");
    this.model = model;
    this.extraHeaders = extraHeaders;
    this.timeoutMs = timeoutMs;
  }

  private headers(): Record<string, string> {
    const h: Record<string, string> = {
      "content-type": "application/json",
      accept: "text/event-stream",
    };
    if (this.apiKey) h.authorization = `Bearer ${this.apiKey}`;
    return { ...h, ...this.extraHeaders };
  }

  private payload(
    messages: ChatMessage[],
    tools: unknown[],
    temperature: number,
  ): Record<string, unknown> {
    const payload: Record<string, unknown> = {
      model: this.model,
      messages,
      stream: true,
      temperature,
    };
    if (!this.skipStreamOptions) {
      payload.stream_options = { include_usage: true };
    }
    if (tools.length > 0) {
      payload.tools = tools;
      payload.tool_choice = "auto";
    }
    return payload;
  }

  async *streamChat(
    messages: ChatMessage[],
    tools: unknown[],
    options: StreamChatOptions = {},
  ): AsyncGenerator<Delta> {
    const url = `${this.baseUrl}/chat/completions`;
    const payload = this.payload(messages, tools, options.temperature ?? 0.2);
    this.lastUsage = null;

    const accContent: string[] = [];
    const accTools = new Map<number, ToolCall>();
    const usage: Usage = { prompt_tokens: 0, completion_tokens: 0 };
    let model = this.model;

    const post = (body: Record<string, unknown>): Promise<Response> =>
      fetch(url, {
        method: "POST",
        headers: this.headers(),
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(this.timeoutMs),
      });

    let resp = await post(payload);
    if (resp.status === 400) {
      const body = await resp.text();
      if (body.includes("stream_options")) {
        this.skipStreamOptions = true;
        delete payload.stream_options;
        resp = await post(payload);
      } else {
        throw new APIError(friendlyError(400, body));
      }
    }
    if (!resp.ok) {
      const body = await resp.text();
      throw new APIError(friendlyError(resp.status, body));
    }
    if (!resp.body) throw new APIError("empty response body");

    const reader = resp.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    const handleEvent = (ev: Record<string, unknown>): Delta | null => {
      if (ev.error) {
        throw new APIError(JSON.stringify(ev.error));
      }
      const u = ev.usage as Record<string, unknown> | undefined;
      if (u) {
        usage.prompt_tokens = Number(u.prompt_tokens ?? 0);
        usage.completion_tokens = Number(u.completion_tokens ?? 0);
      }
      if (typeof ev.model === "string" && ev.model) model = ev.model;

      const choices = (ev.choices as Array<Record<string, unknown>>) ?? [];
      const choice = choices[0] ?? {};
      const delta = (choice.delta ?? choice.message ?? {}) as Record<string, unknown>;

      const piece = delta.content;
      const reasoning = (delta.reasoning_content ?? delta.reasoning) as string | undefined;
      const toolCalls = delta.tool_calls as Array<Record<string, unknown>> | undefined;

      if (typeof piece === "string" && piece) {
        accContent.push(piece);
        return { content: piece };
      }
      if (typeof reasoning === "string" && reasoning) {
        return { reasoning };
      }
      if (Array.isArray(toolCalls)) {
        for (const tc of toolCalls) {
          const idx = Number(tc.index ?? 0);
          const fn = (tc.function ?? {}) as Record<string, unknown>;
          const slot: ToolCall = accTools.get(idx) ?? {
            id: "",
            type: "function",
            function: { name: "", arguments: "" },
          };
          if (typeof tc.id === "string" && tc.id) slot.id = tc.id;
          if (typeof fn.name === "string" && fn.name) slot.function.name = fn.name;
          if (typeof fn.arguments === "string") {
            slot.function.arguments += fn.arguments;
          }
          accTools.set(idx, slot);
        }
      }
      return null;
    };

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let nl: number;
      while ((nl = buffer.indexOf("\n")) !== -1) {
        let line = buffer.slice(0, nl);
        buffer = buffer.slice(nl + 1);
        line = line.replace(/\r$/, "");
        if (!line) continue;
        if (line.startsWith("data: ")) line = line.slice(6);
        if (line.trim() === "[DONE]") continue;
        if (!line.startsWith("{")) continue;
        try {
          const ev = JSON.parse(line) as Record<string, unknown>;
          const d = handleEvent(ev);
          if (d) yield d;
        } catch (e) {
          if (e instanceof APIError) throw e;
          // JSONDecodeError equivalent: skip malformed line
        }
      }
    }

    this.lastUsage = { ...usage, model };
    if (accTools.size > 0) {
      const ordered = [...accTools.entries()].sort(([a], [b]) => a - b).map(([, v]) => v);
      yield { toolCalls: ordered };
    }
  }
}

function friendlyError(status: number, body: string): string {
  if (status === 403 && body.includes("FreeTierError")) {
    return (
      "Zen's free tier is now limited to the official OpenCode client. " +
      "Options: set an API key from opencode.ai/auth (use --config or -k), " +
      "point -b/-k at any other OpenAI-compatible endpoint, or run a local " +
      "model (e.g. Ollama at http://localhost:11434/v1)."
    );
  }
  return `HTTP ${status}: ${trim(body, 800)}`;
}
