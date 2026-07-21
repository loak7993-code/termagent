from __future__ import annotations

import fnmatch
from pathlib import Path

from .base import Tool, ToolContext, ToolResult, register
from .read import resolve


def run(pattern: str, path: str, ctx: ToolContext) -> ToolResult:
    base = resolve(ctx, path or ".")
    if not pattern:
        return ToolResult("error: pattern is required", is_error=True)
    if not base.exists():
        return ToolResult(f"error: base path not found: {base}", is_error=True)

    seg = pattern.lstrip("./")
    is_abs = pattern.startswith("/") or pattern.startswith("~")
    if is_abs:
        base = Path(pattern).expanduser().parent
        seg = Path(pattern).expanduser().name

    matches: list[Path] = []
    root = base if base.is_dir() else base.parent
    max_hits = 2000
    try:
        for p in root.rglob("*"):
            if p.name.startswith(".git"):
                continue
            rel = p.relative_to(root)
            if fnmatch.fnmatch(str(rel), seg) or fnmatch.fnmatch(p.name, seg):
                matches.append(p)
                if len(matches) >= max_hits:
                    break
    except OSError as e:
        return ToolResult(f"error: {e}", is_error=True)

    matches.sort()
    body = "\n".join(str(m) for m in matches[:max_hits])
    if len(matches) >= max_hits:
        body += f"\n… [truncated at {max_hits} matches]"
    return ToolResult(body or "(no matches)")


register(Tool(
    name="glob",
    description="Find files by glob pattern, recursively from a base path.",
    params={
        "type": "object",
        "properties": {
            "pattern": {"type": "string", "description": "Glob pattern, e.g. **/*.py"},
            "path": {"type": "string", "description": "Base directory (default cwd)."},
        },
        "required": ["pattern"],
    },
    read_only=True,
    run=lambda a, ctx: run(a.get("pattern", ""), a.get("path", ""), ctx),
))
