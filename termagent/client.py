from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Iterable, Optional

import httpx


@dataclass
class Delta:
    content: str = ""
    tool_calls: list[dict] = None  # accumulated tool calls


class APIError(Exception):
    pass


class Client:
    def __init__(self, api_key: str, base_url: str, model: str,
                 extra_headers: Optional[dict] = None, timeout: float = 120.0):
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.model = model
        self.extra_headers = extra_headers or {}
        self.timeout = timeout

    def _headers(self) -> dict:
        h = {"content-type": "application/json", "accept": "text/event-stream"}
        if self.api_key:
            h["authorization"] = f"Bearer {self.api_key}"
        h.update(self.extra_headers)
        return h

    def stream_chat(self, messages: list[dict], tools: list[dict], *,
                    temperature: float = 0.2) -> Iterable[Delta]:
        url = f"{self.base_url}/chat/completions"
        payload = {
            "model": self.model,
            "messages": messages,
            "stream": True,
            "stream_options": {"include_usage": False},
            "temperature": temperature,
        }
        if tools:
            payload["tools"] = tools
            payload["tool_choice"] = "auto"

        acc_content: list[str] = []
        acc_tools: dict[int, dict] = {}

        with httpx.Client(timeout=httpx.Timeout(self.timeout, connect=30.0)) as c:
            with c.stream("POST", url, json=payload, headers=self._headers()) as resp:
                if resp.status_code != 200:
                    body = resp.read().decode("utf-8", "replace")
                    raise APIError(f"HTTP {resp.status_code}: {_trim(body, 800)}")
                for line in resp.iter_lines():
                    if not line:
                        continue
                    if line.startswith("data: "):
                        line = line[6:]
                    if line.strip() == "[DONE]":
                        break
                    if not line.startswith("{"):
                        continue
                    try:
                        ev = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if ev.get("error"):
                        raise APIError(json.dumps(ev["error"]))
                    choice = (ev.get("choices") or [{}])[0]
                    delta = choice.get("delta") or {}
                    if not delta and choice.get("message"):
                        delta = choice["message"]

                    piece = delta.get("content")
                    if piece:
                        acc_content.append(piece)
                        yield Delta(content=piece)

                    for tc in delta.get("tool_calls") or []:
                        idx = tc.get("index", 0)
                        slot = acc_tools.setdefault(idx, {
                            "id": tc.get("id", ""),
                            "type": "function",
                            "function": {"name": "", "arguments": ""},
                        })
                        if tc.get("id"):
                            slot["id"] = tc["id"]
                        fn = tc.get("function") or {}
                        if fn.get("name"):
                            slot["function"]["name"] = fn["name"]
                        if fn.get("arguments") is not None:
                            slot["function"]["arguments"] += fn["arguments"]

        if acc_tools:
            ordered = [acc_tools[k] for k in sorted(acc_tools)]
            yield Delta(tool_calls=ordered)
        elif "".join(acc_content):
            pass

    def complete_assistant(self, content: str, tool_calls: list[dict]) -> dict:
        msg = {"role": "assistant"}
        if content:
            msg["content"] = content
        if tool_calls:
            msg["tool_calls"] = tool_calls
            if "content" not in msg:
                msg["content"] = None
        return msg


def _trim(s: str, n: int) -> str:
    return s if len(s) <= n else s[:n] + "…"
