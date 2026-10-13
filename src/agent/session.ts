// JSONL session save/resume. Port of session.py — same file format, so
// sessions saved with the Python build load here and vice versa.

import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { randomUUID } from "node:crypto";

import { configDir } from "./config.js";
import type { ChatMessage } from "./types.js";

function sessionsRoot(): string {
  return join(configDir(), "sessions");
}

export function newSessionId(): string {
  const d = new Date();
  const pad = (n: number, w = 2) => String(n).padStart(w, "0");
  const stamp =
    `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}` +
    `-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;
  return `${stamp}-${randomUUID().slice(0, 6)}`;
}

export function sessionPath(sessionId: string): string {
  return join(sessionsRoot(), `${sessionId}.jsonl`);
}

export function listSessions(): Array<{ id: string; preview: string }> {
  const root = sessionsRoot();
  if (!existsSync(root)) return [];
  const out: Array<{ id: string; preview: string }> = [];
  for (const f of readdirSorted(root)) {
    try {
      const first = JSON.parse(readFileSync(join(root, f), "utf8").split("\n")[0] ?? "{}");
      out.push({ id: f.replace(/\.jsonl$/, ""), preview: first.preview ?? "(no preview)" });
    } catch {
      out.push({ id: f.replace(/\.jsonl$/, ""), preview: "(no preview)" });
    }
  }
  return out;
}

function readdirSorted(root: string): string[] {
  return readdirSync(root)
    .filter((f) => f.endsWith(".jsonl"))
    .sort((a, b) => statSync(join(root, b)).mtimeMs - statSync(join(root, a)).mtimeMs);
}

function previewOf(content: ChatMessage["content"]): string {
  if (typeof content === "string") return content.replace(/\n/g, " ").slice(0, 120);
  return "";
}

export function saveMessage(sessionId: string, role: string, content: ChatMessage["content"]): void {
  const root = sessionsRoot();
  mkdirSync(root, { recursive: true });
  const path = sessionPath(sessionId);
  if (!existsSync(path)) {
    const meta = JSON.stringify({ preview: previewOf(content) });
    writeFileSync(path, meta + "\n", "utf8");
  }
  const rec = JSON.stringify({ role, content, ts: Date.now() / 1000 });
  appendFileSync(path, rec + "\n", "utf8");
}

export function loadSession(sessionId: string): ChatMessage[] {
  const path = sessionPath(sessionId);
  if (!existsSync(path)) return [];
  const msgs: ChatMessage[] = [];
  const lines = readFileSync(path, "utf8").split("\n");
  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    if (!line) continue;
    try {
      const rec = JSON.parse(line) as ChatMessage;
      msgs.push({ role: rec.role, content: rec.content });
    } catch {
      // skip corrupted lines
    }
  }
  return msgs;
}
