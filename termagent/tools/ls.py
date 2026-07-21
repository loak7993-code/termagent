from __future__ import annotations

import os
import stat
from datetime import datetime
from pathlib import Path

from .base import Tool, ToolContext, ToolResult, register
from .read import resolve


def fmt_size(n: int) -> str:
    for unit in ("B", "K", "M", "G", "T"):
        if n < 1024 or unit == "T":
            return f"{n:.0f}{unit}" if unit == "B" else f"{n:.1f}{unit}"
        n /= 1024
    return f"{n}B"


def run(path: str, ctx: ToolContext) -> ToolResult:
    p = resolve(ctx, path or ".")
    if not p.exists():
        return ToolResult(f"error: not found: {p}", is_error=True)
    if p.is_file():
        try:
            st = p.stat()
        except OSError:
            st = None
        if st:
            line = f"{stat.filemode(st.st_mode)} {fmt_size(st.st_size)} {datetime.fromtimestamp(st.st_mtime).strftime('%Y-%m-%d %H:%M')} {p.name}"
        else:
            line = p.name
        return ToolResult(line)
    try:
        entries = sorted(p.iterdir(), key=lambda x: (not x.is_dir(), x.name.lower()))
    except OSError as e:
        return ToolResult(f"error: {e}", is_error=True)
    out = [f"{p}/"]
    for e in entries:
        try:
            st = e.stat()
        except OSError:
            st = None
        if st is None:
            out.append(f"? {e.name}")
            continue
        mark = "d" if e.is_dir() else "-"
        size = "" if e.is_dir() else fmt_size(st.st_size)
        out.append(f"{mark} {size:>7} {e.name}{'/' if e.is_dir() else ''}")
    return ToolResult("\n".join(out))


register(Tool(
    name="ls",
    description="List a directory (or stat a single file). Defaults to cwd.",
    params={
        "type": "object",
        "properties": {"path": {"type": "string", "description": "Directory or file path."}},
    },
    read_only=True,
    run=lambda a, ctx: run(a.get("path", "."), ctx),
))
