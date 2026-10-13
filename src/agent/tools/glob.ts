// glob tool: recursive filename search. Port of tools/glob.py.

import { readdirSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, isAbsolute, join, relative, resolve as resolvePath } from "node:path";

import { err, ok, register, type ToolContext } from "./base.js";
import { resolve } from "./read.js";

function globToRegExp(pattern: string): RegExp {
  // fnmatch-style: ** crosses directory separators, * and ? do not.
  let re = "";
  for (let i = 0; i < pattern.length; i++) {
    const c = pattern[i];
    if (c === "*") {
      if (pattern[i + 1] === "*") {
        re += ".*";
        i++;
        if (pattern[i + 1] === "/") i++;
      } else {
        re += "[^/]*";
      }
    } else if (c === "?") {
      re += "[^/]";
    } else if ("\\^$.|+()[]{}".includes(c)) {
      re += "\\" + c;
    } else {
      re += c;
    }
  }
  return new RegExp(`^${re}$`);
}

function walk(root: string): string[] {
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
      out.push(p);
    }
  }
  return out;
}

export function runGlob(
  pattern: string,
  path: string,
  ctx: ToolContext,
): { content: string; isError: boolean } {
  if (!pattern) return err("error: pattern is required");
  let base = resolve(ctx, path || ".");
  let seg = pattern.replace(/^[.][/\\]+/, "");
  try {
    if (isAbsolute(pattern) || pattern.startsWith("~")) {
      const abs = pattern.startsWith("~") ? join(homedir(), pattern.slice(1)) : pattern;
      base = resolvePath(abs).replace(/[/\\][^/\\]*$/, "");
      seg = basename(abs);
    }
    statSync(base);
  } catch {
    return err(`error: base path not found: ${base}`);
  }

  const rx = globToRegExp(seg);
  const matches: string[] = [];
  const maxHits = 2000;
  for (const p of walk(base)) {
    const rel = relative(base, p).split("\\").join("/");
    if (rx.test(rel) || rx.test(basename(p))) {
      matches.push(p);
      if (matches.length >= maxHits) break;
    }
  }
  matches.sort();
  let body = matches.slice(0, maxHits).join("\n");
  if (matches.length >= maxHits) body += `\n… [truncated at ${maxHits} matches]`;
  return ok(body || "(no matches)");
}

register({
  name: "glob",
  description: "Find files by glob pattern, recursively from a base path.",
  params: {
    type: "object",
    properties: {
      pattern: { type: "string", description: "Glob pattern, e.g. **/*.py" },
      path: { type: "string", description: "Base directory (default cwd)." },
    },
    required: ["pattern"],
  },
  read_only: true,
  run: (a, ctx) => runGlob(String(a.pattern ?? ""), String(a.path ?? ""), ctx),
});
