#!/usr/bin/env bash
# termagent installer — for Termux (Android) and any Linux/macOS shell.
#
# One-liner:
#   curl -fsSL https://raw.githubusercontent.com/loak7993-code/termagent/main/install.sh | bash
#
# Or clone and run locally:
#   git clone https://github.com/loak7993-code/termagent && cd termagent && ./install.sh

set -euo pipefail

PY_MIN_MAJOR=3
PY_MIN_MINOR=9

ok()   { printf "\033[32m✔ %s\033[0m\n" "$*"; }
warn() { printf "\033[33m! %s\033[0m\n" "$*"; }
err()  { printf "\033[31m✗ %s\033[0m\n" "$*" >&2; }

# ---- detect environment -------------------------------------------------------
IS_TERMUX=0
if [ -n "${PREFIX:-}" ] && [[ "$PREFIX" == *com.termux* ]]; then
  IS_TERMUX=1
fi

# ---- 1) python ----------------------------------------------------------------
if ! command -v python3 >/dev/null 2>&1; then
  err "python3 not found."
  if [ "$IS_TERMUX" -eq 1 ]; then
    echo "  Run:  pkg install -y python python-pip"
  else
    echo "  Install Python ${PY_MIN_MAJOR}.${PY_MIN_MINOR}+ from https://python.org"
  fi
  exit 1
fi

PYV="$(python3 -c 'import sys;print("%d.%d"%sys.version_info[:2])')"
PYMAJ="${PYV%%.*}"; PYMIN="${PYV#*.}"
if [ "$PYMAJ" -lt "$PY_MIN_MAJOR" ] || { [ "$PYMAJ" -eq "$PY_MIN_MAJOR" ] && [ "$PYMIN" -lt "$PY_MIN_MINOR" ]; }; then
  err "Python ${PY_MIN_MAJOR}.${PY_MIN_MINOR}+ required, found ${PYV}"
  exit 1
fi
ok "python ${PYV}"

# ---- 2) pip -------------------------------------------------------------------
if ! python3 -m pip --version >/dev/null 2>&1; then
  warn "pip missing; installing…"
  if [ "$IS_TERMUX" -eq 1 ]; then
    pkg install -y python-pip
  else
    # bootstrap get-pip
    curl -fsSL https://bootstrap.pypa.io/get-pip.py | python3 - --user
  fi
fi
ok "pip"

# ---- 3) pick install mode -----------------------------------------------------
# If a local pyproject.toml is present in the current dir, install from there.
# Otherwise clone from GitHub.
HERE="$(pwd)"
if [ -f "$HERE/pyproject.toml" ] && grep -q 'name = "termagent"' "$HERE/pyproject.toml" 2>/dev/null; then
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

# ---- 4) install ---------------------------------------------------------------
PIP_ARGS=(install --no-input)
# On externally-managed systems (PEP 668) without a venv, fall back to --break-system-packages.
if python3 - <<'EOF' 2>/dev/null
import sysconfig, os
p = sysconfig.get_path("stdlib")
for d in (p, os.path.dirname(p)):
    if os.path.exists(os.path.join(d, "EXTERNALLY-MANAGED")):
        raise SystemExit(0)
raise SystemExit(1)
EOF
then
  PIP_ARGS+=(--break-system-packages)
fi

echo "Installing termagent + dependencies (httpx, rich)…"
python3 -m pip "${PIP_ARGS[@]}" "$SRC"
ok "termagent installed"

# ---- 5) optional: prompt_toolkit for nicer input -----------------------------
if [ -t 0 ] && [ -e /dev/tty ]; then
  read -r -p "Install prompt_toolkit for nicer input (arrow-key history)? [y/N] " _ans < /dev/tty 2>/dev/null || _ans=""
else
  _ans=""
fi
if [[ "${_ans:-}" =~ ^[yY] ]]; then
  python3 -m pip "${PIP_ARGS[@]}" prompt_toolkit
  ok "prompt_toolkit installed"
fi

# ---- 6) verify ----------------------------------------------------------------
if command -v termagent >/dev/null 2>&1; then
  ok "termagent command available: $(command -v termagent)"
else
  # maybe on user path not yet in PATH
  BIN="$(python3 -c 'import sysconfig,site; print(sysconfig.get_path("scripts") if "scripts" in dir() else site.getuserbase()+"/bin")')"
  warn "termagent not on PATH. Try: export PATH=\"\$PATH:$BIN\""
fi

echo
ok "Done. Run:  termagent          (free OpenCode Zen model, no API key needed)"
echo "  /models inside the REPL to switch models.  See:  termagent --help"
