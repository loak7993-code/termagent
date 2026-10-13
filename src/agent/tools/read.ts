// read tool: numbered lines with paging. Port of tools/read.py.

import { readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, join, resolve as resolvePath } from "node:path";

import { err, ok, register, type ToolContext } from "./base.js";

export function resolve(ctx: ToolContext, p: string): string {
  let path = p.startsWith("~") ? join(homedir(), p.slice(1)) : p;
  if (!isAbsolute(path)) path = join(ctx.cwd, path);
  return resolvePath(path);
}

export function runRead(
  path: string,
  offset: number,
  limit: number,
  ctx: ToolContext,
): { content: string; isError: boolean } {
  const p = resolve(ctx, path);
  let text: string;
  try {
    const st = statSync(p);
    if (st.isDirectory()) return err(`${p} is a directory`);
    text = readFileSync(p, "utf8");
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "ENOENT") return err(`error: file not found: ${p}`);
    return err(`error: ${(e as Error).message}`);
  }

  const lines = text.split("\n");
  if (lines.length > 0 && lines[lines.length - 1] === "") lines.pop();
  const start = Math.max(1, offset);
  const end = start + (limit > 0 ? limit : 2000) - 1;
  const endIdx = Math.min(end, lines.length);
  const width = String(endIdx).length;
  const out: string[] = [];
  for (let i = start; i <= endIdx; i++) {
    let line = lines[i - 1] ?? "";
    if (line.length > 2000) line = line.slice(0, 2000) + " …[truncated]";
    out.push(`${String(i).padStart(width)}: ${line}`);
  }
  let body = out.join("\n");
  if (start > 1) body = `… [showing lines ${start}-${endIdx} of ${lines.length}]\n` + body;
  if (endIdx < lines.length) body += `\n… [${lines.length - endIdx} more lines]`;
  return ok(body || "(empty file)");
}

register({
  name: "read",
  description:
    "Read a text file and return numbered lines. Use offset/limit to page. Fails on directories.",
  params: {
    type: "object",
    properties: {
      path: { type: "string", description: "File path (relative or absolute)." },
      offset: { type: "integer", description: "1-indexed line to start at (default 1)." },
      limit: { type: "integer", description: "Max lines to return (default 2000)." },
    },
    required: ["path"],
  },
  read_only: true,
  run: (a, ctx) =>
    runRead(
      String(a.path ?? ""),
      Number(a.offset ?? 1) || 1,
      Number(a.limit ?? 2000) || 2000,
      ctx,
    ),
});
