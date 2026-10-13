// Theme registry: all 17 termcn themes + resolution helpers.
// Mirrors termagent/theme.py from the Python build.

import type { Theme } from "../components/ui/types.js";

import { defaultTheme } from "./terminal-themes/default.js";
import { opencodeTheme } from "./terminal-themes/opencode.js";
import { tokyoNightTheme } from "./terminal-themes/tokyo-night.js";
import { catppuccinTheme } from "./terminal-themes/catppuccin.js";
import { nordTheme } from "./terminal-themes/nord.js";
import { gruvboxTheme } from "./terminal-themes/gruvbox.js";
import { vercelTheme } from "./terminal-themes/vercel.js";
import { vesperTheme } from "./terminal-themes/vesper.js";
import { everforestTheme } from "./terminal-themes/everforest.js";
import { githubTheme } from "./terminal-themes/github.js";
import { amoledTheme } from "./terminal-themes/amoled.js";
import { flexokiTheme } from "./terminal-themes/flexoki.js";
import { kanagawaTheme } from "./terminal-themes/kanagawa.js";
import { rosepineTheme } from "./terminal-themes/rosepine.js";
import { mercuryTheme } from "./terminal-themes/mercury.js";
import { ayuTheme } from "./terminal-themes/ayu.js";
import { carbonfoxTheme } from "./terminal-themes/carbonfox.js";

export const THEMES: Record<string, Theme> = Object.fromEntries(
  [
    defaultTheme,
    opencodeTheme,
    tokyoNightTheme,
    catppuccinTheme,
    nordTheme,
    gruvboxTheme,
    vercelTheme,
    vesperTheme,
    everforestTheme,
    githubTheme,
    amoledTheme,
    flexokiTheme,
    kanagawaTheme,
    rosepineTheme,
    mercuryTheme,
    ayuTheme,
    carbonfoxTheme,
  ].map((t) => [t.name, t]),
);

export const DEFAULT_THEME = "opencode";

export const themeNames = (): string[] => Object.keys(THEMES).sort();

export function getTheme(name?: string | null): Theme {
  const key = (name ?? "").trim().toLowerCase();
  return THEMES[key] ?? THEMES[DEFAULT_THEME];
}

export function unicodeEnabled(): boolean {
  if (process.env.TERMAGENT_ASCII) return false;
  const enc = (process.env.LC_ALL ?? process.env.LANG ?? "").toLowerCase();
  return enc.includes("utf") || process.platform !== "win32";
}
