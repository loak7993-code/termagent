# Changelog

## 2.0.1 - 2026-11-15

### Fixed

- The TUI no longer shows an empty alternate screen when the terminal reports a 0x0
  size. It waits briefly for a real size, then explains how to resize and exits.
- Running the TUI without an interactive terminal now prints an actionable message
  instead of `No such device or address`, pointing at `termagent --web`.
- The terminal is restored on panic, so a crash cannot leave the shell in raw mode.
- `--onboard` starts at the provider list even with a half-written config, and a normal
  start asks for the missing key instead of jumping to effort.

### Added

- Prebuilt Termux binaries in the release and installer download path.

## 2.0.0 - 2026-11-14

Rust rewrite. The Ink/TypeScript TUI is gone; one binary now serves both surfaces.

### Added

- Native TUI on ratatui: provider wizard, streaming chat, tool cards with real
  durations, approval modal, model picker, session picker, eight themes, ASCII fallback.
- `termagent --web`: local browser chat on `127.0.0.1:6033` with a session sidebar,
  searchable model chooser, inline approvals, usage, and streaming replies.
- First-run onboarding in both surfaces: provider, key, model, effort.
- Providers and models from models.dev (309 providers, 7,958 tool models in the bundled
  snapshot), refreshed on demand and cached for 24 hours.
- Three protocol adapters: OpenAI-compatible SSE, Anthropic Messages, Google Gemini.
- Per-provider key storage, mode 0600; environment keys are detected but never written.
- Effort control (auto, low, medium, high, max) mapped to each protocol's reasoning field.
- Auto effort heuristic per message, visible as `auto → high` in the UI.
- Tool durations measured around execution, excluding approval wait.
- WebSocket protocol with a per-boot token and origin check; REST API for sessions.
- `--onboard`, `--list-models`, `--refresh`, `--session`, `--cwd`, `--port`.
- One-command installer that bootstraps Termux, Linux, and macOS, downloads prebuilt
  Termux binaries from GitHub releases, and falls back to a source build.
- Custom OpenAI-compatible endpoints via a bare `--base-url`.

### Changed

- Web UI built with React 19, Vite, Tailwind v4, shadcn components, `@aicss/react`
  primitives, and Phosphor icons; web assets are embedded in the binary.
- Sessions use a JSONL event log with a meta line, title events, tool metadata, and totals.
- Tool schemas and messages are converted per protocol instead of assuming one wire shape.

### Removed

- The TypeScript/Ink application and its npm dependencies.
- The hardcoded OpenCode Zen catalog, including model ids the client could not speak to.
- `--config` (replaced by `--onboard`), `TERMAGENT_SHOW_REASONING`, and the old 17-theme set.
- `sudo`, `su`, `doas`, `pkexec`, and `run0` execution; privilege escalation is refused.

### Security

- File tools resolve paths inside the workspace and reject symlink escapes.
- Child processes start without API keys, cloud credentials, or SSH agent sockets.
- API keys never reach the browser, session files, or logs.
- Web server binds 127.0.0.1, requires the boot token on the API and WebSocket, and sends
  no CORS headers.
