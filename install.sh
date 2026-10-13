#!/usr/bin/env bash
# termagent installer (TypeScript build) — for Termux (Android) and any Linux/macOS shell.
#
# One-liner:
#   curl -fsSL https://raw.githubusercontent.com/loak7993-code/termagent/main/install.sh | bash
#
# Or clone and run locally:
#   git clone https://github.com/loak7993-code/termagent && cd termagent && ./install.sh

set -euo pipefail

NODE_MIN_MAJOR=20

ok()   { printf "\033[32m✔ %s\033[0m\n" "$*"; }
warn() { printf "\033[33m! %s\033[0m\n" "$*"; }
err()  { printf "\033[31m✗ %s\033[0m\n" "$*" >&2; }

# ---- detect environment -------------------------------------------------------
IS_TERMUX=0
if [ -n "${PREFIX:-}" ] && [[ "$PREFIX" == *com.termux* ]]; then
  IS_TERMUX=1
fi

# ---- 1) node ------------------------------------------------------------------
if ! command -v node >/dev/null 2>&1; then
  err "node not found."
  if [ "$IS_TERMUX" -eq 1 ]; then
    echo "  Run:  pkg install -y nodejs git"
  else
    echo "  Install Node.js ${NODE_MIN_MAJOR}+ from https://nodejs.org"
  fi
  exit 1
fi
NODEV="$(node -p 'process.versions.node')"
NODEMAJ="${NODEV%%.*}"
if [ "$NODEMAJ" -lt "$NODE_MIN_MAJOR" ]; then
  err "Node ${NODE_MIN_MAJOR}+ required, found ${NODEV}"
  exit 1
fi
ok "node ${NODEV}"

# ---- 2) npm -------------------------------------------------------------------
if ! command -v npm >/dev/null 2>&1; then
  err "npm not found (usually ships with node)."
  exit 1
fi
ok "npm $(npm --version)"

# ---- 3) pick install mode -----------------------------------------------------
HERE="$(pwd)"
if [ -f "$HERE/package.json" ] && grep -q '"name": "termagent"' "$HERE/package.json" 2>/dev/null; then
  SRC="$HERE"
  ok "installing from local source: $SRC"
else
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  SRC="$TMP/termagent"
  echo "Cloning termagent…"
  git clone --depth 1 https://github.com/loak7993-code/termagent.git "$SRC" >/dev/null 2>&1 || {
    err "git clone failed. Install git (Termux: pkg install git) or re-run from inside the repo."
    exit 1
  }
  ok "cloned to $SRC"
fi

# ---- 4) install + build -------------------------------------------------------
cd "$SRC"
echo "Installing dependencies (ink, react)…"
npm install --no-audit --no-fund
ok "dependencies installed"

echo "Building…"
npm run build
ok "built dist/"

# ---- 5) global link -----------------------------------------------------------
if npm link >/dev/null 2>&1 || npm install -g . >/dev/null 2>&1; then
  ok "termagent installed globally"
else
  BIN="$(npm bin -g 2>/dev/null || echo "$HOME/.npm-global/bin")"
  warn "global install failed; run the app with: node $SRC/dist/cli.js"
fi

# ---- 6) verify ----------------------------------------------------------------
if command -v termagent >/dev/null 2>&1; then
  ok "termagent command available: $(command -v termagent)"
else
  warn "termagent not on PATH yet — open a new shell or run: npm link"
fi

echo
echo "Done. Run:  termagent          (OpenCode Zen by default; set a key with"
echo "            termagent --config, or point -b/-k at any OpenAI-compatible endpoint)"
echo "  /help inside the TUI lists all commands.  See:  termagent --help"
