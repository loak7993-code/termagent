from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path
from typing import Optional

from . import session, tools
from . import zen
from .agent import Agent
from .config import Config, default_system_prompt, load, save
from .theme import THEMES, DEFAULT_THEME, get as get_theme
from .tools.base import ToolContext
from .tui import NO_API_KEY_NEXT_STEPS, TUI


def build_parser() -> argparse.ArgumentParser:
    from . import __version__
    p = argparse.ArgumentParser(
        prog="termagent",
        description="A terminal AI coding agent (opencode/codex/claude-code style) for Termux.",
    )
    p.add_argument("prompt", nargs="*", help="If given, run one shot and exit.")
    p.add_argument("-m", "--model", help="Override the model.")
    p.add_argument("-b", "--base-url", help="Override the API base URL.")
    p.add_argument("-k", "--api-key", help="Override the API key.")
    p.add_argument("-c", "--cwd", default=os.getcwd(), help="Working directory.")
    p.add_argument("-a", "--auto-approve", action="store_true", help="Auto-approve tool calls.")
    p.add_argument("-s", "--session", help="Resume a saved session id.")
    p.add_argument("--theme", help=f"UI theme (termcn theme; default {DEFAULT_THEME}).")
    p.add_argument("--list-themes", action="store_true", help="List available themes and exit.")
    p.add_argument("--config", action="store_true", help="Configure interactively and exit.")
    p.add_argument("--max-turns", type=int, help="Max agent turns per user message.")
    p.add_argument("--version", action="version", version=f"termagent {__version__}")
    return p


def make_ctx(cfg: Config, cwd: str, tui: TUI) -> ToolContext:
    return ToolContext(
        cwd=cwd,
        approve=tui.approve,
        emit=lambda s: tui.say(f"[dim]{s}[/]"),
    )


def setup_agent(cfg: Config, tui: TUI, cwd: str) -> Agent:
    ctx = make_ctx(cfg, cwd, tui)
    agent = Agent(
        cfg, ctx,
        on_text=tui.feed,
        on_reasoning=tui.feed_reasoning,
        on_tool_start=tui.tool_start,
        on_tool_result=tui.tool_result,
        on_assistant_done=lambda text: tui.end_stream(),
        on_error=tui.error,
        on_meta=lambda **kw: tui.turn_meta(**kw),
    )
    agent.add_system()
    return agent


def ensure_model_available(cfg: Config, tui: Optional[TUI]) -> None:
    """Auto-heal stale Zen model ids against Zen's live /models list.

    Custom endpoints are never second-guessed: the user chose that model id
    explicitly, and /models there may not match this catalog at all.
    """
    if "opencode.ai/zen" not in cfg.base_url.lower():
        return
    available = zen.fetch_available(cfg.base_url, cfg.api_key)
    if cfg.model in available or not available:
        return
    free = [m for m in available if zen.is_free(m)]
    pick = (free or available)[0]
    old = cfg.model
    cfg.model = pick
    try:
        save(cfg)
    except Exception:
        pass
    msg = f"model {old} is not available; switched to {pick}"
    if tui is not None:
        tui.ui.status_line("warning", msg)
    else:
        print(f"note: {msg}", file=sys.stderr)


def interactive(cfg: Config, cwd: str, resume: Optional[str]) -> int:
    tui = TUI(cfg, ToolContext(cwd=cwd))
    if not cfg.api_key and not _is_no_auth(cfg):
        tui.ui.alert("warning", "no API key", NO_API_KEY_NEXT_STEPS)
    ensure_model_available(cfg, tui)

    agent = setup_agent(cfg, tui, cwd)
    session_id = resume or session.new_session_id()
    if resume:
        for m in session.load_session(resume):
            agent.messages.append(m)
        tui.ui.status_line("info", f"resumed session {resume}")

    while True:
        line = tui.read_user()
        if line is None:
            tui.ui.status_line("neutral", "bye")
            return 0
        if not line.strip():
            continue

        if line.startswith("/"):
            handled = handle_slash(line, cfg, tui, agent, cwd)
            if handled == "exit":
                return 0
            if handled == "newagent":
                agent = setup_agent(cfg, tui, cwd)
            continue

        tui.echo_user(line)
        session.save_message(session_id, "user", line)
        tui.start_stream()
        agent.user_turn(line)
        last = agent.messages[-1] if agent.messages else None
        if last and last.get("role") == "assistant" and last.get("content"):
            session.save_message(session_id, "assistant", last["content"])


