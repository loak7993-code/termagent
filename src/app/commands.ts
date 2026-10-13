// Slash commands. Port of cli.py handle_slash + cmd_models/cmd_theme.

import { Agent } from "../agent/agent.js";
import { configToDict, saveConfig, type Config } from "../agent/config.js";
import {
  catalog,
  fetchAvailable,
  isFree,
} from "../agent/zen.js";
import { allTools } from "../agent/tools/index.js";
import { clearTodos } from "../agent/tools/todo.js";
import { THEMES, themeNames } from "../lib/themes.js";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve as resolvePath } from "node:path";
import {
  listSessions,
  loadSession,
  newSessionId,
  saveMessage,
  sessionPath,
} from "../agent/session.js";
import type { Store } from "./store.js";

export interface CommandDeps {
  cfg: Config;
  agentRef: { current: Agent };
  store: Store;
  sessionId: string;
  getcwd: () => string;
  setcwd: (p: string) => void;
  exit: () => void;
  submit: (text: string) => Promise<void>;
}

export async function handleSlash(
  line: string,
  deps: CommandDeps,
): Promise<void> {
  const { cfg, store } = deps;
  const agent = deps.agentRef.current;
  const parts = line.trim().split(/\s+/);
  const cmd = (parts[0] ?? "").toLowerCase();
  const arg = parts.slice(1).join(" ").trim();

  switch (cmd) {
    case "/help":
      store.setOverlay("help");
      return;
    case "/clear": {
      agent.messages = [];
      agent.addSystem();
      clearTodos(deps.getcwd());
      store.clearFeed();
      store.push({ kind: "notice", text: "conversation cleared" });
      return;
    }
    case "/model": {
      if (arg) {
        setModel(cfg, agent, store, arg);
        saveConfig(cfg);
        store.push({ kind: "notice", text: `model set to ${cfg.model}` });
      } else {
        store.push({ kind: "notice", text: `model: ${cfg.model}` });
      }
      return;
    }
    case "/models": {
      store.setStatus("fetching models…");
      const available = await fetchAvailable(cfg.baseUrl, cfg.apiKey);
      store.setStatus(null);
      const free = catalog().filter((m) => m.free && available.includes(m.id));
      const paid = catalog().filter((m) => !m.free && available.includes(m.id));
      const choices = [...free, ...paid].map((m) => ({
        id: m.id,
        label: m.label,
        free: m.free,
      }));
      if (choices.length === 0) {
        store.push({ kind: "error", text: "no models returned by provider; check base_url." });
        return;
      }
      store.modelChoices = choices;
      store.setOverlay("models");
      return;
    }
    case "/theme": {
      if (!arg) {
        store.modelChoices = themeNames().map((n) => ({
          id: n,
          label: n === "opencode" ? `${n} (default)` : n,
          free: false,
        }));
        store.setOverlay("themes");
        return;
      }
      if (!(arg in THEMES)) {
        store.push({ kind: "error", text: `unknown theme '${arg}'. Try: ${themeNames().join(", ")}` });
        return;
      }
      store.setThemeName(arg);
      cfg.theme = arg;
      saveConfig(cfg);
      store.push({ kind: "notice", text: `theme set to ${arg} (saved)` });
      return;
    }
    case "/config": {
      const rows = Object.entries(configToDict(cfg)).map(
        ([k, v]) => [k, String(v)] as [string, string],
      );
      store.push({ kind: "panel", title: "config", rows });
      return;
    }
    case "/approve": {
      if (["on", "true", "1", "yes"].includes(arg.toLowerCase())) cfg.autoApprove = true;
      else if (["off", "false", "0", "no"].includes(arg.toLowerCase())) cfg.autoApprove = false;
      else cfg.autoApprove = !cfg.autoApprove;
      store.autoApprove = cfg.autoApprove;
      store.push({
        kind: "notice",
        text: `auto-approve ${cfg.autoApprove ? "on" : "off"}`,
      });
      return;
    }
    case "/tools": {
      const rows = allTools().map(
        (t) =>
          [
            t.read_only ? `${t.name} [read-only]` : t.name,
            t.description,
          ] as [string, string],
      );
      store.push({ kind: "panel", title: "tools", rows });
      return;
    }
    case "/sys": {
      if (!arg) {
        store.push({ kind: "notice", text: "usage: /sys <text to append to system message>" });
        return;
      }
      const sys = agent.messages[0];
      if (sys && sys.role === "system") {
        sys.content = (sys.content ?? "") + "\n" + arg;
        store.push({ kind: "notice", text: "appended to system message" });
      }
      return;
    }
    case "/cwd": {
      if (arg) {
        const p = resolvePath(arg.replace(/^~(?=$|\/)/, process.env.HOME ?? "~"));
        try {
          if (!statSync(p).isDirectory()) {
            store.push({ kind: "error", text: `not a directory: ${p}` });
            return;
          }
        } catch {
          store.push({ kind: "error", text: `not a directory: ${p}` });
          return;
        }
        deps.setcwd(p);
        agent.ctx.cwd = p;
        store.setCwd(p);
        store.push({ kind: "notice", text: `cwd → ${p}` });
      } else {
        store.push({ kind: "notice", text: `cwd: ${deps.getcwd()}` });
      }
      return;
    }
    case "/sessions": {
      const rows = listSessions().map(
        (s) => [s.id, s.preview] as [string, string],
      );
      if (rows.length === 0) {
        store.push({ kind: "notice", text: "no saved sessions — /save to create one" });
      } else {
        store.push({ kind: "panel", title: "sessions", rows });
      }
      return;
    }
    case "/save": {
      const sid = arg || newSessionId();
      if (existsSync(sessionPath(sid))) {
        store.push({ kind: "error", text: `session ${sid} exists; choose a new id` });
        return;
      }
      let saved = 0;
      for (const m of agent.messages) {
        if (m.role === "system") continue;
        saveMessage(sid, m.role, m.content);
        saved++;
      }
      store.push({ kind: "notice", text: `saved session ${sid} (${saved} messages)` });
      return;
    }
    case "/load": {
      if (!arg) {
        store.push({ kind: "notice", text: "usage: /load <session-id>" });
        return;
      }
      const msgs = loadSession(arg);
      if (msgs.length === 0) {
        store.push({ kind: "error", text: `no session ${arg}` });
        return;
      }
      agent.messages = agent.messages.filter((m) => m.role === "system");
      agent.messages.push(...msgs);
      store.push({ kind: "notice", text: `loaded ${msgs.length} messages from ${arg}` });
      return;
    }
    case "/paste": {
      const text = openEditor();
      if (text.trim()) {
        await deps.submit(text);
      }
      return;
    }
    default:
      store.push({ kind: "error", text: `unknown command: ${cmd} (try /help)` });
  }
}

