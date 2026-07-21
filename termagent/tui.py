from __future__ import annotations

import sys
from typing import Optional

from rich.console import Console
from rich.live import Live
from rich.markdown import Markdown
from rich.panel import Panel
from rich.prompt import Confirm, Prompt
from rich.text import Text

from .config import Config
from .tools.base import ToolContext

try:
    from prompt_toolkit import PromptSession
    from prompt_toolkit.history import InMemoryHistory
    _HAS_PTK = True
except Exception:
    _HAS_PTK = False


BANNER = r"""
 [bold cyan]termagent[/] — terminal AI coding agent
  /help for commands · Ctrl-D or /exit to quit
"""

HELP = """\
[bold]Slash commands[/]
  /help            show this help
  /exit  (or /q)   quit
  /clear           clear conversation
  /models          list & switch models (free Zen models need no auth)
  /model [name]    show or set the model
  /config          show current config
  /approve [on|off] toggle auto-approve of tool calls
  /save [id]       save this session (id auto-generated if omitted)
  /load <id>       load a saved session
  /sessions        list saved sessions
  /cwd [path]      show or change the working directory
  /tools           list available tools
  /sys <text>      append to the system message
Type /paste for multi-line input (end with a line containing only .).
"""

NO_API_KEY = (
    "[bold red]No API key configured.[/]\n"
    "Set one of: [cyan]OPENAI_API_KEY[/] or [cyan]TERMAGENT_API_KEY[/],\n"
    "or run [cyan]termagent --config[/] to set it interactively.\n"
)


class TUI:
    def __init__(self, cfg: Config, ctx: ToolContext):
        self.cfg = cfg
        self.ctx = ctx
        self.console = Console(highlight=False)
        self.console.print(BANNER, highlight=True)
        self._history = InMemoryHistory() if _HAS_PTK else None
        self._ptk = PromptSession(history=self._history) if _HAS_PTK else None
        self._live: Optional[Live] = None
        self._buf: list[str] = []
        self._md_enabled = True

    # ---- input ----
    def read_user(self) -> Optional[str]:
        cwd = self.ctx.cwd
        try:
            if self._ptk is not None:
                line = self._ptk.prompt(
                    f"\n{cwd}> ",
                    multiline=False,
                )
            else:
                line = input(f"\n{cwd}> ")
        except (EOFError, KeyboardInterrupt):
            return None
        if line is None:
            return None
        line = line.rstrip("\n")
        if line.strip() == "/paste":
            return self._read_multiline()
        return line

    def _read_multiline(self) -> str:
        self.console.print("[dim]Paste mode. End with a line containing only .[/]")
        lines = []
        while True:
            try:
                if self._ptk is not None:
                    s = self._ptk.prompt("... ")
                else:
                    s = input("... ")
            except (EOFError, KeyboardInterrupt):
                break
            if s.strip() == ".":
                break
            lines.append(s)
        return "\n".join(lines)

    # ---- streaming ----
    # Each assistant message gets its own buffer. The Live display starts lazily
    # on the first streamed token so tool-only turns don't print an empty frame,
    # and resets between turns so intermediate text isn't double-printed.
    def start_stream(self) -> None:
        if self._live is not None:
            self._live.stop()
            self._live = None
        self._buf = []

    def _ensure_live(self) -> None:
        if self._live is None:
            self._live = Live(Text(""), console=self.console, refresh_per_second=30,
                              transient=True, vertical_overflow="visible")
            self._live.start()

    def feed(self, chunk: str) -> None:
        self._buf.append(chunk)
        self._ensure_live()
        self._live.update(self._render("".join(self._buf)))

    def end_stream(self) -> str:
        text = "".join(self._buf)
        if self._live is not None:
            self._live.stop()
            self._live = None
        if text.strip():
            self.console.print(self._render(text))
        self._buf = []
        return text

    def _render(self, text: str):
        if self._md_enabled:
            try:
                return Markdown(text)
            except Exception:
                return Text(text)
        return Text(text)

    # ---- tool / status output ----
    def tool_start(self, name: str, args: dict) -> None:
        arg_str = _format_args(args)
        label = Text.assemble(("⏵ ", "bold cyan"), (name, "bold cyan"), (" ", ""), (arg_str, "cyan"))
        self.console.print(label)

    def tool_result(self, name: str, args: dict, result: str, is_error: bool) -> None:
        color = "red" if is_error else "dim"
        marker = "✗" if is_error else "✓"
        body = result if len(result) <= 4000 else result[:4000] + f"\n… [truncated, {len(result)} bytes]"
        title = f"{marker} {name}"
        self.console.print(Panel(body, title=title, title_align="left",
                                 border_style=color, expand=True))

    def say(self, text: str, *, style: str = "") -> None:
        self.console.print(text, style=style)

    def error(self, text: str) -> None:
        self.console.print(f"[bold red]error:[/] {text}")

    def hr(self) -> None:
        self.console.print("[dim]─────[/]")

    # ---- approval ----
    def approve(self, kind: str, detail: str) -> bool:
        if self.cfg.auto_approve:
            return True
        head = {"bash": "Run command?", "write": "Write file?", "edit": "Edit file?"}.get(kind, "Approve?")
        self.console.print(Panel(detail, title=head, title_align="left", border_style="yellow"))
        try:
            ans = Prompt.ask("[bold]Approve?[/] (y=yes / a=yes-to-all / N=no)", default="n", console=self.console)
        except (EOFError, KeyboardInterrupt):
            return False
        ans = ans.strip().lower()
        if ans in ("a", "all"):
            self.cfg.auto_approve = True
            self.say("[dim]auto-approve enabled for this session[/]")
            return True
        return ans in ("y", "yes")


def _format_args(args: dict) -> str:
    if not args:
        return ""
    parts = []
    for k, v in args.items():
        if k == "command":
            s = str(v).replace("\n", " ")
            parts.append(f"command={s[:160]}{'…' if len(s) > 160 else ''}")
        elif k == "content":
            parts.append(f"content=<{len(str(v))} bytes>")
        elif k == "old":
            s = str(v).replace("\n", "\\n")
            parts.append(f"old={s[:80]}{'…' if len(s) > 80 else ''}")
        elif k == "new":
            s = str(v).replace("\n", "\\n")
            parts.append(f"new={s[:80]}{'…' if len(s) > 80 else ''}")
        elif k == "todos":
            parts.append(f"todos=[{len(v) if isinstance(v, list) else 1}]")
        else:
            s = str(v).replace("\n", " ")
            parts.append(f"{k}={s[:80]}{'…' if len(s) > 80 else ''}")
    return " ".join(parts)
