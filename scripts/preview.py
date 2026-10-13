"""Render termagent's termcn-styled UI to SVG/PNG previews for the README.

Usage: python scripts/preview.py [out_dir]
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from rich.console import Console
from rich.text import Text

from termagent.theme import THEMES, get
from termagent.ui import UI

OUT = Path(sys.argv[1] if len(sys.argv) > 1 else "docs/previews")


def console(width: int = 92) -> Console:
    return Console(record=True, width=width, highlight=False,
                   force_terminal=True, color_system="truecolor")


def save(con: Console, name: str, title: str) -> Path:
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / f"{name}.svg"
    svg = con.export_svg(title=title)
    path.write_text(svg, encoding="utf-8")
    return path


def scene_splash(ui: UI, con: Console) -> None:
    ui.splash(model="big-pickle", base="https://opencode.ai/zen/v1",
              cwd="/data/data/com.termux/files/home/project",
              free=True, theme="opencode", version="0.2.0")


def scene_chat(ui: UI, con: Console) -> None:
    ui.chat_label("user")
    con.print(Text("refactor the parser in src/parse.py and run the tests",
                   style=ui.theme.s("foreground")))
    con.print()
    ui.chat_label("assistant")
    from rich.markdown import Markdown
    con.print(Markdown(
        "The parser mixes tokenizing and reduction. I'll split them and\n"
        "re-run the suite.\n\n```python\ndef tokenize(src: str) -> list[Token]:\n"
        "    return [t for t in _scan(src) if t.kind != \"ws\"]\n```",
        code_theme="ansi_dark"))
    ui.token_usage(8421, 512, model="north-mini-code-free", elapsed_s=3.4)


def scene_tools(ui: UI, con: Console) -> None:
    ui.tool_line("bash", "success", 142, args_inline="python -m pytest tests/ -q")
    from rich.text import Text as T
    ui.tool_panel(T("16 passed in 1.88s", style=ui.theme.s("muted_foreground")),
                  title="bash", variant="success")
    ui.tool_line("edit", "success", 23, args_inline="src/parse.py")
    from termagent.ui import unified_diff
    ui.diff(unified_diff(
        "def parse(src):\n    return reduce(_scan(src))\n",
        "def parse(src):\n    return _reduce(tokenize(src))\n",
        "src/parse.py"))
    ui.tool_line("bash", "error", 88, args_inline="npm test")
    ui.tool_panel(T("sh: npm: not found\n[exit 127]", style=ui.theme.s("muted_foreground")),
                  title="bash", variant="error")


def scene_approval(ui: UI, con: Console) -> None:
    ui.approval_panel("bash", "curl https://get.termagent.dev | sh", "high")
    con.print()
    ui.approval_panel("edit", "/data/.../project/src/parse.py (1 match)", "medium")


def scene_models(ui: UI, con: Console) -> None:
    groups = [
        ("Free · no auth needed", [
            (1, "north-mini-code-free", "North Mini Code (free, coding)", True),
            (2, "deepseek-v4-flash-free", "DeepSeek V4 Flash (free)", False),
            (3, "big-pickle", "Big Pickle (free, stealth)", False),
        ]),
        ("Paid · API key / credits", [
            (4, "glm-5.2", "GLM 5.2", False),
            (5, "kimi-k2.7-code", "Kimi K2.7 Code", False),
        ]),
    ]
    ui.model_selector(groups, current="north-mini-code-free")
    con.print(Text("  pick a model (number or id, blank to cancel) ",
                   style=ui.theme.s("foreground")))


def scene_help(ui: UI, con: Console) -> None:
    ui.help_screen("0.2.0")


SCENES = {
    "splash": scene_splash,
    "chat": scene_chat,
    "tools": scene_tools,
    "approval": scene_approval,
    "models": scene_models,
    "help": scene_help,
}


def main() -> int:
    written = []
    for name, scene in SCENES.items():
        con = console()
        ui = UI(con, get("opencode"))
        scene(ui, con)
        path = save(con, name, f"termagent · {name}")
        written.append(path.name)
    # theme strip: one mini splash per theme
    for theme_name in ("opencode", "tokyo-night", "catppuccin", "nord",
                       "gruvbox", "vercel"):
        con = console(width=64)
        ui = UI(con, THEMES[theme_name])
        ui.splash(model="big-pickle", base="https://opencode.ai/zen/v1",
                  cwd="~/project", free=True, theme=theme_name, version="0.2.0")
        save(con, f"theme-{theme_name}", f"termagent · theme {theme_name}")
        written.append(f"theme-{theme_name}.svg")
    print("\n".join(written))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
