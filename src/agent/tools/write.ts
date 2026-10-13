// write tool: create/overwrite a file with approval gate. Port of tools/write.py.

import { existsSync, mkdirSync, writeFileSync } from "node:fs";

import { err, ok, register, type ToolContext } from "./base.js";
import { resolve } from "./read.js";

export function runWrite(
  path: string,
  content: string,
  ctx: ToolContext,
): Promise<{ content: string; isError: boolean }> {
  if (!path) return Promise.resolve(err("error: path is required"));
  const p = resolve(ctx, path);
  const existed = existsSync(p);

  const doWrite = (): { content: string; isError: boolean } => {
    try {
      mkdirSync(p.replace(/[/\\][^/\\]*$/, ""), { recursive: true });
      writeFileSync(p, content, "utf8");
    } catch (e) {
      return err(`error: ${(e as Error).message}`);
    }
    const action = existed ? "updated" : "created";
    if (ctx.emit) ctx.emit(`write ${action} ${p} (${content.length} bytes)`);
    return ok(`${action} ${p} (${content.length} bytes)`);
  };

  if (ctx.approve) {
    const preview = content.slice(0, 400) + (content.length > 400 ? " …" : "");
    return Promise.resolve(ctx.approve("write", `${p}\n--- preview ---\n${preview}`)).then(
      (approved) => {
        if (!approved) return err("Write not approved by user.");
        return doWrite();
      },
    );
  }
  return Promise.resolve(doWrite());
}

register({
  name: "write",
  description:
    "Write text to a file (overwrites if it exists). Reads the existing file first before deciding to overwrite.",
  params: {
    type: "object",
    properties: {
      path: { type: "string", description: "File path." },
      content: { type: "string", description: "Full file contents to write." },
    },
    required: ["path", "content"],
  },
  read_only: false,
  run: (a, ctx) => runWrite(String(a.path ?? ""), String(a.content ?? ""), ctx),
});
