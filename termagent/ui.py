"""Terminal UI components ported from termcn (https://termcn.dev).

termcn is a registry of Ink/OpenTUI components with a shared theme contract
spinners). This module reimplements the components termagent needs — badge,
alert, status message, spinner, tool call, tool approval, model selector,
token usage, thinking block, chat message, diff view, help screen, splash
screen — on top of rich, using the same visual language and the same theme
palettes. No new dependencies; everything renders with plain rich.
"""

from __future__ import annotations

import contextlib

import difflib
import re
from typing import Optional, Sequence

from rich.box import ASCII, ROUNDED
from rich.console import Console, Group, RenderableType
from rich.markdown import Markdown
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

from .theme import Theme, unicode_enabled

# termcn terminal-symbols: unicode and ascii status symbol tables.
_STATUS_UNICODE = {
    "error": "✗",
    "info": "ℹ",
    "neutral": "·",
    "pending": "○",
    "success": "✓",
    "warning": "⚠",
}
_STATUS_ASCII = {
    "error": "x",
    "info": "i",
    "neutral": "-",
    "pending": "o",
    "success": "v",
    "warning": "!",
}

VARIANT_COLORS = {
    "default": "primary",
    "success": "success",
    "warning": "warning",
    "error": "error",
    "info": "info",
    "secondary": "muted_foreground",
}

_DANGEROUS = re.compile(
    r"\b(rm\s+(-[a-z]*r[a-z]*f|-[a-z]*f[a-z]*r)|mkfs|dd\s+if=|shutdown|reboot|"
    r"init\s+0|:\(\)\{.*\};:|chmod\s+-R\s+777\s+/|git\s+push\s+--force|"
    r">\s*/dev/[sh]d|mkswap|format)\b"
)
_REMOTE_SCRIPT = re.compile(r"\b(curl|wget)\b[^|;]*\|\s*(sudo\s+)?(ba)?sh\b")


def symbol(status: str) -> str:
    table = _STATUS_UNICODE if unicode_enabled() else _STATUS_ASCII
    return table.get(status, table["neutral"])


def classify_risk(kind: str, detail: str) -> str:
    """Map a pending tool call to termcn ToolApproval's risk levels."""
    if kind == "bash":
        if _DANGEROUS.search(detail) or _REMOTE_SCRIPT.search(detail):
            return "high"
        return "medium"
    if kind in ("write", "edit"):
        return "medium"
    return "low"


def format_tokens(n: int) -> str:
    if n >= 1_000_000:
        return f"{n / 1_000_000:.1f}M"
    if n >= 1000:
        return f"{n / 1000:.1f}k"
    return str(n)


def shorten(cwd: str) -> str:
    home = "/"  # Termux homes are not /root; only shorten the literal home.
    try:
        import os
        home = os.path.expanduser("~")
    except Exception:
        pass
    if cwd == home:
        return "~"
    if cwd.startswith(home + "/"):
        return "~" + cwd[len(home):]
    return cwd


