// bash tool: shell execution with approval gate. Port of tools/bash.py.

import { exec } from "node:child_process";
import { existsSync } from "node:fs";

import { err, ok, register, type ToolContext } from "./base.js";

function defaultShell(): string {
  const env = process.env.SHELL ?? "";
  if (env && existsSync(env)) return env;
  const candidates = [
    "/data/data/com.termux/files/usr/bin/bash",
    "/bin/bash",
    "/bin/sh",
  ];
  for (const c of candidates) {
    if (existsSync(c)) return c;
  }
  return "sh";
}

export function runBash(
  command: string,
  ctx: ToolContext,
  timeout = 120,
): Promise<{ content: string; isError: boolean }> {
  const cmd = (command ?? "").trim();
  if (!cmd) return Promise.resolve(err("error: empty command"));

  const proceed = ctx.approve ? ctx.approve("bash", cmd) : Promise.resolve(true);

  return Promise.resolve(proceed).then((approved) => {
    if (!approved) return err("Command not approved by user.");
    if (ctx.emit) ctx.emit(`$ ${cmd}`);
    return new Promise((res) => {
      exec(cmd, { shell: defaultShell(), cwd: ctx.cwd, timeout: timeout * 1000, maxBuffer: 64 * 1024 * 1024 }, (error, stdout, stderr) => {
        if (error && error.killed) {
          res(err(`error: command timed out after ${timeout}s`));
          return;
        }
        const parts: string[] = [];
        if (stdout) parts.push(stdout.replace(/\n$/, ""));
        if (stderr) parts.push(stderr.replace(/\n$/, ""));
        let body = parts.join("\n");
        if (body.length > 60_000) body = body.slice(0, 60_000) + `\n... [truncated, ${body.length} bytes total]`;
        const code = error ? (typeof error.code === "number" ? error.code : 1) : 0;
        if (code !== 0) {
          body += `\n[exit ${code}]`;
          res({ content: body || `error: exit ${code}`, isError: true });
          return;
        }
        res(ok(body || "(no output)"));
      });
    });
  });
}

register({
  name: "bash",
  description:
    "Run a shell command in the working directory. Returns stdout+stderr. Approval required unless auto-approve is on.",
  params: {
    type: "object",
    properties: { command: { type: "string", description: "Shell command to run." } },
    required: ["command"],
  },
  read_only: false,
  run: (a, ctx) => runBash(String(a.command ?? ""), ctx),
});
