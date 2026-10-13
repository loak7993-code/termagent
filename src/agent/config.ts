// Config: JSON file (~/.config/termagent/config.json) + env overrides.
// Port of termagent/config.py — same file, same fields, same precedence.

import { existsSync, mkdirSync, readFileSync, writeFileSync, chmodSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import { DEFAULT_FREE_MODEL } from "./zen.js";

export const CONFIG_DIR_ENV = "TERMAGENT_CONFIG_DIR";
const CONFIG_FILENAME = "config.json";

export interface Config {
  apiKey: string;
  baseUrl: string;
  model: string;
  system: string;
  maxTurns: number;
  autoApprove: boolean;
  historyLines: number;
  tools: string[];
  extraHeaders: Record<string, string>;
  theme: string;
  showReasoning: boolean;
}

export function defaultConfig(): Config {
  return {
    apiKey: "",
    baseUrl: "https://opencode.ai/zen/v1",
    model: DEFAULT_FREE_MODEL,
    system: "",
    maxTurns: 24,
    autoApprove: false,
    historyLines: 2000,
    tools: [],
    extraHeaders: {},
    theme: "opencode",
    showReasoning: false,
  };
}

export function configDir(): string {
  const env = process.env[CONFIG_DIR_ENV];
  if (env) return env;
  return join(homedir(), ".config", "termagent");
}

export function configPath(): string {
  return join(configDir(), CONFIG_FILENAME);
}

export function defaultSystemPrompt(): string {
  return [
    "You are termagent, an interactive terminal coding agent running on the",
    "user's machine (often Termux on Android). You help with software",
    "engineering tasks by using tools.",
    "",
    "Operating principles:",
    "- Be concise. Do not add commentary the user did not ask for.",
    "- Use tools to inspect the environment before assuming anything.",
    "- Prefer reading files before editing them; never guess file contents.",
    "- When editing files, preserve existing style and conventions.",
    "- Do not add comments to code unless asked.",
    "- Do not commit, push, or run destructive commands unless the user asks.",
    "- When a task needs multiple steps, use the todo tool to track them.",
    "- Answer in plain text; use markdown sparingly since output is a terminal.",
  ].join("\n");
}

const ENV_OVERRIDES: Array<[string, keyof Config, (v: string) => unknown]> = [
  ["OPENAI_API_KEY", "apiKey", (v) => v],
  ["TERMAGENT_API_KEY", "apiKey", (v) => v],
  ["OPENAI_BASE_URL", "baseUrl", (v) => v],
  ["TERMAGENT_BASE_URL", "baseUrl", (v) => v],
  ["OPENAI_MODEL", "model", (v) => v],
  ["TERMAGENT_MODEL", "model", (v) => v],
  ["TERMAGENT_THEME", "theme", (v) => v],
  [
    "TERMAGENT_AUTO_APPROVE",
    "autoApprove",
    (v) => ["1", "true", "yes"].includes(v.toLowerCase()),
  ],
  [
    "TERMAGENT_SHOW_REASONING",
    "showReasoning",
    (v) => ["1", "true", "yes"].includes(v.toLowerCase()),
  ],
  ["TERMAGENT_MAX_TURNS", "maxTurns", (v) => Number.parseInt(v, 10) || 24],
];

export function loadConfig(): Config {
  const cfg = defaultConfig();
  const path = configPath();
  if (existsSync(path)) {
    try {
      const data = JSON.parse(readFileSync(path, "utf8")) as Record<string, unknown>;
      for (const key of Object.keys(cfg) as Array<keyof Config>) {
        if (key in data) {
          (cfg[key] as unknown) = data[key];
        }
      }
    } catch {
      // corrupted config: fall back to defaults
    }
  }
  for (const [env, key, parse] of ENV_OVERRIDES) {
    const v = process.env[env];
    if (v !== undefined && v !== "") {
      (cfg[key] as unknown) = parse(v);
    }
  }
  if (!cfg.system) cfg.system = defaultSystemPrompt();
  return cfg;
}

export function saveConfig(cfg: Config): void {
  const path = configPath();
  mkdirSync(configDir(), { recursive: true });
  writeFileSync(path, JSON.stringify(cfg, null, 2) + "\n", "utf8");
  try {
    chmodSync(path, 0o600);
  } catch {
    // best effort
  }
}

export function configToDict(cfg: Config): Record<string, unknown> {
  const { apiKey, ...rest } = cfg;
  return { ...rest, hasApiKey: Boolean(apiKey) };
}
