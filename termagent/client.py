from __future__ import annotations

import contextlib
import json
from dataclasses import dataclass
from typing import Iterable, Iterator, Optional

import httpx


@dataclass
class Delta:
    content: str = ""
    reasoning: str = ""
    tool_calls: Optional[list[dict]] = None


class APIError(Exception):
    pass


@dataclass
class Usage:
    prompt_tokens: int = 0
    completion_tokens: int = 0
    model: str = ""


def _friendly_error(status: int, body: str) -> str:
    if status == 403 and "FreeTierError" in body:
        return (
            "Zen's free tier is now limited to the official OpenCode client. "
            "Options: set an API key from opencode.ai/auth (use --config or -k), "
            "point -b/-k at any other OpenAI-compatible endpoint, or run a local "
            "model (e.g. Ollama at http://localhost:11434/v1)."
        )
    return f"HTTP {status}: {_trim(body, 800)}"


class Client:
    def __init__(self, api_key: str, base_url: str, model: str,
                 extra_headers: Optional[dict] = None, timeout: float = 120.0):
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.model = model
        self.extra_headers = extra_headers or {}
        self.timeout = timeout
        self.last_usage: Optional[Usage] = None
        self._skip_stream_options = False

    def _headers(self) -> dict:
        h = {"content-type": "application/json", "accept": "text/event-stream"}
        if self.api_key:
            h["authorization"] = f"Bearer {self.api_key}"
        h.update(self.extra_headers)
        return h

    def _payload(self, messages: list[dict], tools: list[dict],
                 temperature: float) -> dict:
        payload = {
            "model": self.model,
            "messages": messages,
            "stream": True,
            "temperature": temperature,
        }
        if not self._skip_stream_options:
            payload["stream_options"] = {"include_usage": True}
        if tools:
            payload["tools"] = tools
            payload["tool_choice"] = "auto"
        return payload

    def stream_chat(self, messages: list[dict], tools: list[dict], *,
                    temperature: float = 0.2) -> Iterable[Delta]:
        url = f"{self.base_url}/chat/completions"
        payload = self._payload(messages, tools, temperature)
        self.last_usage = None

        acc_content: list[str] = []
        acc_reasoning: list[str] = []
        acc_tools: dict[int, dict] = {}
        usage = Usage(model=self.model)

        with httpx.Client(timeout=httpx.Timeout(self.timeout, connect=30.0)) as c:
            with self._open_stream(c, url, payload) as resp:
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
                    u = ev.get("usage")
                    if isinstance(u, dict):
                        usage.prompt_tokens = int(u.get("prompt_tokens") or 0)
                        usage.completion_tokens = int(u.get("completion_tokens") or 0)
                    if ev.get("model"):
                        usage.model = str(ev["model"])

                    choice = (ev.get("choices") or [{}])[0]
                    delta = choice.get("delta") or {}
                    if not delta and choice.get("message"):
                        delta = choice["message"]

                    piece = delta.get("content")
                    if piece:
                        acc_content.append(piece)
                        yield Delta(content=piece)

                    reasoning = (delta.get("reasoning_content")
                                 or delta.get("reasoning") or "")
                    if reasoning:
                        acc_reasoning.append(reasoning)
                        yield Delta(reasoning=reasoning)

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

        self.last_usage = usage
        if acc_tools:
            ordered = [acc_tools[k] for k in sorted(acc_tools)]
            yield Delta(tool_calls=ordered)
        elif "".join(acc_content):
            pass

    @contextlib.contextmanager
    def _open_stream(self, client: httpx.Client, url: str,
                     payload: dict) -> Iterator[httpx.Response]:
        """POST the streaming request; degrade gracefully when a provider
        rejects stream_options (some OpenAI-compatible servers do)."""
        stream_cm = client.stream("POST", url, json=payload, headers=self._headers())
        resp = stream_cm.__enter__()
        try:
            if resp.status_code == 400 and "stream_options" in \
                    resp.read().decode("utf-8", "replace")[:2000]:
                stream_cm.__exit__(None, None, None)
                self._skip_stream_options = True
                payload.pop("stream_options", None)
                stream_cm = client.stream("POST", url, json=payload,
                                          headers=self._headers())
                resp = stream_cm.__enter__()
            if resp.status_code != 200:
                body = resp.read().decode("utf-8", "replace")
                raise APIError(_friendly_error(resp.status_code, body))
            yield resp
        finally:
            stream_cm.__exit__(None, None, None)

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
