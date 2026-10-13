from __future__ import annotations

import contextlib
import time
from typing import Optional

from rich.console import Console
from rich.live import Live
from rich.markdown import Markdown
from rich.prompt import Prompt
from rich.text import Text

from .config import Config
from .theme import Theme, get as get_theme, theme_from_env, unicode_enabled
from .tools.base import ToolContext
from .ui import UI, classify_risk, shorten, symbol

try:
    from prompt_toolkit import PromptSession
    from prompt_toolkit.formatted_text import ANSI
    from prompt_toolkit.history import InMemoryHistory
    _HAS_PTK = True
except Exception:
    _HAS_PTK = False


NO_API_KEY_NEXT_STEPS = (
    "Zen's free tier is now limited to the official OpenCode client, so this "
    "provider needs an API key (get one at opencode.ai/auth). Alternatives:\n"
    "  --config            set a key interactively\n"
    "  -b <url> -k <key>   any OpenAI-compatible endpoint (OpenRouter, Groq,\n"
    "                      local Ollama, llama.cpp, LM Studio)\n"
    "  /models             pick a model once a key is set"
)


def _ansi256(hex_color: str) -> int:
    """Nearest xterm-256 index for a #rrggbb color (for prompt_toolkit)."""
    try:
        r = int(hex_color[1:3], 16)
        g = int(hex_color[3:5], 16)
        b = int(hex_color[5:7], 16)
    except (ValueError, IndexError):
        return 7
    # grayscale ramp
    if abs(r - g) < 10 and abs(g - b) < 10:
        if r < 8:
            return 16
        if r > 248:
            return 231
        return 232 + round((r - 8) / 247 * 23)
    def idx(v: int) -> int:
        return 0 if v < 48 else 1 if v < 115 else (v - 35) // 40
    return 16 + 36 * idx(r) + 6 * idx(g) + idx(b)


