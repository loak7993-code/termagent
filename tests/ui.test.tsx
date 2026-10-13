import { describe, expect, it } from "vitest";
import React from "react";
import { render } from "ink-testing-library";

import { ThemeProvider } from "../src/providers/theme-provider.js";
import { opencodeTheme } from "../src/lib/terminal-themes/opencode.js";
import { Badge } from "../src/components/ui/badge.js";
import { ToolCall } from "../src/components/ui/tool-call.js";
import { ToolApproval } from "../src/components/ui/tool-approval.js";
import { ModelSelector } from "../src/components/ui/model-selector.js";
import { TokenUsage } from "../src/components/ui/token-usage.js";
import { ThinkingBlock } from "../src/components/ui/thinking-block.js";
import { DiffView } from "../src/components/ui/diff-view.js";

const frame = (output: string): string => output.replace(/\u001b\[[0-9;]*m/g, "");

describe("termcn component renders", () => {
  it("Badge shows variant text", () => {
    const { lastFrame } = render(
      React.createElement(ThemeProvider, { theme: opencodeTheme, children: React.createElement(Badge, { variant: "success" }, "free tier") }),
    );
    expect(frame(lastFrame() ?? "")).toContain("free tier");
  });

  it("ToolCall shows running spinner then success + duration", () => {
    const { lastFrame, rerender } = render(
      React.createElement(
        ThemeProvider,
        {
          theme: opencodeTheme,
          children: React.createElement(ToolCall, { name: "bash", args: { command: "echo hi" }, status: "running" }),
        },
      ),
    );
    expect(frame(lastFrame() ?? "")).toContain("bash");
    rerender(
      React.createElement(
        ThemeProvider,
        {
          theme: opencodeTheme,
          children: React.createElement(ToolCall, { name: "bash", args: { command: "echo hi" }, status: "success", duration: 840 }),
        },
      ),
    );
    expect(frame(lastFrame() ?? "")).toContain("840ms");
  });

  it("ToolApproval shows risk badge and action shortcuts", () => {
    const { lastFrame } = render(
      React.createElement(
        ThemeProvider,
        {
          theme: opencodeTheme,
          children: React.createElement(ToolApproval, {
            name: "bash",
            description: "rm -rf /tmp/old",
            risk: "high",
          }),
        },
      ),
    );
    const out = frame(lastFrame() ?? "");
    expect(out).toContain("HIGH");
    expect(out).toContain("rm -rf /tmp/old");
    expect(out).toContain("Approve");
    expect(out).toContain("Deny");
  });

  it("ModelSelector lists models with provider groups", () => {
    const { lastFrame } = render(
      React.createElement(
        ThemeProvider,
        {
          theme: opencodeTheme,
          children: React.createElement(ModelSelector, {
            models: [
              { id: "big-pickle", name: "Big Pickle", provider: "free" },
              { id: "glm-5.2", name: "GLM 5.2", provider: "paid" },
            ],
            defaultValue: "big-pickle",
            groupByProvider: true,
          }),
        },
      ),
    );
    const out = frame(lastFrame() ?? "");
    expect(out).toContain("Big Pickle");
    expect(out).toContain("GLM 5.2");
    expect(out).toContain("free");
    expect(out).toContain("paid");
  });

  it("TokenUsage renders the ⟨ in / out ⟩ line", () => {
    const { lastFrame } = render(
      React.createElement(
        ThemeProvider,
        {
          theme: opencodeTheme,
          children: React.createElement(TokenUsage, { prompt: 1500, completion: 340, model: "mock-free" }),
        },
      ),
    );
    const out = frame(lastFrame() ?? "");
    expect(out).toContain("1.5k");
    expect(out).toContain("340");
    expect(out).toContain("mock-free");
  });

  it("ThinkingBlock renders collapsed with duration", () => {
    const { lastFrame } = render(
      React.createElement(
        ThemeProvider,
        {
          theme: opencodeTheme,
          children: React.createElement(ThinkingBlock, { content: "hidden", label: "Thought", duration: 2300 }),
        },
      ),
    );
    const out = frame(lastFrame() ?? "");
    expect(out).toContain("Thought");
    expect(out).toContain("2.3s");
    expect(out).not.toContain("hidden");
  });

  it("DiffView shows + and - lines", () => {
    const { lastFrame } = render(
      React.createElement(
        ThemeProvider,
        {
          theme: opencodeTheme,
          children: React.createElement(DiffView, {
            oldText: "const a = 1;\n",
            newText: "const a = 2;\n",
            filename: "x.ts",
          }),
        },
      ),
    );
    const out = frame(lastFrame() ?? "");
    expect(out).toContain("-const a = 1;");
    expect(out).toContain("+const a = 2;");
  });
});