def _set_model(cfg: Config, model_id: str, agent: Agent) -> None:
    cfg.model = model_id
    if getattr(agent, "client", None) is not None:
        agent.client.model = model_id


def cmd_models(cfg: Config, tui: TUI, agent: Agent) -> Optional[str]:
    from .config import save

    with tui.ui.live_fetch("fetching models"):
        available = zen.fetch_available(cfg.base_url, cfg.api_key)
    avail_set = set(available)

    free = [m for m in zen.catalog() if m.free and m.id in avail_set]
    paid = [m for m in zen.catalog() if not m.free and m.id in avail_set]
    if not free and not paid:
        tui.error("no models returned by provider; check base_url.")
        return None

    rows: list[tuple[int, zen.ZenModel]] = []
    groups = []
    idx = 1
    free_rows = []
    for m in free:
        free_rows.append((idx, m.id, m.label, m.id == cfg.model))
        rows.append((idx, m))
        idx += 1
    if free_rows:
        groups.append(("Free · no auth needed", free_rows))
    if paid:
        paid_rows = []
        for m in paid:
            paid_rows.append((idx, m.id, m.label, m.id == cfg.model))
            rows.append((idx, m))
            idx += 1
        groups.append(("Paid · API key / credits", paid_rows))

    tui.ui.model_selector(groups, current=cfg.model)

    from rich.prompt import Prompt

    sel = Prompt.ask(
        "pick a model (number or id, blank to cancel)",
        default="",
        console=tui.console,
    ).strip()
    if not sel:
        return None

    chosen = None
    if sel.isdigit():
        n = int(sel)
        for num, m in rows:
            if num == n:
                chosen = m
                break
    if chosen is None and sel in avail_set:
        chosen = zen.ZenModel(sel, sel, zen.is_free(sel))
    if chosen is None:
        # Allow arbitrary id (e.g. user knows a model not in catalog) if it looks valid.
        if sel and "/" not in sel and " " not in sel:
            chosen = zen.ZenModel(sel, sel, False)
        else:
            tui.error(f"not a valid selection: {sel}")
            return None

    _set_model(cfg, chosen.id, agent)
    try:
        save(cfg)
    except Exception:
        pass
    tui.ui.status_line("success", f"switched to {chosen.id}" +
                       (" (free)" if chosen.free else ""))
    return None


def cmd_theme(cfg: Config, tui: TUI, arg: str) -> None:
    if not arg:
        current = cfg.theme
        for name in sorted(THEMES):
            mark = "  ✓ current" if name == current else (
                "  (default)" if name == DEFAULT_THEME else "")
            tui.console.print(
                _theme_swatch(THEMES[name]) + f" {name}{mark}"
            )
        tui.ui.status_line("info", "/theme <name> to switch")
        return
    if arg not in THEMES:
        tui.error(f"unknown theme '{arg}'. Try: {', '.join(sorted(THEMES))}")
        return
    tui.set_theme(arg)
    try:
        save(cfg)
    except Exception:
        pass
    tui.ui.status_line("success", f"theme set to {arg} (saved)")


def _theme_swatch(theme) -> str:
    def sw(hexcolor: str) -> str:
        return f"[on {hexcolor}]  [/]"
    return (sw(theme.primary) + sw(theme.success) + sw(theme.warning)
            + sw(theme.error) + sw(theme.info))


