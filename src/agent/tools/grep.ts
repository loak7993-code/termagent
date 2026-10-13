// grep tool: ripgrep if available, pure-JS fallback. Port of tools/grep.py.

import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { basename, isAbsolute, join, relative } from "node:path";

import { err, ok, register, type ToolContext } from "./base.js";
import { resolve } from "./read.js";

const MAX_HITS = 500;

function matchInclude(name: string, include: string): boolean {
  for (const pat of include.split(",")) {
    const p = pat.trim();
    if (!p) continue;
    if (globMatch(p, name)) return true;
  }
  return false;
}

function globMatch(pattern: string, name: string): boolean {
  const re = pattern
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*/g, ".*")
    .replace(/\?/g, ".");
  return new RegExp(`^${re}$`).test(name);
}

function walkFiles(root: string): string[] {
  const out: string[] = [];
  const stack = [root];
  while (stack.length > 0) {
    const dir = stack.pop() as string;
    let entries: string[];
    try {
      entries = readdirSync(dir);
    } catch {
      continue;
    }
    for (const name of entries) {
      const p = join(dir, name);
      if (name.startsWith(".git")) continue;
      let st;
      try {
        st = statSync(p);
      } catch {
        continue;
      }
      if (st.isDirectory()) stack.push(p);
      else out.push(p);
    }
  }
  return out;
}

function runFallback(
  pattern: string,
  path: string,
  include: string,
  ctx: ToolContext,
): { content: string; isError: boolean } {
  let rx: RegExp;
  try {
    rx = new RegExp(pattern);
  } catch (e) {
    return err(`error: invalid regex: ${(e as Error).message}`);
  }
  const base = resolve(ctx, path || ".");
  let rootSt;
  try {
    rootSt = statSync(base);
  } catch {
    return err(`error: not found: ${base}`);
  }
  const root = rootSt.isDirectory() ? base : base.replace(/[/\\][^/\\]*$/, "");
  const out: string[] = [];
  let count = 0;
  for (const p of walkFiles(root)) {
    if (include && !matchInclude(basename(p), include)) continue;
    let text: string;
    try {
      text = readFileSync(p, "utf8");
    } catch {
      continue;
    }
    const rel = isAbsolute(relative(ctx.cwd, p)) ? p : relative(ctx.cwd, p);
    const lines = text.split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (!rx.test(lines[i])) continue;
      const snippet = lines[i].length <= 500 ? lines[i] : lines[i].slice(0, 500) + " …";
      out.push(`${rel}:${i + 1}: ${snippet}`);
      count++;
      if (count >= MAX_HITS) {
        out.push(`… [truncated at ${MAX_HITS} matches]`);
        return ok(out.join("\n"));
      }
    }
  }
  return ok(out.join("\n") || "(no matches)");
}

export function runGrep(
  pattern: string,
  path: string,
  include: string,
  ctx: ToolContext,
): { content: string; isError: boolean } {
  if (!pattern) return err("error: pattern is required");
  const base = resolve(ctx, path || ".");
  try {
    const rg = process.platform === "win32" ? "rg.exe" : "rg";
    const cmd = [rg, "-n", "--no-heading", "--color=never", "--max-count", String(MAX_HITS)];
    if (include) {
      for (const pat of include.split(",")) {
        if (pat.trim()) cmd.push("-g", pat.trim());
      }
    }
    cmd.push("--", pattern, base);
    const body = execFileSync(cmd[0], cmd.slice(1), {
      timeout: 60_000,
      stdio: ["ignore", "pipe", "ignore"],
      encoding: "utf8",
    }).replace(/\n$/, "");
    return ok(body || "(no matches)");
  } catch {
    // rg missing or failed: fall back
  }
  return runFallback(pattern, path, include, ctx);
}

register({
  name: "grep",
  description:
    "Search file contents with a regex. Returns path:line:match. Uses ripgrep if available, else a built-in fallback.",
  params: {
    type: "object",
    properties: {
      pattern: { type: "string", description: "Regex pattern." },
      path: { type: "string", description: "File or dir to search (default cwd)." },
      include: {
        type: "string",
        description: "Comma-separated glob filter e.g. *.py,*.ts",
      },
    },
    required: ["pattern"],
  },
  read_only: true,
  run: (a, ctx) =>
    runGrep(String(a.pattern ?? ""), String(a.path ?? ""), String(a.include ?? ""), ctx),
});
