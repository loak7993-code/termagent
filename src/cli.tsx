#!/usr/bin/env node
// CLI entry. Interactive = full-screen Ink app; one-shot = plain streaming.

import React from "react";
import { render } from "ink";

import { App } from "./app/App.js";
import { Store } from "./app/store.js";
import { Agent } from "./agent/agent.js";
import {
  loadConfig,
  saveConfig,
  defaultSystemPrompt,
  type Config,
} from "./agent/config.js";
import { ensureModelAvailable, handleSlash, setModel, type CommandDeps } from "./app/commands.js";
import { THEMES, getTheme } from "./lib/themes.js";
import { newSessionId, loadSession, saveMessage } from "./agent/session.js";
import { createToolContext } from "./agent/tools/base.js";
import "./agent/tools/index.js";
import { UnicodeProvider } from "./providers/unicode-provider.js";

export const VERSION = "1.0.0";

interface CliArgs {
  prompt: string[];
  model?: string;
  baseUrl?: string;
  apiKey?: string;
  cwd: string;
  autoApprove: boolean;
  session?: string;
  theme?: string;
  listThemes: boolean;
  config: boolean;
  maxTurns?: number;
  version: boolean;
  help: boolean;
}

function parseArgs(argv: string[]): CliArgs {
  const args: CliArgs = {
    prompt: [],
    cwd: process.cwd(),
    autoApprove: false,
    listThemes: false,
    config: false,
    version: false,
    help: false,
  };
  const valueFlags: Array<[string, keyof CliArgs]> = [
    ["-m", "model"],
    ["--model", "model"],
    ["-b", "baseUrl"],
    ["--base-url", "baseUrl"],
    ["-k", "apiKey"],
    ["--api-key", "apiKey"],
    ["-c", "cwd"],
    ["--cwd", "cwd"],
    ["-s", "session"],
    ["--session", "session"],
    ["--theme", "theme"],
    ["--max-turns", "maxTurns"],
  ];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i] ?? "";
    if (a === "-a" || a === "--auto-approve") args.autoApprove = true;
    else if (a === "--list-themes") args.listThemes = true;
    else if (a === "--config") args.config = true;
    else if (a === "--version" || a === "-v") args.version = true;
    else if (a === "--help" || a === "-h") args.help = true;
    else {
      const flag = valueFlags.find(([f]) => f === a);
      if (flag) {
        const key = flag[1];
        const v = argv[++i] ?? "";
        (args[key] as unknown) = key === "maxTurns" ? Number.parseInt(v, 10) || 24 : v;
      } else {
        args.prompt.push(a);
      }
    }
  }
  return args;
}

const NO_KEY_MESSAGE =
  "Zen's free tier is now limited to the official OpenCode client, so this " +
  "provider needs an API key (get one at opencode.ai/auth). Alternatives:\n" +
  "  --config            set a key interactively\n" +
  "  -b <url> -k <key>   any OpenAI-compatible endpoint (OpenRouter, Groq,\n" +
  "                      local Ollama, llama.cpp, LM Studio)\n" +
  "  /models             pick a model once a key is set";

export async function main(): Promise<number> {
  const args = parseArgs(process.argv.slice(2));
  if (args.version) {
    console.log(`termagent ${VERSION}`);
    return 0;
  }
  if (args.help) {
    console.log(
      [
        "termagent — terminal AI coding agent (Ink + termcn UI)",
        "",
        "Usage:",
        "  termagent                      interactive TUI",
        '  termagent "prompt"            one-shot, then exit',
        "  termagent -m <model> -b <url> -k <key>   override provider settings",
        "  termagent -c <dir>             working directory",
        "  termagent -a                   auto-approve tool calls",
        "  termagent -s <id>              resume a saved session",
        "  termagent --theme <name>       UI theme (see --list-themes)",
        "  termagent --config             interactive configuration",
        "",
        "Inside the TUI: /help lists all commands.",
      ].join("\n"),
    );
    return 0;
  }

  const cfg = loadConfig();
  if (args.model) cfg.model = args.model;
  if (args.baseUrl) cfg.baseUrl = args.baseUrl;
  if (args.apiKey) cfg.apiKey = args.apiKey;
  if (args.maxTurns) cfg.maxTurns = args.maxTurns;
  if (args.autoApprove) cfg.autoApprove = true;
  if (args.theme) cfg.theme = args.theme;
  cfg.theme = getTheme(cfg.theme).name;

  if (args.listThemes) {
    for (const name of Object.keys(THEMES).sort()) {
      const mark = name === "opencode" ? "  ← default" : "";
      console.log(`  ${name}${mark}`);
    }
    return 0;
  }

  if (args.config) return configureInteractive(cfg);

  if (args.prompt.length === 0) {
    return interactive(cfg, args.cwd, args.session);
  }
  return oneshot(cfg, args.cwd, args.prompt.join(" "));
}

