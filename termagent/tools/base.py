from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Callable, Optional


@dataclass
class Tool:
    name: str
    description: str
    params: dict  # JSON schema "parameters"
    run: Callable[[dict, "ToolContext"], "ToolResult"]
    read_only: bool = False


@dataclass
class ToolContext:
    cwd: str
    approve: Optional[Callable[[str, str], bool]] = None
    emit: Optional[Callable[[str], None]] = None


@dataclass
class ToolResult:
    content: str
    is_error: bool = False

    def to_message(self) -> str:
        return self.content


_REGISTRY: dict[str, Tool] = {}


def register(tool: Tool) -> Tool:
    _REGISTRY[tool.name] = tool
    return tool


def get(name: str) -> Optional[Tool]:
    return _REGISTRY.get(name)


def all_tools() -> list[Tool]:
    return list(_REGISTRY.values())


def schemas() -> list[dict]:
    return [
        {"type": "function", "function": {
            "name": t.name,
            "description": t.description,
            "parameters": t.params,
        }}
        for t in _REGISTRY.values()
    ]


def parse_args(raw: Any) -> dict:
    if raw is None:
        return {}
    if isinstance(raw, dict):
        return raw
    if isinstance(raw, str):
        raw = raw.strip()
        if not raw:
            return {}
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            return {"_raw": raw}
    return {}
