from __future__ import annotations

import re
import shutil
import subprocess
from pathlib import Path

from .base import Tool, ToolContext, ToolResult, register
from .read import resolve


def run_python(pattern: str, path: str, include: str, ctx: ToolContext, max_hits: int) -> ToolResult:
    base = resolve(ctx, path or ".")
    if not base.exists():
        return ToolResult(f"error: not found: {base}", is_error=True)
    root = base if base.is_dir() else base.parent
    rx = re.compile(pattern)
    out: list[str] = []
    count = 0
    for p in root.rglob("*"):
        if not p.is_file():
            continue
        if p.name.startswith(".git"):
            continue
        if include and not _match_include(p.name, include):
            continue
        try:
            text = p.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        rel = str(p.relative_to(Path(ctx.cwd))) if p.is_relative_to(Path(ctx.cwd)) else str(p)
        for i, line in enumerate(text.splitlines(), 1):
            if rx.search(line):
                snippet = line if len(line) <= 500 else line[:500] + " …"
                out.append(f"{rel}:{i}: {snippet}")
                count += 1
                if count >= max_hits:
                    out.append(f"… [truncated at {max_hits} matches]")
                    return ToolResult("\n".join(out))
    return ToolResult("\n".join(out) or "(no matches)")


def _match_include(name: str, include: str) -> bool:
    import fnmatch
    for pat in include.split(","):
        pat = pat.strip()
        if not pat:
            continue
        if fnmatch.fnmatch(name, pat):
            return True
    return False


def run(pattern: str, path: str, include: str, ctx: ToolContext) -> ToolResult:
    if not pattern:
        return ToolResult("error: pattern is required", is_error=True)
    base = resolve(ctx, path or ".")
    max_hits = 500
    rg = shutil.which("rg")
    if rg:
        cmd = [rg, "-n", "--no-heading", "--color=never", "--max-count", str(max_hits)]
        if include:
            for pat in include.split(","):
                cmd += ["-g", pat.strip()]
        cmd += ["--", pattern, str(base)]
        try:
            proc = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
            body = (proc.stdout or "").rstrip("\n")
            if not body:
                return ToolResult("(no matches)")
            return ToolResult(body)
        except Exception:
            pass
    return run_python(pattern, path, include, ctx, max_hits)


register(Tool(
    name="grep",
    description=(
        "Search file contents with a regex. Returns path:line:match. "
        "Uses ripgrep if available, else a pure-Python fallback."
    ),
    params={
        "type": "object",
        "properties": {
            "pattern": {"type": "string", "description": "Regex pattern."},
            "path": {"type": "string", "description": "File or dir to search (default cwd)."},
            "include": {"type": "string", "description": "Comma-separated glob filter e.g. *.py,*.ts"},
        },
        "required": ["pattern"],
    },
    read_only=True,
    run=lambda a, ctx: run(a.get("pattern", ""), a.get("path", ""), a.get("include", ""), ctx),
))