interface InteractiveRefs {
  agent: Agent;
  sessionId: string;
  cwd: string;
  agentToolSeq: number;
  reasoning: { text: string; startedAt: number } | null;
}

async function interactive(cfg: Config, cwd: string, resume?: string): Promise<number> {
  const refs: InteractiveRefs = {
    agent: null as unknown as Agent,
    sessionId: resume ?? newSessionId(),
    cwd,
    agentToolSeq: 0,
    reasoning: null,
  };

  const store = new Store({
    model: cfg.model,
    cwd,
    themeName: cfg.theme,
    version: VERSION,
  });
  store.autoApprove = cfg.autoApprove;

  const buildAgent = (): Agent => {
    const ctx = createToolContext({
      cwd: refs.cwd,
      approve: (kind, detail) => store.requestApproval(kind, detail),
      emit: (line) => store.push({ kind: "notice", text: line }),
    });
    const agent = new Agent(cfg, ctx, {
      onText: (chunk) => {
        if (refs.reasoning) {
          const seconds = (Date.now() - refs.reasoning.startedAt) / 1000;
          const text = refs.reasoning.text;
          refs.reasoning = null;
          if (cfg.showReasoning && text.trim()) {
            store.push({ kind: "panel", title: "reasoning", rows: [["text", text.trim()]] });
          } else if (seconds > 0.05) {
            store.push({ kind: "thinking", seconds });
          }
        }
        store.appendAssistant(chunk);
      },
      onReasoning: (chunk) => {
        if (!refs.reasoning) refs.reasoning = { text: "", startedAt: Date.now() };
        refs.reasoning.text += chunk;
      },
      onToolStart: (id, name, args) => {
        store.pushTool({ id, name, args, status: "running" });
      },
      onToolResult: (name, _args, result, isError, durationMs) => {
        for (let i = store.feed.length - 1; i >= 0; i--) {
          const f = store.feed[i];
          if (f.kind === "tool" && f.call.name === name && f.call.status === "running") {
            store.updateTool(f.call.id, {
              status: isError ? "error" : "success",
              result,
              isError,
              durationMs,
            });
            break;
          }
        }
      },
      onAssistantDone: () => store.endAssistant(),
      onError: (message) => store.push({ kind: "error", text: message }),
      onMeta: (meta) => {
        if (meta.promptTokens || meta.completionTokens) {
          store.push({ kind: "usage", meta });
        }
      },
    });
    agent.addSystem();
    return agent;
  };

  refs.agent = buildAgent();

  const deps: CommandDeps = {
    cfg,
    agentRef: { get current() { return refs.agent; }, set current(a: Agent) { refs.agent = a; } },
    store,
    sessionId: refs.sessionId,
    getcwd: () => refs.cwd,
    setcwd: (p) => {
      refs.cwd = p;
    },
    exit: () => undefined,
    submit: async (text: string) => {
      await submitTurn(text);
    },
  };

  const submitTurn = async (text: string): Promise<void> => {
    if (store.busy) return;
    store.pushUser(text);
    saveMessage(refs.sessionId, "user", text);
    store.setBusy(true);
    store.startAssistant();
    try {
      await refs.agent.userTurn(text);
      const last = refs.agent.messages[refs.agent.messages.length - 1];
      if (last && last.role === "assistant" && last.content) {
        saveMessage(refs.sessionId, "assistant", last.content);
      }
    } finally {
      store.setBusy(false);
    }
  };

  const resetAgent = (): void => {
    refs.agent = buildAgent();
  };

  const handleSubmit = async (text: string): Promise<void> => {
    if (text.startsWith("/")) {
      const cmd = text.trim().split(/\s+/)[0]?.toLowerCase();
      if (cmd === "/clear") {
        // /clear resets the agent; handleSlash does the bookkeeping via resetAgent
        await handleSlash(text, { ...deps, submit: handleSubmit });
        resetAgent();
        return;
      }
      if (cmd === "/exit" || cmd === "/quit" || cmd === "/q") {
        store.push({ kind: "notice", text: "bye" });
        setTimeout(() => instance.unmount(), 200);
        return;
      }
      await handleSlash(text, { ...deps, submit: handleSubmit });
      return;
    }
    await submitTurn(text);
  };

  if (!cfg.apiKey && cfg.baseUrl.toLowerCase().includes("opencode.ai/zen")) {
    store.push({ kind: "error", text: NO_KEY_MESSAGE });
  }
  await ensureModelAvailable(cfg, store);

  if (resume) {
    const msgs = loadSession(resume);
    refs.agent.messages.push(...msgs);
    store.push({ kind: "notice", text: `resumed session ${resume}` });
  }

  const makeRoot = (themeName: string): React.ReactElement => {
    const inner = React.createElement(App, {
      store,
      onSubmit: handleSubmit,
      onModelPick: (id: string) => {
        setModel(cfg, refs.agent, store, id);
        saveConfig(cfg);
        store.push({ kind: "notice", text: `switched to ${id}` });
      },
      onThemePick: (name: string) => {
        store.setThemeName(name);
        cfg.theme = name;
        saveConfig(cfg);
        store.push({ kind: "notice", text: `theme set to ${name} (saved)` });
      },
      onExit: () => {
        store.push({ kind: "notice", text: "bye" });
        setTimeout(() => instance.unmount(), 250);
      },
    });
    return React.createElement(
      UnicodeProvider,
      { children: inner },
    );
  };

  const instance = render(makeRoot(cfg.theme), { exitOnCtrlC: true });
  await instance.waitUntilExit();
  return 0;
}

