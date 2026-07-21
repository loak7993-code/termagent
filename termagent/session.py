from __future__ import annotations

import json
import time
import uuid
from pathlib import Path

from .config import config_dir


def sessions_root() -> Path:
    return config_dir() / "sessions"


def new_session_id() -> str:
    return time.strftime("%Y%m%d-%H%M%S-") + uuid.uuid4().hex[:6]


def session_path(session_id: str) -> Path:
    return sessions_root() / f"{session_id}.jsonl"


def list_sessions() -> list[tuple[str, str]]:
    root = sessions_root()
    if not root.exists():
        return []
    out = []
    for p in sorted(root.glob("*.jsonl"), reverse=True):
        try:
            first = json.loads(p.read_text(encoding="utf-8").splitlines()[0])
        except Exception:
            first = {}
        out.append((p.stem, first.get("preview", "(no preview)")))
    return out


def save_message(session_id: str, role: str, content, **extra) -> None:
    root = sessions_root()
    root.mkdir(parents=True, exist_ok=True)
    path = session_path(session_id)
    preview = ""
    if isinstance(content, str):
        preview = content.replace("\n", " ")[:120]
    elif isinstance(content, list):
        for c in content:
            if isinstance(c, dict) and c.get("type") == "text":
                preview = c.get("text", "")[:120]
                break
    rec = {"role": role, "content": content, "ts": time.time(), **extra}
    line = json.dumps(rec, ensure_ascii=False)
    if not path.exists():
        meta = json.dumps({"preview": preview}, ensure_ascii=False)
        path.write_text(meta + "\n", encoding="utf-8")
    with path.open("a", encoding="utf-8") as f:
        f.write(line + "\n")


def load_session(session_id: str) -> list[dict]:
    path = session_path(session_id)
    if not path.exists():
        return []
    msgs = []
    for i, line in enumerate(path.read_text(encoding="utf-8").splitlines()):
        if i == 0:
            continue
        try:
            rec = json.loads(line)
        except json.JSONDecodeError:
            continue
        msgs.append({"role": rec["role"], "content": rec["content"]})
    return msgs
