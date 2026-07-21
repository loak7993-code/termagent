from __future__ import annotations

import os
import shlex
import subprocess

from .base import Tool, ToolContext, ToolResult, register


def _default_shell() -> str:
    sh = os.environ.get("SHELL") or ""
    if sh and os.path.exists(sh):
        return sh
    for cand in ("/data/data/com.termux/files/usr/bin/bash", "/bin/bash", "/bin/sh"):
        if os.path.exists(cand):
            return cand
    return "sh"


def run(command: str, ctx: ToolContext, *, timeout: int | None = 120) -> ToolResult:
    command = (command or "").strip()
    if not command:
        return ToolResult("error: empty command", is_error=True)

    if ctx.approve is not None and not ctx.approve("bash", command):
        return ToolResult("Command not approved by user.", is_error=True)

    if ctx.emit:
        ctx.emit(f"$ {command}")

    try:
        proc = subprocess.run(
            command,
            shell=True,
            executable=_default_shell(),
            cwd=ctx.cwd,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return ToolResult(f"error: command timed out after {timeout}s", is_error=True)
    except FileNotFoundError as e:
        return ToolResult(f"error: {e}", is_error=True)

    out = proc.stdout or ""
    err = proc.stderr or ""
    parts = []
    if out:
        parts.append(out.rstrip("\n"))
    if err:
        parts.append(err.rstrip("\n"))
    body = "\n".join(parts)
    if len(body) > 60000:
        body = body[:60000] + f"\n... [truncated, {len(body)} bytes total]"
    if proc.returncode != 0:
        body = body + f"\n[exit {proc.returncode}]"
        return ToolResult(body, is_error=True)
    return ToolResult(body if body else "(no output)")


PARAMS = {
    "type": "object",
    "properties": {
        "command": {"type": "string", "description": "The shell command to run."},
        "timeout": {
            "type": "integer",
            "description": "Timeout in seconds (default 120).",
        },
    },
    "required": ["command"],
}


register(Tool(
    name="bash",
    description=(
        "Run a shell command and return stdout/stderr. Use for inspection and "
        "running builds/tests. Avoid destructive commands unless the user asks."
    ),
    params=PARAMS,
    run=lambda args, ctx: run(
        args.get("command", ""),
        ctx,
        timeout=int(args.get("timeout", 120)) if args.get("timeout") else 120,
    ),
))
