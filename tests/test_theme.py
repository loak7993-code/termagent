from __future__ import annotations

import re

from termagent.theme import DEFAULT_THEME, THEMES, get, names

HEX = re.compile(r"^#[0-9a-fA-F]{6}$")

REQUIRED_ROLES = [
    "primary", "primary_foreground", "secondary", "secondary_foreground",
    "accent", "accent_foreground", "success", "success_foreground",
    "warning", "warning_foreground", "error", "error_foreground",
    "info", "info_foreground", "background", "foreground",
    "muted", "muted_foreground", "border", "focus_ring",
    "selection", "selection_foreground",
]


def test_all_17_termcn_themes_present():
    assert len(THEMES) == 17
    assert len(names()) == len(set(names()))


def test_every_theme_has_complete_color_tokens():
    for name, t in THEMES.items():
        for role in REQUIRED_ROLES:
            value = getattr(t, role)
            assert HEX.match(value), f"{name}.{role} = {value!r} is not #rrggbb"
        assert t.border_style == "round"


def test_default_theme_is_opencode():
    assert DEFAULT_THEME == "opencode"
    assert get("opencode").primary == "#fab283"


def test_unknown_theme_falls_back_to_default():
    assert get("does-not-exist").name == DEFAULT_THEME
    assert get("").name == DEFAULT_THEME
    assert get(None).name == DEFAULT_THEME


def test_style_helper_composes_modifiers():
    t = get("opencode")
    assert t.s("primary") == "#fab283"
    assert t.s("primary", bold=True) == "bold #fab283"
    assert t.s("primary", bold=True, dim=True) == "bold dim #fab283"
