# Changelog

## 2.0.6 - 2026-11-15

### Fixed

- Pasted text now reaches onboarding fields and overlay search boxes. On Termux a
  pasted API key was previously dropped, so the old key kept being used and requests
  stayed at 401.
- Bracketed paste is enabled for the TUI lifetime and disabled on exit.
- The key screen says when a saved key exists and that pasting replaces it.
- The cursor sits on the key field regardless of the environment or saved-key notice.

## 2.0.5 - 2026-11-15

### Fixed

- CoreWeave / W&B Inference works with only an API key. The team/project requirement and
  its wizard step were wrong; the project is optional again and stays available through
  `--project`, `TERMAGENT_PROJECT`, and `OPENAI_PROJECT`.
- A missing project no longer blocks startup or forces onboarding.

## 2.0.4 - 2026-11-15

### Fixed

- A CoreWeave config without a project now counts as unconfigured, so the TUI opens
  the project step instead of starting an agent that fails with repeated errors.
- Resuming a CoreWeave setup with a key but no project opens the project screen.
- The TUI worker no longer starts before onboarding is complete, removing duplicate
  startup errors and the follow-up `session is not ready` failure.

## 2.0.3 - 2026-11-15

### Fixed

- CoreWeave / W&B Inference now collects the Forge team and project and sends the
  required `OpenAI-Project` header. A missing project fails before the request instead
  of returning 401.
- Provider switching clears the previous project.
- Duplicate error cards are collapsed so one failure shows once.

### Added

- `--project team/project` and `TERMAGENT_PROJECT` / `OPENAI_PROJECT`.
- A project step in the TUI wizard and a project field in the browser wizard for
  CoreWeave.

## 2.0.2 - 2026-11-15

### Fixed

- A legacy single key no longer follows a provider switch, and pasted keys are cleaned
  of quotes, whitespace, and a leading `Bearer ` before use.
- Onboarding and provider changes clear a leftover custom base URL, so a key is no
  longer sent to the previous endpoint.
- Environment keys are checked before the legacy runtime key.

### Added

- `termagent --status`: provider, model, effort, endpoint, registry, paths, and the key
  source with a masked value.
- `/key` in the TUI and a key replacement screen that keeps the current provider.
- Auth failures now name the provider and model and say how to replace the key.

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
