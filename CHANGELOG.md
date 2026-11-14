# Changelog

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
