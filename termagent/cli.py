from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from . import session, tools
from . import zen
from .agent import Agent
from .config import Config, default_system_prompt, load, save
from .tools.base import ToolContext
from .tui import HELP, TUI


def build_parser() -> argparse.ArgumentParser:
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
    p.add_argument("--config", action="store_true", help="Configure interactively and exit.")
    p.add_argument("--max-turns", type=int, help="Max agent turns per user message.")
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
        on_tool_start=tui.tool_start,
        on_tool_result=tui.tool_result,
        on_assistant_done=lambda text: tui.end_stream(),
        on_error=tui.error,
    )
    agent.add_system()
    return agent


def interactive(cfg: Config, cwd: str, resume: str | None) -> int:
    tui = TUI(cfg, ToolContext(cwd=cwd))
    if not cfg.api_key and not _is_no_auth(cfg):
        tui.say(HELP)
        tui.error("No API key set and provider requires auth. Run /models to pick a free Zen model, set OPENAI_API_KEY, or run --config.")
    tui.say(f"[dim]model: {cfg.model}  base: {cfg.base_url}  cwd: {cwd}[/]")

    agent = setup_agent(cfg, tui, cwd)
    session_id = resume or session.new_session_id()
    if resume:
        for m in session.load_session(resume):
            agent.messages.append(m)
        tui.say(f"[dim]resumed session {resume}[/]")

    while True:
        line = tui.read_user()
        if line is None:
            tui.say("[dim]bye[/]")
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


def cmd_models(cfg: Config, tui: TUI, agent: Agent) -> str | None:
    from rich.prompt import Prompt

    from .config import save

    tui.say("[dim]fetching model list from OpenCode Zen…[/]")
    available = zen.fetch_available(cfg.base_url, cfg.api_key)
    avail_set = set(available)

    # Free models first, then paid; only show chat/completions models the client supports.
    free = [m for m in zen.catalog() if m.free and m.id in avail_set]
    paid = [m for m in zen.catalog() if not m.free and m.id in avail_set]
    # Also surface any catalog entries that the live endpoint didn't return? No — trust live.
    if not free and not paid:
        tui.error("no models returned by provider; check base_url.")
        return None

    rows: list[tuple[int, zen.ZenModel]] = []
    tui.say("")
    tui.say("[bold green]Free models (no auth needed)[/]")
    idx = 1
    for m in free:
        mark = " <— current" if m.id == cfg.model else ""
        tui.say(f"  [cyan]{idx:>2}[/]  {m.id:<26} {m.label}{mark}")
        rows.append((idx, m))
        idx += 1
    if paid:
        tui.say("")
        tui.say("[bold]Paid models (require API key / credits)[/]")
        for m in paid:
            mark = " <— current" if m.id == cfg.model else ""
            tui.say(f"  [cyan]{idx:>2}[/]  {m.id:<26} {m.label}{mark}")
            rows.append((idx, m))
            idx += 1

    tui.say("")
    sel = Prompt.ask(
        "[bold]Pick a model[/] (number or id, blank to cancel)",
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
    tag = " [green]free[/]" if chosen.free else ""
    tui.say(f"[dim]switched to {chosen.id}{tag}[/]")
    return None


def handle_slash(line: str, cfg: Config, tui: TUI, agent: Agent, cwd: str) -> str | None:
    parts = line.strip().split(maxsplit=1)
    cmd = parts[0].lower()
    arg = parts[1].strip() if len(parts) > 1 else ""

    if cmd in ("/exit", "/quit", "/q"):
        return "exit"
    if cmd == "/help":
        tui.say(HELP)
        return None
    if cmd == "/clear":
        agent.messages = []
        agent.add_system()
        tools.todo._state.pop(cwd, None)
        tui.say("[dim]conversation cleared[/]")
        return "newagent"
    if cmd == "/model":
        if arg:
            _set_model(cfg, arg, agent)
            tui.say(f"[dim]model set to {cfg.model}[/]")
        else:
            tui.say(f"model: {cfg.model}")
        return None
    if cmd == "/models":
        return cmd_models(cfg, tui, agent)
    if cmd == "/config":
        import json
        tui.say(json.dumps(cfg.to_dict(), indent=2))
        return None
    if cmd == "/approve":
        if arg.lower() in ("on", "true", "1", "yes"):
            cfg.auto_approve = True
        elif arg.lower() in ("off", "false", "0", "no"):
            cfg.auto_approve = False
        else:
            cfg.auto_approve = not cfg.auto_approve
        tui.say(f"auto_approve = {cfg.auto_approve}")
        return None
    if cmd == "/tools":
        tui.say("\n".join(f"- {t.name}: {t.description}" for t in tools.all_tools()))
        return None
    if cmd == "/sys":
        if not arg:
            tui.say("usage: /sys <text to append to system message>")
            return None
        if agent.messages and agent.messages[0]["role"] == "system":
            agent.messages[0]["content"] += "\n" + arg
            tui.say("[dim]appended to system message[/]")
        return None
    if cmd == "/cwd":
        if arg:
            try:
                p = Path(arg).expanduser().resolve()
                if not p.is_dir():
                    tui.error(f"not a directory: {p}")
                    return None
                tui.ctx.cwd = str(p)
                tui.say(f"[dim]cwd -> {p}[/]")
            except Exception as e:
                tui.error(str(e))
        else:
            tui.say(f"cwd: {tui.ctx.cwd}")
        return None
    if cmd == "/sessions":
        rows = session.list_sessions()
        if not rows:
            tui.say("[dim]no saved sessions[/]")
        else:
            for sid, prev in rows:
                tui.say(f"[cyan]{sid}[/]  {prev}")
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
        tui.say(f"[dim]saved session {sid} ({saved} messages)[/]")
        return None
    if cmd == "/load":
        if not arg:
            tui.say("usage: /load <session-id>")
            return None
        msgs = session.load_session(arg)
        if not msgs:
            tui.error(f"no session {arg}")
            return None
        agent.messages = [m for m in agent.messages if m["role"] == "system"]
        agent.messages.extend(msgs)
        tui.say(f"[dim]loaded {len(msgs)} messages from {arg}[/]")
        return None
    tui.error(f"unknown command: {cmd} (try /help)")
    return None


def oneshot(cfg: Config, cwd: str, prompt: str) -> int:
    if not cfg.api_key and not _is_no_auth(cfg):
        print("error: no API key. Set OPENAI_API_KEY, run --config, or use a free Zen model.", file=sys.stderr)
        return 2
    ctx = ToolContext(cwd=cwd, approve=lambda k, d: cfg.auto_approve)
    tui = TUI(cfg, ctx) if sys.stdout.isatty() else None
    ctx.emit = lambda s: (tui.say(f"[dim]{s}[/]") if tui else None)

    agent = Agent(
        cfg, ctx,
        on_text=(tui.feed if tui else lambda s: sys.stdout.write(s)),
        on_tool_start=(tui.tool_start if tui else lambda n, a: None),
        on_tool_result=(tui.tool_result if tui else lambda n, a, r, e: None),
        on_assistant_done=(lambda text: (tui.end_stream() if tui else print())),
        on_error=(tui.error if tui else lambda s: print(s, file=sys.stderr)),
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

    cfg = load()
    console = Console()
    console.print("[bold]termagent config[/]")
    cfg.api_key = Prompt.ask("API key", default=cfg.api_key, password=True)
    cfg.base_url = Prompt.ask("Base URL", default=cfg.base_url)
    cfg.model = Prompt.ask("Model", default=cfg.model)
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
