// ls tool: directory listing or single-file stat. Port of tools/ls.py.

import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";

import { err, ok, register, type ToolContext } from "./base.js";
import { resolve } from "./read.js";

function fmtSize(n: number): string {
  let size = n;
  for (const unit of ["B", "K", "M", "G", "T"] as const) {
    if (size < 1024 || unit === "T") {
      return unit === "B" ? `${Math.round(size)}${unit}` : `${size.toFixed(1)}${unit}`;
    }
    size /= 1024;
  }
  return `${size}B`;
}

function fileMode(mode: number): string {
  const isDir = (mode & 0o170000) === 0o040000;
  const perms = ((mode & 0o777)
    .toString(8)
    .padStart(3, "0")
    .split("")
    .map((d) => {
      const n = Number.parseInt(d, 10);
      return `${n & 4 ? "r" : "-"}${n & 2 ? "w" : "-"}${n & 1 ? "x" : "-"}`;
    })
    .join(""));
  return (isDir ? "d" : "-") + perms;
}

export function runLs(path: string, ctx: ToolContext): { content: string; isError: boolean } {
  const p = resolve(ctx, path || ".");
  let st;
  try {
    st = statSync(p);
  } catch {
    return err(`error: not found: ${p}`);
  }
  if (st.isFile()) {
    const mtime = new Date(st.mtimeMs);
    const pad = (n: number) => String(n).padStart(2, "0");
    const ts = `${mtime.getFullYear()}-${pad(mtime.getMonth() + 1)}-${pad(mtime.getDate())} ${pad(mtime.getHours())}:${pad(mtime.getMinutes())}`;
    const name = p.split("/").pop() ?? p;
    return ok(`${fileMode(st.mode)} ${fmtSize(st.size)} ${ts} ${name}`);
  }
  let entries: string[];
  try {
    entries = readdirSync(p);
  } catch (e) {
    return err(`error: ${(e as Error).message}`);
  }
  const rows = entries.map((name) => {
    let est;
    try {
      est = statSync(join(p, name));
    } catch {
      return `? ${name}`;
    }
    const isDir = est.isDirectory();
    const size = isDir ? "" : fmtSize(est.size).padStart(7);
    return `${isDir ? "d" : "-"} ${size} ${name}${isDir ? "/" : ""}`;
  });
  const dirsFirst = rows.sort((a, b) => {
    const da = a.startsWith("d") ? 0 : 1;
    const db = b.startsWith("d") ? 0 : 1;
    return da - db || a.slice(2).toLowerCase().localeCompare(b.slice(2).toLowerCase());
  });
  return ok([`${p}/`, ...dirsFirst].join("\n"));
}

register({
  name: "ls",
  description: "List a directory (or stat a single file). Defaults to cwd.",
  params: {
    type: "object",
    properties: { path: { type: "string", description: "Directory or file path." } },
  },
  read_only: true,
  run: (a, ctx) => runLs(String(a.path ?? "."), ctx),
});
