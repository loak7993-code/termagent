"""Terminal themes ported from the termcn registry (https://termcn.dev).

termcn ships its themes as a `Theme` object with semantic color tokens
(primary, success, error, ...) plus border/spacing/typography tokens. This
module is a Python port of that contract for rich, with the theme values
taken from the termcn registry sources (registry/themes/*.ts, MIT).
"""

from __future__ import annotations

import os
from dataclasses import dataclass


@dataclass(frozen=True)
class Theme:
    name: str
    # Semantic color tokens, mirroring termcn's ColorTokens.
    primary: str
    primary_foreground: str
    secondary: str
    secondary_foreground: str
    accent: str
    accent_foreground: str
    success: str
    success_foreground: str
    warning: str
    warning_foreground: str
    error: str
    error_foreground: str
    info: str
    info_foreground: str
    background: str
    foreground: str
    muted: str
    muted_foreground: str
    border: str
    focus_ring: str
    selection: str
    selection_foreground: str
    border_style: str = "round"

    def s(self, role: str, *, bold: bool = False, dim: bool = False) -> str:
        """Return a rich style string for a semantic role."""
        color = getattr(self, role, self.foreground)
        mods = []
        if bold:
            mods.append("bold")
        if dim:
            mods.append("dim")
        return " ".join(mods + [color])


def _t(
    name: str,
    bg: str,
    fg: str,
    prim: str,
    prim_fg: str,
    sec: str,
    sec_fg: str,
    accent: str,
    accent_fg: str,
    succ: str,
    err: str,
    warn: str,
    info: str,
    border: str,
    muted: str,
    focus: str,
    sel: str,
    sel_fg: str,
) -> Theme:
    """Build a Theme from the terse termcn registry field order."""
    return Theme(
        name=name,
        primary=prim,
        primary_foreground=prim_fg,
        secondary=sec,
        secondary_foreground=sec_fg,
        accent=accent,
        accent_foreground=accent_fg,
        success=succ,
        success_foreground=prim_fg,
        warning=warn,
        warning_foreground=prim_fg,
        error=err,
        error_foreground=prim_fg,
        info=info,
        info_foreground=prim_fg,
        background=bg,
        foreground=fg,
        muted=sec,
        muted_foreground=muted,
        border=border,
        focus_ring=focus,
        selection=sel,
        selection_foreground=sel_fg,
    )