def handle_slash(line: str, cfg: Config, tui: TUI, agent: Agent, cwd: str) -> Optional[str]:
    parts = line.strip().split(maxsplit=1)
    cmd = parts[0].lower()
    arg = parts[1].strip() if len(parts) > 1 else ""

    if cmd in ("/exit", "/quit", "/q"):
        return "exit"
    if cmd == "/help":
        from . import __version__
        tui.ui.help_screen(__version__)
        return None
    if cmd == "/clear":
        agent.messages = []
        agent.add_system()
        tools.todo._state.pop(cwd, None)
        tui.ui.status_line("success", "conversation cleared")
        return "newagent"
    if cmd == "/model":
        if arg:
            _set_model(cfg, arg, agent)
            tui.ui.status_line("success", f"model set to {cfg.model}")
        else:
            tui.ui.kv([("model", cfg.model)])
        return None
    if cmd == "/models":
        return cmd_models(cfg, tui, agent)
    if cmd == "/theme":
        cmd_theme(cfg, tui, arg)
        return None
    if cmd == "/config":
        tui.ui.kv(list(cfg.to_dict().items()), title="config")
        return None
    if cmd == "/approve":
        if arg.lower() in ("on", "true", "1", "yes"):
            cfg.auto_approve = True
        elif arg.lower() in ("off", "false", "0", "no"):
            cfg.auto_approve = False
        else:
            cfg.auto_approve = not cfg.auto_approve
        state = "on" if cfg.auto_approve else "off"
        tui.ui.status_line("success" if cfg.auto_approve else "neutral",
                           f"auto-approve {state}")
        return None
    if cmd == "/tools":
        table_rows = []
        for t in tools.all_tools():
            badge = "  [read-only]" if t.read_only else ""
            table_rows.append((t.name + badge, t.description))
        tui.ui.kv(table_rows, title="tools")
        return None
    if cmd == "/sys":
        if not arg:
            tui.ui.status_line("info", "usage: /sys <text to append to system message>")
            return None
        if agent.messages and agent.messages[0]["role"] == "system":
            agent.messages[0]["content"] += "\n" + arg
            tui.ui.status_line("success", "appended to system message")
        return None
    if cmd == "/cwd":
        if arg:
            try:
                p = Path(arg).expanduser().resolve()
                if not p.is_dir():
                    tui.error(f"not a directory: {p}")
                    return None
                tui.ctx.cwd = str(p)
                tui.ui.status_line("success", f"cwd → {p}")
            except Exception as e:
                tui.error(str(e))
        else:
            tui.ui.kv([("cwd", tui.ctx.cwd)])
        return None
    if cmd == "/sessions":
        rows = session.list_sessions()
        if not rows:
            tui.ui.status_line("info", "no saved sessions — /save to create one")
        else:
            tui.ui.kv([(sid, prev) for sid, prev in rows], title="sessions")
        return None
    if cmd == "/save":
        from . import session as sess
        sid = arg or sess.new_session_id()
        path = sess.session_path(sid)
        if path.exists():
            tui.error(f"session {sid} exists; choose a new id")
            return None
        saved = 0
        for m in agent.messages:
            if m["role"] == "system":
                continue
            content = m.get("content", "") or ""
            sess.save_message(sid, m["role"], content)
            saved += 1
        tui.ui.status_line("success", f"saved session {sid} ({saved} messages)")
        return None
    if cmd == "/load":
        if not arg:
            tui.ui.status_line("info", "usage: /load <session-id>")
            return None
        msgs = session.load_session(arg)
        if not msgs:
            tui.error(f"no session {arg}")
            return None
        agent.messages = [m for m in agent.messages if m["role"] == "system"]
        agent.messages.extend(msgs)
        tui.ui.status_line("success", f"loaded {len(msgs)} messages from {arg}")
        return None
    tui.error(f"unknown command: {cmd} (try /help)")
    return None


