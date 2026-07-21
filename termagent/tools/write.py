from __future__ import annotations

from pathlib import Path

from .base import Tool, ToolContext, ToolResult, register
from .read import resolve


def run(path: str, content: str, ctx: ToolContext) -> ToolResult:
    if not path:
        return ToolResult("error: path is required", is_error=True)
    if content is None:
        content = ""
    p = resolve(ctx, path)
    if ctx.approve is not None:
        preview = content[:400] + (" …" if len(content) > 400 else "")
        if not ctx.approve("write", f"{p}\n--- preview ---\n{preview}"):
            return ToolResult("Write not approved by user.", is_error=True)
    try:
        p.parent.mkdir(parents=True, exist_ok=True)
        existed = p.exists()
        p.write_text(content, encoding="utf-8")
    except OSError as e:
        return ToolResult(f"error: {e}", is_error=True)
    action = "updated" if existed else "created"
    if ctx.emit:
        ctx.emit(f"write {action} {p} ({len(content)} bytes)")
    return ToolResult(f"{action} {p} ({len(content)} bytes)")


register(Tool(
    name="write",
    description=(
        "Write text to a file (overwrites if it exists). Reads the existing "
        "file first before deciding to overwrite."
    ),
    params={
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "File path."},
            "content": {"type": "string", "description": "Full file contents to write."},
        },
        "required": ["path", "content"],
    },
    run=lambda a, ctx: run(a.get("path", ""), a.get("content", ""), ctx),
))
