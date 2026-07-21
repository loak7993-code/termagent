from __future__ import annotations

from pathlib import Path

from .base import Tool, ToolContext, ToolResult, register
from .read import resolve


def run(path: str, old: str, new: str, ctx: ToolContext, replace_all: bool) -> ToolResult:
    if not path:
        return ToolResult("error: path is required", is_error=True)
    if old == new:
        return ToolResult("error: old and new text are identical", is_error=True)
    p = resolve(ctx, path)
    if not p.exists():
        return ToolResult(f"error: file not found: {p}", is_error=True)
    if p.is_dir():
        return ToolResult(f"error: {p} is a directory", is_error=True)
    try:
        text = p.read_text(encoding="utf-8")
    except OSError as e:
        return ToolResult(f"error: {e}", is_error=True)

    count = text.count(old)
    if count == 0:
        return ToolResult(
            "error: oldString not found. Re-read the file; it may have changed or "
            "use different whitespace/line endings.",
            is_error=True,
        )
    if count > 1 and not replace_all:
        return ToolResult(
            f"error: oldString matches {count} locations. Provide more context to "
            "make it unique, or set replace_all=true.",
            is_error=True,
        )

    if replace_all:
        new_text = text.replace(old, new)
    else:
        new_text = text.replace(old, new, 1)

    if ctx.approve is not None:
        if not ctx.approve("edit", f"{p} ({count} match{'es' if count != 1 else ''})"):
            return ToolResult("Edit not approved by user.", is_error=True)
    try:
        p.write_text(new_text, encoding="utf-8")
    except OSError as e:
        return ToolResult(f"error: {e}", is_error=True)
    if ctx.emit:
        ctx.emit(f"edit {p} ({count} replacement{'s' if count != 1 else ''})")
    return ToolResult(f"edited {p} ({count} replacement{'s' if count != 1 else ''})")


register(Tool(
    name="edit",
    description=(
        "Replace exact text in a file. oldString must be unique unless "
        "replace_all is true. Fails if oldString is absent or ambiguous."
    ),
    params={
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "File path."},
            "old": {"type": "string", "description": "Exact text to replace."},
            "new": {"type": "string", "description": "Replacement text."},
            "replace_all": {"type": "boolean", "description": "Replace all matches."},
        },
        "required": ["path", "old", "new"],
    },
    run=lambda a, ctx: run(
        a.get("path", ""), a.get("old", ""), a.get("new", ""), ctx,
        bool(a.get("replace_all", False)),
    ),
))
