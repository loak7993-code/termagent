# Changelog

## 0.2.0 — termcn UI, provider reality check

### UI (ported from termcn.dev)
- New theme system with 17 termcn themes (semantic color tokens: primary,
  success, warning, error, info, muted, border...), default `opencode`.
  Switch live with `/theme <name>`, persist via config, `--theme` /
  `--list-themes` / `TERMAGENT_THEME` on the CLI.
- New component library on rich: badges, alerts, status lines, splash screen,
  help screen, key-value panels, model selector, token usage line, thinking
  note, chat role labels, unified diff view, braille spinner.
- Tool calls now render like termcn's ToolCall: status symbol, colored name,
  inline args, duration in ms, themed result panel; `edit` results render a
  colored unified diff.
- Approval prompts now render like termcn's ToolApproval: risk border and
  badge (`LOW`/`MEDIUM`/`HIGH`), tool + arguments, `[y] Approve [n] Deny
  [a] Always allow`. Dangerous shell patterns (`rm -rf`, `curl | sh`, ...)
  classify as HIGH risk.
- REPL prompt restyled: dim cwd line + `❯` accent prompt (prompt_toolkit and
  plain-input paths), user messages echoed into the transcript with role
  labels.
- ASCII fallback (`TERMAGENT_ASCII=1`) for all symbols and borders.

### Agent / client
- Token usage per turn via `stream_options.include_usage`; providers that
  reject the field are retried without it automatically. Usage renders as
  `⟨ 1.2k in / 340 out · model · 3.4s ⟩` (suppressed when the provider sends
  none).
- Reasoning deltas (`reasoning_content`) render a `✻ thought for Ns` note;
  `TERMAGENT_SHOW_REASONING=1` prints the full text.
- Tool executions are timed and shown per call.

### Provider fixes (Zen changed upstream)
- The 0.1.0 catalog was stale: `north-mini-code-free` and three other free
  models no longer exist, so fresh installs failed on first message. Catalog
  rebuilt from the live `/models` list (Oct 2026).
- Zen's free tier is now gated to the official OpenCode client; the 403
  FreeTierError is detected and explained with actionable options instead of
  a raw JSON dump. termagent does not spoof OpenCode's client headers.
- Stale saved Zen model ids auto-heal against the live list at startup
  (custom endpoints are never second-guessed).
- Default model moved to `big-pickle`.

### Housekeeping
- `--version` flag added; `termagent/__init__.py` exports `__version__`.
- pytest suite added: themes, UI components, and the full agent loop against
  a local mock SSE server (streaming, tool accumulation, usage, reasoning,
  stream_options fallback).
- README rewritten; verified UI previews added under `docs/previews/`.

## 0.1.0 — initial release
- Interactive REPL, streaming markdown, 8 tools, human-in-the-loop approval,
  /models picker, session save/resume, OpenCode Zen defaults.