class TUI:
    def __init__(self, cfg: Config, ctx: ToolContext, *, banner: bool = True):
        self.cfg = cfg
        self.ctx = ctx
        self.console = Console(highlight=False)
        self.theme: Theme = get_theme(getattr(cfg, "theme", None) or theme_from_env().name)
        self.ui = UI(self.console, self.theme)
        if banner:
            self.splash()
        self._history = InMemoryHistory() if _HAS_PTK else None
        self._ptk = PromptSession(history=self._history) if _HAS_PTK else None
        self._live: Optional[Live] = None
        self._buf: list[str] = []
        self._md_enabled = True
        self._tool_status = None
        self._reason_live: Optional[Live] = None
        self._reason_buf: list[str] = []
        self._reason_t0: Optional[float] = None
        self._label_pending = True

    # ---- startup ---------------------------------------------------------

    def splash(self) -> None:
        from . import zen
        free = zen.is_free(self.cfg.model)
        from . import __version__
        self.ui.splash(
            model=self.cfg.model,
            base=self.cfg.base_url,
            cwd=self.ctx.cwd,
            free=free,
            theme=self.theme.name,
            version=__version__,
        )

    def set_theme(self, name: str) -> None:
        self.theme = get_theme(name)
        self.ui.set_theme(self.theme)
        self.cfg.theme = self.theme.name

    # ---- input -----------------------------------------------------------

    def _prompt_text(self) -> str:
        if not unicode_enabled():
            return "> "
        if _HAS_PTK:
            p = _ansi256(self.theme.primary)
            return ANSI(f"\x1b[1;38;5;{p}m❯\x1b[0m ")
        return "❯ "

    def read_user(self) -> Optional[str]:
        cwd = shorten(self.ctx.cwd)
        self.console.print()
        self.console.print(Text(f"  {cwd}", style=self.theme.s("muted_foreground")))
        try:
            if self._ptk is not None:
                line = self._ptk.prompt(self._prompt_text(), multiline=False)
            else:
                self.console.print(
                    Text("❯ ", style=self.theme.s("primary", bold=True)), end="")
                line = input()
        except (EOFError, KeyboardInterrupt):
            self.console.print()
            return None
        if line is None:
            return None
        line = line.rstrip("\n")
        if line.strip() == "/paste":
            return self._read_multiline()
        return line

    def echo_user(self, text: str) -> None:
        """Render the user's message into the transcript (termcn ChatMessage)."""
        self.ui.chat_label("user")
        self.console.print(Text(text, style=self.theme.s("foreground")))

    def _read_multiline(self) -> str:
        self.ui.status_line("info", "paste mode — end with a line containing only .")
        lines = []
        while True:
            try:
                if self._ptk is not None:
                    s = self._ptk.prompt("… ")
                else:
                    s = input("… ")
            except (EOFError, KeyboardInterrupt):
                break
            if s.strip() == ".":
                break
            lines.append(s)
        return "\n".join(lines)

    # ---- streaming -------------------------------------------------------

    def start_stream(self) -> None:
        self._close_live()
        self._end_reasoning()
        self._buf = []
        self._label_pending = True

    def feed(self, chunk: str) -> None:
        if self._label_pending:
            self._end_reasoning()
            self.ui.chat_label("assistant")
            self._label_pending = False
        self._buf.append(chunk)
        self._ensure_live()
        self._live.update(self._render("".join(self._buf)))

    def end_stream(self) -> str:
        text = "".join(self._buf)
        self._close_live()
        if text.strip():
            self.console.print(self._render(text))
        self._buf = []
        return text

    def _ensure_live(self) -> None:
        if self._live is None:
            self._live = Live(
                Text(""),
                console=self.console,
                refresh_per_second=30,
                transient=True,
                vertical_overflow="visible",
            )
            self._live.start()

    def _close_live(self) -> None:
        if self._live is not None:
            self._live.stop()
            self._live = None

    def _render(self, text: str):
        if self._md_enabled:
            try:
                return Markdown(text, code_theme="ansi_dark")
            except Exception:
                return Text(text)
        return Text(text)

    # ---- reasoning (termcn ThinkingBlock) ---------------------------------

    def feed_reasoning(self, chunk: str) -> None:
        if self._reason_live is None:
            if not self.console.is_terminal:
                return
            self._reason_t0 = time.monotonic()
            self._reason_buf = []
            self._reason_live = Live(
                Text("✻ thinking…", style=self.theme.s("accent", dim=True)),
                console=self.console,
                refresh_per_second=10,
                transient=True,
            )
            self._reason_live.start()
        self._reason_buf.append(chunk)
        self._reason_live.update(Text(
            "✻ thinking… " + _inline("".join(self._reason_buf), 96),
            style=self.theme.s("muted_foreground", dim=True),
        ))

    def _end_reasoning(self) -> None:
        if self._reason_live is None:
            return
        elapsed = time.monotonic() - (self._reason_t0 or time.monotonic())
        self._reason_live.stop()
        self._reason_live = None
        if self.cfg.show_reasoning and self._reason_buf:
            self.ui.chat_label("system")
            self.console.print(Text(
                "".join(self._reason_buf), style=self.theme.s("muted_foreground")))
        elif elapsed > 0.05:
            self.ui.thinking_note(elapsed)
        self._reason_buf = []
        self._reason_t0 = None

    # ---- meta -------------------------------------------------------------

    def turn_meta(self, *, prompt_tokens: int = 0, completion_tokens: int = 0,
                  model: str = "", elapsed_ms: int = 0) -> None:
        if not self.console.is_terminal:
            return
        if not prompt_tokens and not completion_tokens:
            return
        self.ui.token_usage(prompt_tokens, completion_tokens,
                            model=model, elapsed_s=elapsed_ms / 1000.0)

    # ---- tool output ------------------------------------------------------

    def tool_start(self, name: str, args: dict) -> None:
        if self.console.is_terminal:
            self._tool_status = self.console.status(
                f"{symbol('pending')} {name}",
                spinner_style=self.theme.s("primary"),
            )
            self._tool_status.start()
        else:
            self.ui.tool_line(name, "pending", args_inline=_inline_args(args))

    def tool_result(self, name: str, args: dict, result: str, is_error: bool,
                    duration_ms: int = 0) -> None:
        self._stop_tool_status()
        self.ui.tool_line(name, "error" if is_error else "success",
                          duration_ms, args_inline=_inline_args(args))
        body = result if len(result) <= 4000 else result[:4000] + \
            f"\n… [truncated, {len(result)} bytes]"
        if not is_error and name == "edit" and args.get("old") is not None:
            from .ui import unified_diff
            diff = unified_diff(str(args.get("old", "")), str(args.get("new", "")),
                                str(args.get("path", name)))
            if diff:
                self.ui.diff(diff)
                return
        if not is_error and name == "write":
            content = str(args.get("content", ""))
            preview = "\n".join(content.splitlines()[:24])
            if len(content.splitlines()) > 24:
                preview += "\n…"
            self.ui.tool_panel(Text(preview, style=self.theme.s("muted_foreground")),
                               title=name, variant="success")
            return
        self.ui.tool_panel(Text(body, style=self.theme.s("muted_foreground")),
                           title=name, variant="error" if is_error else "success")

    def _stop_tool_status(self) -> None:
        if self._tool_status is not None:
            with contextlib.suppress(Exception):
                self._tool_status.stop()
            self._tool_status = None

    def say(self, text: str, *, style: str = "") -> None:
        self.console.print(text, style=style, markup=True, highlight=False)

    def info(self, text: str) -> None:
        self.ui.status_line("neutral", text)

    def error(self, text: str) -> None:
        self.ui.alert("error", "error", text)

    def hr(self) -> None:
        self.ui.divider()

    # ---- approval (termcn ToolApproval) ------------------------------------

    def approve(self, kind: str, detail: str) -> bool:
        if self.cfg.auto_approve:
            return True
        self._stop_tool_status()
        risk = classify_risk(kind, detail)
        self.ui.approval_panel(kind, detail, risk)
        try:
            ans = Prompt.ask(
                Text("approve?", style=self.theme.s("foreground", bold=True)),
                choices=["y", "n", "a"],
                default="n",
                show_default=False,
                console=self.console,
            )
        except (EOFError, KeyboardInterrupt):
            self.console.print()
            return False
        ans = ans.strip().lower()
        if ans == "a":
            self.cfg.auto_approve = True
            self.ui.status_line("warning", "auto-approve enabled for this session")
            return True
        return ans in ("y", "yes")


def _inline(s: str, limit: int) -> str:
    s = s.replace("\n", " ⏎ ").strip()
    return s if len(s) <= limit else s[:limit] + "…"


def _inline_args(args: dict) -> str:
    if not args:
        return ""
    parts = []
    for k, v in args.items():
        if k == "command":
            parts.append(_inline(str(v), 120))
        elif k == "content":
            parts.append(f"<{len(str(v))} bytes>")
        elif k in ("old", "new"):
            parts.append(f"{k}={_inline(str(v), 60)}")
        elif k == "todos":
            parts.append(f"todos=[{len(v) if isinstance(v, list) else 1}]")
        else:
            parts.append(f"{k}={_inline(str(v), 60)}")
    return " ".join(parts)