def oneshot(cfg: Config, cwd: str, prompt: str) -> int:
    if not cfg.api_key and not _is_no_auth(cfg):
        print("error: no API key. Set OPENAI_API_KEY, run --config, or use a free Zen model.", file=sys.stderr)
        return 2
    ctx = ToolContext(cwd=cwd, approve=lambda k, d: cfg.auto_approve)
    tui = TUI(cfg, ctx, banner=False) if sys.stdout.isatty() else None
    ctx.emit = lambda s: (tui.say(f"[dim]{s}[/]") if tui else None)

    agent = Agent(
        cfg, ctx,
        on_text=(tui.feed if tui else lambda s: sys.stdout.write(s)),
        on_reasoning=(tui.feed_reasoning if tui else lambda s: None),
        on_tool_start=(tui.tool_start if tui else lambda n, a: None),
        on_tool_result=(tui.tool_result if tui else lambda n, a, r, e, d: None),
        on_assistant_done=(lambda text: (tui.end_stream() if tui else print())),
        on_error=(tui.error if tui else lambda s: print(s, file=sys.stderr)),
        on_meta=(lambda **kw: tui.turn_meta(**kw) if tui else None),
    )
    agent.add_system()
    if tui:
        tui.start_stream()
    agent.user_turn(prompt)
    if tui:
        tui.end_stream()
    else:
        sys.stdout.write("\n")
    return 0


def configure_interactive() -> int:
    from rich.console import Console
    from rich.prompt import Prompt

    from .config import config_path
    from .theme import names as theme_names

    cfg = load()
    console = Console(highlight=False)
    console.print("[bold]termagent config[/]")
    cfg.api_key = Prompt.ask("API key (blank keeps current)", default=cfg.api_key, password=True, show_default=False)
    cfg.base_url = Prompt.ask("Base URL", default=cfg.base_url)
    cfg.model = Prompt.ask("Model", default=cfg.model)
    theme = Prompt.ask("Theme", default=cfg.theme, choices=sorted(theme_names()))
    cfg.theme = theme
    val = Prompt.ask("Max turns", default=str(cfg.max_turns))
    try:
        cfg.max_turns = int(val)
    except ValueError:
        pass
    auto = Prompt.ask("Auto-approve tool calls? (y/N)", default="n")
    cfg.auto_approve = auto.lower().startswith("y")
    cfg.system = cfg.system or default_system_prompt()
    save(cfg)
    console.print(f"[green]saved to {config_path()}[/]")
    return 0


def list_themes(console) -> int:
    for name in sorted(THEMES):
        t = THEMES[name]
        mark = "  ← default" if name == DEFAULT_THEME else ""
        console.print(_theme_swatch(t) + f" {name}{mark}")
    return 0


def main() -> int:
    args = build_parser().parse_args()
    cfg = load()
    if args.model:
        cfg.model = args.model
    if args.base_url:
        cfg.base_url = args.base_url
    if args.api_key:
        cfg.api_key = args.api_key
    if args.max_turns:
        cfg.max_turns = args.max_turns
    if args.auto_approve:
        cfg.auto_approve = True
    if args.theme:
        cfg.theme = args.theme

    if args.list_themes:
        from rich.console import Console
        return list_themes(Console(highlight=False))

    # Validate the theme early so typos surface before the first render.
    cfg.theme = get_theme(cfg.theme).name

    if args.config:
        return configure_interactive()

    if not args.prompt:
        return interactive(cfg, args.cwd, args.session)

    prompt = " ".join(args.prompt)
    return oneshot(cfg, args.cwd, prompt)


def _is_no_auth(cfg: Config) -> bool:
    """True when the configured provider doesn't require an API key (e.g. Zen free)."""
    return "opencode.ai/zen" in cfg.base_url.lower()


if __name__ == "__main__":
    raise SystemExit(main())
