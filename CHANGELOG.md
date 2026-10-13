# Changelog

## 1.0.0 — TypeScript rewrite on Ink + termcn

The Python implementation remains available at the `v0.2.0-python` tag. Session
files (`~/.config/termagent/sessions/*.jsonl`) and `config.json` carry over.

### UI
- Full rewrite of the interface as a full-screen Ink app on the termcn component
  system (https://termcn.dev): app-shell layout (header with status badges,
  scrollable chat feed, bordered composer, hints footer).
- Real interactive termcn components replace the Python build's static renders:
  ToolCall (live braille spinner, elapsed ms, args), ToolApproval (risk badge with
  y/n/a keys), ModelSelector (arrow keys + enter, grouped free/paid), TokenUsage,
  ChatMessage role labels, ThinkingBlock, DiffView (edit results), Badge, Panel.
- 17 termcn themes with live hot-swap via `/theme` (no restart), persisted to
  config; also `--theme` / `--list-themes` / `TERMAGENT_THEME`.
- Edit results render a colored unified diff (termcn DiffView).
- Arrow keys scroll the feed; esc closes overlays.

### Agent
- Ported agent core: streaming OpenAI-compatible client over fetch (chunked
  tool-call accumulation, reasoning deltas, usage capture), tool-calling loop with
  per-tool durations, per-turn token usage.
- Same 8 tools ported from Python (bash, read, ls, glob, grep, write, edit, todo)
  with the same result formats; grep still prefers ripgrep.
- Graceful `stream_options` fallback (providers that reject the field are retried
  without it).

### Provider fixes carried over from 0.2.0
- Zen catalog rebuilt from the live /models list; the 0.1.0 default model no
  longer exists upstream.
- Zen's free tier is OpenCode-client-gated; the 403 FreeTierError is detected and
  explained with actionable options (key, other provider, local model). No UA
  spoofing.
- Stale Zen model ids auto-heal at startup; custom endpoints are never
  second-guessed.

### Packaging / quality
- npm package: `npm link` or `npm i -g .` installs the `termagent` binary;
  Node >= 20; ships compiled `dist/` only (no source maps).
- vitest suite (29 tests): full agent loop against a scripted local SSE mock
  server, SSE parsing and fallbacks, all 8 tools on temp dirs, theme registry
  completeness, termcn component renders via ink-testing-library.
- Strict TypeScript, zero `tsc` errors; UI verified in tmux at 100x40 with
  screenshots in `docs/previews/`.

## 0.2.0 — termcn-themed Python build (see the v0.2.0-python tag)
- termcn design language ported to rich: 17 themes, badges/alerts/tool
  call/approval/model selector/token usage/diff view, Zen provider fixes, test
  suite.

## 0.1.0 — initial release (Python)
- Interactive REPL, streaming markdown, 8 tools, human-in-the-loop approval,
  /models picker, session save/resume, OpenCode Zen defaults.
