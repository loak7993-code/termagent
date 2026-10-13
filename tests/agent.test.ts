import { describe, expect, it, afterEach } from "vitest";

import { FakeZen } from "./helpers/mock-server.js";
import { Agent } from "../src/agent/agent.js";
import { loadConfig, defaultConfig } from "../src/agent/config.js";
import { createToolContext } from "../src/agent/tools/base.js";

const cleanup: Array<() => void> = [];
afterEach(() => {
  while (cleanup.length) cleanup.pop()?.();
});

async function makeAgent(withReasoning = false) {
  const fake = new FakeZen();
  fake.options = { withReasoning };
  const baseUrl = await fake.start();
  cleanup.push(() => fake.stop());
  const cfg = { ...defaultConfig(), apiKey: "", baseUrl, model: "mock-free", maxTurns: 4 };
  const events = {
    text: [] as string[],
    reasoning: [] as string[],
    tools: [] as Array<{ id: string; name: string; args: Record<string, unknown> }>,
    results: [] as Array<{ name: string; result: string; isError: boolean; durationMs: number }>,
    meta: [] as Array<Record<string, unknown>>,
    errors: [] as string[],
  };
  const agent = new Agent(cfg, createToolContext({ cwd: "/tmp", approve: () => true }), {
    onText: (c) => events.text.push(c),
    onReasoning: (c) => events.reasoning.push(c),
    onToolStart: (id, name, args) => events.tools.push({ id, name, args }),
    onToolResult: (name, _a, result, isError, durationMs) =>
      events.results.push({ name, result, isError, durationMs }),
    onError: (m) => events.errors.push(m),
    onMeta: (m) => events.meta.push(m as unknown as Record<string, unknown>),
  });
  agent.addSystem();
  return { agent, events, fake };
}

describe("agent loop against a mock OpenAI SSE endpoint", () => {
  it("streams text, accumulates chunked tool calls, executes them, reports usage", async () => {
    const { agent, events, fake } = await makeAgent();
    await agent.userTurn("run echo hi");

    expect(events.errors).toEqual([]);
    expect(events.text.join("")).toBe("Running done");
    expect(events.tools).toEqual([{ id: "call_1", name: "bash", args: { command: "echo hi" } }]);
    expect(events.results[0]?.name).toBe("bash");
    expect(events.results[0]?.result).toContain("hi");
    expect(events.results[0]?.isError).toBe(false);
    expect(typeof events.results[0]?.durationMs).toBe("number");
    const meta = events.meta.at(-1) as { promptTokens: number; completionTokens: number; model: string };
    expect(meta.promptTokens).toBe(1200);
    expect(meta.completionTokens).toBe(34);
    expect(meta.model).toBe("mock-free");
    const roles = agent.messages.map((m) => m.role);
    expect(roles).toEqual(["system", "user", "assistant", "tool", "assistant"]);
    expect(agent.messages[3]?.tool_call_id).toBe("call_1");
    void fake;
  });

  it("falls back gracefully when the provider rejects stream_options", async () => {
    const fake = new FakeZen();
    fake.options = { rejectStreamOptions: true };
    const baseUrl = await fake.start();
    cleanup.push(() => fake.stop());
    const cfg = { ...defaultConfig(), baseUrl, model: "mock-free", maxTurns: 4 };
    const agent = new Agent(cfg, createToolContext({ cwd: "/tmp", approve: () => true }), {
      onText: () => undefined,
    });
    agent.addSystem();
    await agent.userTurn("hello");
    expect(agent.client.lastUsage?.completion_tokens).toBe(34);
  });

  it("forwards reasoning deltas", async () => {
    const { agent, events } = await makeAgent(true);
    await agent.userTurn("go");
    expect(events.reasoning.join("")).toContain("thinking hard");
    expect(events.errors).toEqual([]);
  });
});

describe("config", () => {
  it("applies env overrides over file values", () => {
    process.env.TERMAGENT_MODEL = "test-model";
    const cfg = loadConfig();
    expect(cfg.model).toBe("test-model");
    delete process.env.TERMAGENT_MODEL;
  });
});