async function oneshot(cfg: Config, cwd: string, prompt: string): Promise<number> {
  if (!cfg.apiKey && !cfg.baseUrl.toLowerCase().includes("opencode.ai/zen")) {
    console.error("error: no API key. Set OPENAI_API_KEY, run --config, or use a free Zen model.");
    return 2;
  }
  const ctx = createToolContext({ cwd, approve: () => cfg.autoApprove });
  const agent = new Agent(cfg, ctx, {
    onText: (chunk) => process.stdout.write(chunk),
    onToolResult: (name, args, result, isError) => {
      if (process.stderr.isTTY) {
        const sym = isError ? "✗" : "✓";
        console.error(`${sym} ${name} (${isError ? "error" : "ok"})`);
        void args;
        void result;
      }
    },
    onError: (m) => console.error(m),
  });
  agent.addSystem();
  await agent.userTurn(prompt);
  process.stdout.write("\n");
  return 0;
}

async function configureInteractive(cfg: Config): Promise<number> {
  const readline = await import("node:readline/promises");
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  console.log("termagent config");
  const apiKey = (await rl.question("API key (blank keeps current): ")).trim();
  if (apiKey) cfg.apiKey = apiKey;
  cfg.baseUrl = (await rl.question(`Base URL [${cfg.baseUrl}]: `)).trim() || cfg.baseUrl;
  cfg.model = (await rl.question(`Model [${cfg.model}]: `)).trim() || cfg.model;
  cfg.theme = (await rl.question(`Theme [${cfg.theme}]: `)).trim() || cfg.theme;
  const turns = (await rl.question(`Max turns [${cfg.maxTurns}]: `)).trim();
  if (turns) cfg.maxTurns = Number.parseInt(turns, 10) || cfg.maxTurns;
  const auto = (await rl.question("Auto-approve tool calls? (y/N): ")).trim();
  cfg.autoApprove = auto.toLowerCase().startsWith("y");
  cfg.system = cfg.system || defaultSystemPrompt();
  rl.close();
  saveConfig(cfg);
  console.log("config saved");
  return 0;
}

import { realpathSync } from "node:fs";
import { pathToFileURL } from "node:url";

const invokedDirectly =
  !!process.argv[1] &&
  import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href;
if (invokedDirectly) {
  main().then((code) => {
    process.exitCode = code;
  });
}