// submitted text from /paste needs the normal turn path
export interface CommandDepsWithSubmit extends CommandDeps {
  submit: (text: string) => Promise<void>;
}

function openEditor(): string {
  const editor = process.env.EDITOR ?? process.env.VISUAL ?? "vi";
  const file = join(tmpdir(), `termagent-paste-${Date.now()}.md`);
  try {
    writeFileSync(file, "", "utf8");
    execFileSync(editor, [file], { stdio: "inherit" });
    return readFileSync(file, "utf8");
  } catch {
    return "";
  } finally {
    try {
      rmSync(file);
    } catch {
      // ignore
    }
  }
}

export function setModel(cfg: Config, agent: Agent, store: Store, modelId: string): void {
  cfg.model = modelId;
  agent.client.model = modelId;
  store.setModel(modelId);
}

export async function ensureModelAvailable(cfg: Config, store: Store): Promise<void> {
  if (!cfg.baseUrl.toLowerCase().includes("opencode.ai/zen")) return;
  const available = await fetchAvailable(cfg.baseUrl, cfg.apiKey);
  if (available.includes(cfg.model) || available.length === 0) return;
  const free = available.filter((m) => isFree(m));
  const pick = free[0] ?? available[0];
  const old = cfg.model;
  cfg.model = pick;
  store.setModel(pick);
  saveConfig(cfg);
  store.push({
    kind: "notice",
    text: `model ${old} is not available; switched to ${pick}`,
  });
}
