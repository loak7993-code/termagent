from __future__ import annotations

import os
from pathlib import Path

from .base import Tool, ToolContext, ToolResult, register


def resolve(ctx: ToolContext, p: str) -> Path:
    path = Path(p).expanduser()
    if not path.is_absolute():
        path = Path(ctx.cwd) / path
    return path


def run(path: str, offset: int, limit: int, ctx: ToolContext) -> ToolResult:
    p = resolve(ctx, path)
    try:
        text = p.read_text(encoding="utf-8", errors="replace")
    except FileNotFoundError:
        return ToolResult(f"error: file not found: {p}", is_error=True)
    except IsADirectoryError:
        return ToolResult(f"error: {p} is a directory", is_error=True)
    except OSError as e:
        return ToolResult(f"error: {e}", is_error=True)

    lines = text.splitlines()
    start = max(1, offset)
    end = start + (limit if limit and limit > 0 else 2000) - 1
    start_idx = start - 1
    end_idx = min(end, len(lines))
    chunk = lines[start_idx:end_idx]
    out = []
    width = len(str(end_idx)) if end_idx else 1
    for i, line in enumerate(chunk, start=start):
        ln = str(i).rjust(width)
        if len(line) > 2000:
            line = line[:2000] + " …[truncated]"
        out.append(f"{ln}: {line}")
    body = "\n".join(out)
    if start_idx > 0:
        body = f"… [showing lines {start}-{end_idx} of {len(lines)}]\n" + body
    if end_idx < len(lines):
        body = body + f"\n… [{len(lines) - end_idx} more lines]"
    return ToolResult(body or "(empty file)")


register(Tool(
    name="read",
    description=(
        "Read a text file and return numbered lines. Use offset/limit to page. "
        "Fails on directories."
    ),
    params={
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "File path (relative or absolute)."},
            "offset": {"type": "integer", "description": "1-indexed line to start at (default 1)."},
            "limit": {"type": "integer", "description": "Max lines to return (default 2000)."},
        },
        "required": ["path"],
    },
    read_only=True,
    run=lambda a, ctx: run(a.get("path", ""), int(a.get("offset") or 1), int(a.get("limit") or 2000), ctx),
))
