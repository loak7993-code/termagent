from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest

from termagent.agent import Agent
from termagent.config import Config
from termagent.tools.base import ToolContext


class FakeZen(BaseHTTPRequestHandler):
    """Scripted OpenAI-compatible SSE endpoint.

    Turn 1: streams a text token, then a bash tool call (arguments split
    across chunks to exercise accumulation). Turn 2: streams the final
    answer plus a usage chunk. Can be told to 400 on stream_options to
    exercise the client's graceful fallback.
    """

    reject_stream_options = False
    with_reasoning = False

    def log_message(self, *args):  # silence request logging
        pass

    def _sse(self, events):
        body = "".join(f"data: {json.dumps(e)}\n\n" for e in events)
        body += "data: [DONE]\n\n"
        raw = body.encode()
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("content-length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        if self.path.endswith("/models"):
            payload = json.dumps({"data": [{"id": "mock-free"}]}).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        length = int(self.headers.get("content-length", 0))
        req = json.loads(self.rfile.read(length) or b"{}")
        if self.reject_stream_options and "stream_options" in req:
            body = b'{"error": {"message": "Unknown field: stream_options"}}'
            self.send_response(400)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        has_tool_result = any(m.get("role") == "tool" for m in req["messages"])
        if not has_tool_result:
            events = []
            if self.with_reasoning:
                events.append({"choices": [{"delta": {"reasoning_content": "thinking hard"}}]})
            events += [
                {"choices": [{"delta": {"content": "Running "}}]},
                {"choices": [{"delta": {"tool_calls": [
                    {"index": 0, "id": "call_1", "type": "function",
                     "function": {"name": "bash", "arguments": ""}}]}}]},
                {"choices": [{"delta": {"tool_calls": [
                    {"index": 0, "function": {"arguments": "{\"command\": "}}]}}]},
                {"choices": [{"delta": {"tool_calls": [
                    {"index": 0, "function": {"arguments": "\"echo hi\"}"}}]}}]},
            ]
            self._sse(events)
        else:
            self._sse([
                {"choices": [{"delta": {"content": "done"}}]},
                {"usage": {"prompt_tokens": 1200, "completion_tokens": 34},
                 "model": "mock-free",
                 "choices": []},
            ])


@pytest.fixture()
def fake_zen():
    server = ThreadingHTTPServer(("127.0.0.1", 0), FakeZen)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{server.server_address[1]}/v1", server
    server.shutdown()


def make_agent(base_url):
    events = {"text": [], "reasoning": [], "tools": [], "results": [],
              "meta": [], "errors": []}
    cfg = Config(api_key="", base_url=base_url, model="mock-free", max_turns=4)
    ctx = ToolContext(cwd="/tmp", approve=lambda k, d: True)
    agent = Agent(
        cfg, ctx,
        on_text=events["text"].append,
        on_reasoning=events["reasoning"].append,
        on_tool_start=lambda n, a: events["tools"].append((n, a)),
        on_tool_result=lambda n, a, r, e, d: events["results"].append((n, r, e, d)),
        on_assistant_done=lambda t: None,
        on_error=events["errors"].append,
        on_meta=lambda **kw: events["meta"].append(kw),
    )
    agent.add_system()
    return agent, events


def test_full_tool_loop_with_usage(fake_zen):
    base_url, _ = fake_zen
    agent, events = make_agent(base_url)
    agent.user_turn("run echo hi")

    assert events["errors"] == []
    assert "".join(events["text"]) == "Running done"
    assert events["tools"] == [("bash", {"command": "echo hi"})]
    name, result, is_error, duration_ms = events["results"][0]
    assert name == "bash" and "hi" in result and not is_error
    assert isinstance(duration_ms, int)
    # usage from the final chunk reached on_meta
    meta = events["meta"][-1]
    assert meta["prompt_tokens"] == 1200
    assert meta["completion_tokens"] == 34
    assert meta["model"] == "mock-free"
    assert meta["elapsed_ms"] >= 0
    # transcript assembled correctly
    roles = [m["role"] for m in agent.messages]
    assert roles == ["system", "user", "assistant", "tool", "assistant"]
    assert agent.messages[3]["tool_call_id"] == "call_1"


def test_stream_options_fallback_on_400(fake_zen):
    base_url, server = fake_zen
    server.RequestHandlerClass.reject_stream_options = True
    agent, events = make_agent(base_url)
    agent.user_turn("hello")
    assert events["errors"] == []
    assert "".join(events["text"]) == "Running done"
    assert agent.client._skip_stream_options is True
    # usage still parsed on the retried request
    assert events["meta"][-1]["completion_tokens"] == 34


def test_reasoning_deltas_forwarded(fake_zen):
    base_url, server = fake_zen
    server.RequestHandlerClass.with_reasoning = True
    agent, events = make_agent(base_url)
    agent.user_turn("go")
    assert events["errors"] == []
    assert "thinking hard" in "".join(events["reasoning"])
    assert "".join(events["text"]) == "Running done"
