from __future__ import annotations

import json

from .base import Tool, ToolContext, ToolResult, register

_VALID = {"pending", "in_progress", "completed", "cancelled"}
_PRIORITY = {"high", "medium", "low"}

_state: dict[str, list[dict]] = {}


def _store(ctx: ToolContext) -> list[dict]:
    return _state.setdefault(ctx.cwd, [])


def run(action: str, todos: list | None, ctx: ToolContext) -> ToolResult:
    store = _store(ctx)
    action = (action or "").strip()
    if action == "list":
        return ToolResult(_render(store))
    if action == "set":
        if not isinstance(todos, list):
            return ToolResult("error: 'todos' must be a list", is_error=True)
        store.clear()
        for i, item in enumerate(todos):
            if not isinstance(item, dict):
                continue
            content = str(item.get("content", "")).strip()
            if not content:
                continue
            status = item.get("status", "pending")
            if status not in _VALID:
                status = "pending"
            priority = item.get("priority", "medium")
            if priority not in _PRIORITY:
                priority = "medium"
            store.append({
                "content": content,
                "status": status,
                "priority": priority,
                "order": i,
            })
        return ToolResult(_render(store))
    return ToolResult(
        "error: action must be 'list' or 'set'. Use 'set' with a full todos list "
        "(content, status, priority) to replace the whole plan.",
        is_error=True,
    )


def _render(store: list[dict]) -> str:
    if not store:
        return "(no todos)"
    sym = {
        "pending": "[ ]",
        "in_progress": "[~]",
        "completed": "[x]",
        "cancelled": "[-]",
    }
    lines = []
    for i, t in enumerate(store, 1):
        lines.append(f"{i}. {sym.get(t['status'], '[ ]')} ({t['priority']}) {t['content']}")
    return "\n".join(lines)


register(Tool(
    name="todo",
    description=(
        "Track a multi-step plan. action='set' replaces the whole list with the "
        "given todos (each: content, status in pending/in_progress/completed/"
        "cancelled, priority in high/medium/low). action='list' returns it."
    ),
    params={
        "type": "object",
        "properties": {
            "action": {"type": "string", "enum": ["list", "set"]},
            "todos": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "content": {"type": "string"},
                        "status": {"type": "string"},
                        "priority": {"type": "string"},
                    },
                    "required": ["content", "status", "priority"],
                },
            },
        },
        "required": ["action"],
    },
    read_only=True,
    run=lambda a, ctx: run(a.get("action", "list"), a.get("todos"), ctx),
))