# Values from the termcn registry (registry/themes/*.ts). Each entry keeps the
# upstream palette verbatim; see PROVENANCE note in the README.
THEMES: dict[str, Theme] = {t.name: t for t in [
    _t("opencode",
       bg="#0a0a0a", fg="#eeeeee", prim="#fab283", prim_fg="#0a0a0a",
       sec="#151515", sec_fg="#eeeeee", accent="#9d7cd8", accent_fg="#0a0a0a",
       succ="#7fd88f", err="#e06c75", warn="#f5a742", info="#56b6c2",
       border="#808080", muted="#808080", focus="#9d7cd8",
       sel="#fab283", sel_fg="#0a0a0a"),
    _t("default",
       bg="#000000", fg="#FFFFFF", prim="#7C3AED", prim_fg="#FFFFFF",
       sec="#6B7280", sec_fg="#FFFFFF", accent="#8B5CF6", accent_fg="#FFFFFF",
       succ="#10B981", err="#EF4444", warn="#F59E0B", info="#3B82F6",
       border="#4B5563", muted="#9CA3AF", focus="#8B5CF6",
       sel="#7C3AED", sel_fg="#FFFFFF"),
    _t("tokyo-night",
       bg="#24283B", fg="#C0CAF5", prim="#7AA2F7", prim_fg="#1A1B26",
       sec="#BB9AF7", sec_fg="#1A1B26", accent="#BB9AF7", accent_fg="#1A1B26",
       succ="#9ECE6A", err="#F7768E", warn="#E0AF68", info="#7DCFFF",
       border="#3B4261", muted="#565F89", focus="#7AA2F7",
       sel="#364A82", sel_fg="#C0CAF5"),
    _t("catppuccin",
       bg="#1E1E2E", fg="#CDD6F4", prim="#CBA6F7", prim_fg="#1E1E2E",
       sec="#89B4FA", sec_fg="#1E1E2E", accent="#F38BA8", accent_fg="#1E1E2E",
       succ="#A6E3A1", err="#F38BA8", warn="#F9E2AF", info="#89B4FA",
       border="#45475A", muted="#6C7086", focus="#CBA6F7",
       sel="#313244", sel_fg="#CDD6F4"),
    _t("nord",
       bg="#2E3440", fg="#ECEFF4", prim="#88C0D0", prim_fg="#2E3440",
       sec="#4C566A", sec_fg="#ECEFF4", accent="#81A1C1", accent_fg="#2E3440",
       succ="#A3BE8C", err="#BF616A", warn="#EBCB8B", info="#5E81AC",
       border="#4C566A", muted="#4C566A", focus="#88C0D0",
       sel="#3B4252", sel_fg="#ECEFF4"),
    _t("gruvbox",
       bg="#282828", fg="#ebdbb2", prim="#83a598", prim_fg="#282828",
       sec="#32302f", sec_fg="#ebdbb2", accent="#fb4934", accent_fg="#282828",
       succ="#b8bb26", err="#fb4934", warn="#fabd2f", info="#d3869b",
       border="#928374", muted="#928374", focus="#fb4934",
       sel="#83a598", sel_fg="#282828"),
    _t("vercel",
       bg="#000000", fg="#ededed", prim="#0070f3", prim_fg="#000000",
       sec="#1a1a1a", sec_fg="#ededed", accent="#8e4ec6", accent_fg="#000000",
       succ="#46a758", err="#e5484d", warn="#ffb224", info="#52a8ff",
       border="#454545", muted="#878787", focus="#f75590",
       sel="#0070f3", sel_fg="#000000"),
    _t("vesper",
       bg="#101010", fg="#ffffff", prim="#ffc799", prim_fg="#101010",
       sec="#181818", sec_fg="#ffffff", accent="#ff8080", accent_fg="#101010",
       succ="#99ffe4", err="#ff8080", warn="#ffc799", info="#ffc799",
       border="#8b8b8b", muted="#8b8b8b", focus="#a0a0a0",
       sel="#ffc799", sel_fg="#101010"),
    _t("everforest",
       bg="#2d353b", fg="#d3c6aa", prim="#a7c080", prim_fg="#2d353b",
       sec="#373e44", sec_fg="#d3c6aa", accent="#d699b6", accent_fg="#2d353b",
       succ="#a7c080", err="#e67e80", warn="#e69875", info="#83c092",
       border="#7a8478", muted="#7a8478", focus="#d699b6",
       sel="#a7c080", sel_fg="#2d353b"),
    _t("github",
       bg="#0d1117", fg="#c9d1d9", prim="#58a6ff", prim_fg="#0d1117",
       sec="#21262d", sec_fg="#c9d1d9", accent="#39c5cf", accent_fg="#0d1117",
       succ="#3fb950", err="#f85149", warn="#e3b341", info="#d29922",
       border="#30363d", muted="#8b949e", focus="#58a6ff",
       sel="#58a6ff", sel_fg="#0d1117"),
    _t("amoled",
       bg="#000000", fg="#ffffff", prim="#b388ff", prim_fg="#000000",
       sec="#333333", sec_fg="#ffffff", accent="#ff4081", accent_fg="#000000",
       succ="#00ff88", err="#ff1744", warn="#ffea00", info="#18ffff",
       border="#555555", muted="#888888", focus="#b388ff",
       sel="#b388ff", sel_fg="#000000"),
    _t("flexoki",
       bg="#100f0f", fg="#cecdc3", prim="#da702c", prim_fg="#100f0f",
       sec="#1a1916", sec_fg="#cecdc3", accent="#8b7ec8", accent_fg="#100f0f",
       succ="#879a39", err="#d14d41", warn="#da702c", info="#3aa99f",
       border="#6f6e69", muted="#6f6e69", focus="#879a39",
       sel="#da702c", sel_fg="#100f0f"),
    _t("kanagawa",
       bg="#1f1f28", fg="#dcd7ba", prim="#7e9cd8", prim_fg="#1f1f28",
       sec="#292a3f", sec_fg="#dcd7ba", accent="#d27e99", accent_fg="#1f1f28",
       succ="#98bb6c", err="#e82424", warn="#d7a657", info="#76946a",
       border="#727169", muted="#727169", focus="#957fbb",
       sel="#7e9cd8", sel_fg="#1f1f28"),
    _t("rosepine",
       bg="#191724", fg="#e0def4", prim="#9ccfd8", prim_fg="#191724",
       sec="#262330", sec_fg="#e0def4", accent="#ebbcba", accent_fg="#191724",
       succ="#31748f", err="#eb6f92", warn="#f6c177", info="#9ccfd8",
       border="#6e6a86", muted="#6e6a86", focus="#31748f",
       sel="#9ccfd8", sel_fg="#191724"),
    _t("mercury",
       bg="#171721", fg="#dddde5", prim="#8da4f5", prim_fg="#171721",
       sec="#1e1e28", sec_fg="#dddde5", accent="#8da4f5", accent_fg="#171721",
       succ="#77c599", err="#fc92b4", warn="#fc9b6f", info="#77becf",
       border="#9d9da8", muted="#9d9da8", focus="#8da4f5",
       sel="#8da4f5", sel_fg="#171721"),
    _t("ayu",
       bg="#0f1419", fg="#d6dae0", prim="#3fb7e3", prim_fg="#0f1419",
       sec="#242a33", sec_fg="#d6dae0", accent="#f2856f", accent_fg="#0f1419",
       succ="#78d05c", err="#f58572", warn="#e4a75c", info="#66c6f1",
       border="#5a6673", muted="#5a6673", focus="#3fb7e3",
       sel="#3fb7e3", sel_fg="#0f1419"),
    _t("carbonfox",
       bg="#393939", fg="#f2f4f8", prim="#33b1ff", prim_fg="#393939",
       sec="#454545", sec_fg="#f2f4f8", accent="#ff8389", accent_fg="#393939",
       succ="#42be65", err="#ff8389", warn="#f1c21b", info="#78a9ff",
       border="#6f6f6f", muted="#6f6f6f", focus="#be95ff",
       sel="#33b1ff", sel_fg="#393939"),
]}

DEFAULT_THEME = "opencode"


def get(name: str | None) -> Theme:
    """Resolve a theme by name; unknown names fall back to the default."""
    key = (name or "").strip().lower()
    return THEMES.get(key, THEMES[DEFAULT_THEME])


def names() -> list[str]:
    return sorted(THEMES)


def theme_from_env() -> Theme:
    return get(os.environ.get("TERMAGENT_THEME"))


def unicode_enabled() -> bool:
    """termcn's unicode provider equivalent: ASCII fallback via env switch."""
    if os.environ.get("TERMAGENT_ASCII"):
        return False
    enc = (os.environ.get("LC_ALL") or os.environ.get("LANG") or "").lower()
    return "utf" in enc or os.name == "posix"
