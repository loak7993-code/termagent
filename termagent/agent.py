from __future__ import annotations

from . import tools
from .client import APIError, Client
from .config import Config
from .tools.base import ToolContext, parse_args


class Agent:
    def __init__(self, cfg: Config, ctx: ToolContext, *, on_text=None, on_tool_start=None,
                 on_tool_result=None, on_assistant_done=None, on_error=None, on_meta=None):
        self.cfg = cfg
        self.ctx = ctx
        self.client = Client(cfg.api_key, cfg.base_url, cfg.model, cfg.extra_headers)
        self.messages: list[dict] = []
        self.on_text = on_text or (lambda s: None)
        self.on_tool_start = on_tool_start or (lambda name, args: None)
        self.on_tool_result = on_tool_result or (lambda name, args, result, err: None)
        self.on_assistant_done = on_assistant_done or (lambda text: None)
        self.on_error = on_error or (lambda msg: None)
        self.on_meta = on_meta or (lambda **kw: None)

    def add_system(self) -> None:
        sys_text = self.cfg.system
        cwd = self.ctx.cwd
        sys_text += (
            f"\nEnvironment:\n- working directory: {cwd}\n- platform: detect via tools\n"
            "You have tools: " + ", ".join(t.name for t in tools.all_tools()) + "."
        )
        self.messages.append({"role": "system", "content": sys_text})

    def user_turn(self, text: str) -> None:
        self.messages.append({"role": "user", "content": text})
        self._run_loop()

    def _run_loop(self) -> None:
        schemas = tools.schemas()
        for turn in range(self.cfg.max_turns):
            content_parts: list[str] = []
            tool_calls: list[dict] = []
            try:
                for delta in self.client.stream_chat(self.messages, schemas):
                    if delta.content:
                        content_parts.append(delta.content)
                        self.on_text(delta.content)
                    if delta.tool_calls:
                        tool_calls = delta.tool_calls
            except APIError as e:
                self.on_error(f"API error: {e}")
                return
            except Exception as e:
                self.on_error(f"request failed: {e}")
                return

            assistant = {"role": "assistant"}
            content = "".join(content_parts)
            if content:
                assistant["content"] = content
            else:
                assistant["content"] = None
            if tool_calls:
                assistant["tool_calls"] = tool_calls
            self.messages.append(assistant)
            self.on_assistant_done(content)

            if not tool_calls:
                return

            for call in tool_calls:
                fn = (call.get("function") or {})
                name = fn.get("name", "")
                raw_args = fn.get("arguments", "")
                args = parse_args(raw_args)
                self.on_tool_start(name, args)
                result = self._exec(name, args)
                self.on_tool_result(name, args, result.content, result.is_error)
                self.messages.append({
                    "role": "tool",
                    "tool_call_id": call.get("id", name),
                    "name": name,
                    "content": result.content,
                })

        self.on_error("max turns reached; stopping to avoid runaway tool use.")

    def _exec(self, name: str, args: dict):
        tool = tools.get(name)
        if tool is None:
            return tools.base.ToolResult(f"error: unknown tool '{name}'", is_error=True)
        try:
            return tool.run(args, self.ctx)
        except Exception as e:
            return tools.base.ToolResult(f"error in tool {name}: {e}", is_error=True)