class UI:
    """Themed rendering surface. Holds the active termcn theme."""

    def __init__(self, console: Console, theme: Theme):
        self.console = console
        self.theme = theme

    def set_theme(self, theme: Theme) -> None:
        self.theme = theme

    # ---- primitives ------------------------------------------------------

    def _box(self):
        return ROUNDED if unicode_enabled() else ASCII

    def badge(self, text: str, variant: str = "default", *, bold: bool = True) -> Text:
        color = self.theme.s(VARIANT_COLORS.get(variant, "primary"), bold=bold)
        return Text(f" {text} ", style=color)

    def status_line(self, variant: str, text: str) -> None:
        color = VARIANT_COLORS.get(variant, "info")
        self.console.print(
            Text.assemble(
                (symbol(variant) + " ", self.theme.s(color, bold=True)),
                (text, self.theme.s(color)),
            )
        )

    def alert(self, variant: str, title: str, body: str = "") -> None:
        color = VARIANT_COLORS.get(variant, "info")
        lines: list[RenderableType] = [
            Text.assemble(
                (symbol(variant) + "  ", self.theme.s(color, bold=True)),
                (title, self.theme.s(color, bold=True)),
            )
        ]
        if body:
            lines.append(Text(body, style=self.theme.s("foreground")))
        self.console.print(
            Panel(
                Group(*lines),
                border_style=self.theme.s(color),
                box=self._box(),
                padding=(0, 1),
                expand=False,
            )
        )

    def divider(self) -> None:
        self.console.print(Text("─" * 3, style=self.theme.s("muted_foreground")))

    @contextlib.contextmanager
    def live_fetch(self, label: str):
        """termcn-style braille spinner while fetching (no-op off-tty)."""
        if self.console.is_terminal:
            with self.console.status(label, spinner_style=self.theme.s("primary")):
                yield
        else:
            yield

    def kv(self, rows: Sequence[tuple[str, str]], title: str = "") -> None:
        table = Table.grid(padding=(0, 2))
        table.add_column(style=self.theme.s("muted_foreground"), justify="right")
        table.add_column(style=self.theme.s("foreground"))
        for k, v in rows:
            table.add_row(k, v)
        if title:
            self.console.print(
                Panel(
                    table,
                    title=title,
                    title_align="left",
                    border_style=self.theme.s("border"),
                    box=self._box(),
                    padding=(0, 1),
                    expand=False,
                )
            )
        else:
            self.console.print(table)

    # ---- splash / help ---------------------------------------------------

    def splash(self, *, model: str, base: str, cwd: str, free: bool,
               theme: str, version: str) -> None:
        c = self.console
        c.print(Text.assemble(
            ("❯_", self.theme.s("primary", bold=True)),
            (" termagent", self.theme.s("foreground", bold=True)),
        ))
        c.print(Text(" terminal AI coding agent · termcn themed",
                     style=self.theme.s("muted_foreground")))
        c.print()
        status = Table.grid(padding=(0, 1))
        status.add_column(style=self.theme.s("muted_foreground"), justify="right")
        status.add_column()
        status.add_row("model", Text(model, style=self.theme.s("primary", bold=True)))
        status.add_row("provider", Text(base, style=self.theme.s("foreground")))
        status.add_row("cwd", Text(shorten(cwd), style=self.theme.s("foreground")))
        status.add_row(
            "theme",
            Text.assemble(
                self.badge(theme, "default"),
                self.badge("free tier", "success") if free else "",
            ),
        )
        c.print(status)
        c.print()
        c.print(Text.assemble(
            ("❯ ", self.theme.s("primary", bold=True)),
            ("/help", self.theme.s("info")),
            (" commands · ", self.theme.s("muted_foreground")),
            ("/models", self.theme.s("info")),
            (" switch · ", self.theme.s("muted_foreground")),
            ("Ctrl-D", self.theme.s("info")),
            (" quits", self.theme.s("muted_foreground")),
        ))

    def help_screen(self, version: str) -> None:
        c = self.console
        c.print()
        c.print(Text.assemble(
            ("termagent ", self.theme.s("primary", bold=True)),
            (version, self.theme.s("muted_foreground")),
            ("  ·  terminal AI coding agent", self.theme.s("muted_foreground")),
        ))
        c.print()

        cmds = [
            ("/help", "show this help"),
            ("/exit  /q", "quit (Ctrl-D also quits)"),
            ("/clear", "clear the conversation"),
            ("/models", "list & switch models (free Zen models need no auth)"),
            ("/model <id>", "show or set the model"),
            ("/theme [name]", "show or switch the UI theme (termcn themes)"),
            ("/config", "show current config"),
            ("/approve [on|off]", "toggle auto-approve of tool calls"),
            ("/save [id]", "save this session"),
            ("/load <id>", "load a saved session"),
            ("/sessions", "list saved sessions"),
            ("/cwd [path]", "show or change the working directory"),
            ("/tools", "list available tools"),
            ("/sys <text>", "append to the system message"),
            ("/paste", "multi-line input; end with a line containing only ."),
        ]
        table = Table.grid(padding=(0, 2))
        table.add_column(style=self.theme.s("primary", bold=True), no_wrap=True)
        table.add_column(style=self.theme.s("muted_foreground"))
        for cmd, desc in cmds:
            table.add_row(cmd, desc)
        c.print(table)
        c.print()

    # ---- chat ------------------------------------------------------------

    def chat_label(self, role: str) -> None:
        color = {"user": "primary", "assistant": "success",
                 "system": "muted_foreground", "error": "error"}.get(role, "primary")
        self.console.print(Text.assemble(
            ("● ", self.theme.s(color, bold=True)),
            (role, self.theme.s(color, bold=True)),
        ))

    def thinking_note(self, seconds: float) -> None:
        self.console.print(Text.assemble(
            ("✻ ", self.theme.s("accent")),
            (f"thought for {seconds:.1f}s", self.theme.s("muted_foreground")),
        ))

    def token_usage(self, prompt: int, completion: int, model: str = "",
                    elapsed_s: float = 0.0) -> None:
        line = Text()
        line.append("⟨ ", style=self.theme.s("muted_foreground"))
        line.append(format_tokens(prompt), style=self.theme.s("primary", bold=True))
        line.append(" in / ", style=self.theme.s("muted_foreground"))
        line.append(format_tokens(completion), style=self.theme.s("info", bold=True))
        line.append(" out", style=self.theme.s("muted_foreground"))
        if model:
            line.append(f" · {model}", style=self.theme.s("muted_foreground"))
        if elapsed_s > 0:
            line.append(f" · {elapsed_s:.1f}s", style=self.theme.s("muted_foreground"))
        line.append(" ⟩", style=self.theme.s("muted_foreground"))
        self.console.print(line)

    # ---- tools -----------------------------------------------------------

    def tool_line(self, name: str, status: str, duration_ms: Optional[int] = None,
                  args_inline: str = "") -> None:
        color = {"success": "success", "error": "error",
                 "running": "primary", "pending": "muted_foreground"}.get(status, "primary")
        line = Text()
        line.append(symbol(status) + " ", style=self.theme.s(color, bold=True))
        line.append(name, style=self.theme.s(color, bold=(status != "pending")))
        if args_inline:
            line.append(" " + args_inline, style=self.theme.s("muted_foreground"))
        if duration_ms is not None:
            line.append(f" ({duration_ms}ms)", style=self.theme.s("muted_foreground"))
        self.console.print(line)

    def tool_panel(self, body: RenderableType, *, title: str,
                   variant: str) -> None:
        self.console.print(
            Panel(
                body,
                title=f"{symbol('success' if variant != 'error' else 'error')} {title}",
                title_align="left",
                border_style=self.theme.s(
                    "error" if variant == "error" else "muted_foreground"),
                box=self._box(),
                padding=(0, 1),
                expand=True,
            )
        )

    def args_block(self, args: dict) -> RenderableType:
        """termcn ToolCall args list: accent keys, dim values."""
        if not args:
            return Text("")
        table = Table.grid(padding=(0, 1))
        table.add_column(style=self.theme.s("accent"), no_wrap=True)
        table.add_column(style=self.theme.s("muted_foreground"))
        for k, v in args.items():
            table.add_row(f"{k}:", _stringify(v))
        return table

    def approval_panel(self, kind: str, detail: str, risk: str) -> None:
        risk_color = {"high": "error", "medium": "warning", "low": "success"}[risk]
        head = Text.assemble(
            ("Tool approval required", self.theme.s("foreground", bold=True)),
            ("  ", ""),
            (f"[{risk.upper()} RISK]", self.theme.s(risk_color, bold=True)),
        )
        body: list[RenderableType] = [
            head,
            Text.assemble(
                ("Tool: ", self.theme.s("muted_foreground")),
                (kind, self.theme.s("primary", bold=True)),
            ),
            Text("Arguments:", style=self.theme.s("muted_foreground")),
            Text(detail, style=self.theme.s("foreground")),
            Text(""),
            Text.assemble(
                ("[y] ", self.theme.s("success", bold=True)),
                ("Approve   ", self.theme.s("foreground")),
                ("[n] ", self.theme.s("error", bold=True)),
                ("Deny   ", self.theme.s("foreground")),
                ("[a] ", self.theme.s("warning", bold=True)),
                ("Always allow", self.theme.s("foreground")),
            ),
        ]
        self.console.print(
            Panel(
                Group(*body),
                border_style=self.theme.s(risk_color),
                box=self._box(),
                padding=(0, 1),
                expand=False,
            )
        )

    def model_selector(self, groups: Sequence[tuple[str, list[tuple[int, str, str, bool]]]],
                       current: str) -> None:
        """groups: (header, rows) where row = (num, id, label, is_current)."""
        for header, rows in groups:
            self.console.print(Text(header, style=self.theme.s("primary", bold=True)))
            for num, mid, label, is_current in rows:
                line = Text("  ")
                line.append(f"{num:>2} ", style=self.theme.s("muted_foreground"))
                line.append(f"{mid:<26}", style=self.theme.s("foreground", bold=is_current))
                line.append(f" {label}", style=self.theme.s("muted_foreground"))
                if is_current:
                    line.append(f"  {symbol('success')} current",
                                style=self.theme.s("success", bold=True))
                self.console.print(line)
            self.console.print()

    # ---- diff ------------------------------------------------------------

    def diff(self, text: str, max_lines: int = 400) -> None:
        lines = text.splitlines()
        if len(lines) > max_lines:
            lines = lines[:max_lines] + [f"… [{len(text.splitlines()) - max_lines} more lines]"]
        out = Text()
        for ln in lines:
            if ln.startswith(("--- ", "+++ ")):
                style = self.theme.s("muted_foreground", bold=True)
            elif ln.startswith("@@"):
                style = self.theme.s("info", bold=True)
            elif ln.startswith("+"):
                style = self.theme.s("success")
            elif ln.startswith("-"):
                style = self.theme.s("error")
            else:
                style = self.theme.s("muted_foreground")
            out.append(ln + "\n", style=style)
        self.console.print(out)

    def markdown(self, text: str) -> Markdown:
        return Markdown(text)


def _stringify(v: object, limit: int = 120) -> str:
    s = str(v).replace("\n", " ⏎ ")
    if len(s) > limit:
        s = s[:limit] + "…"
    return s


def unified_diff(old: str, new: str, path: str, max_lines: int = 60) -> str:
    """Unified diff used by edit/write results; rendered by UI.diff."""
    diff_lines = list(difflib.unified_diff(
        old.splitlines(), new.splitlines(),
        fromfile=f"a/{path}", tofile=f"b/{path}", lineterm="",
    ))
    if len(diff_lines) > max_lines:
        diff_lines = diff_lines[:max_lines] + ["… (diff truncated)"]
    return "\n".join(diff_lines)
