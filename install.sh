#!/usr/bin/env bash
# One-command installer for termagent.
#
#   ./install.sh
#   curl -fsSL https://raw.githubusercontent.com/loak7993-code/termagent/main/install.sh | bash
#
# It installs the build toolchain when needed (Termux: pkg install rust; Linux/macOS:
# rustup), downloads the source when the script is piped, builds the release binary,
# and installs it into a user-writable bin directory. No root is used for the app.
#
# Options:
#   --prefix DIR     install into DIR/bin (default: ~/.local/bin, or $PREFIX/bin on Termux)
#   --add-path       append the install dir to your shell rc when it is not on PATH
#   --no-bootstrap   fail instead of installing a toolchain
#   --dry-run        print the plan without changing anything
#   -h, --help       show this help
#
# Environment:
#   TERMAGENT_PREFIX       same as --prefix
#   TERMAGENT_SRC          existing source tree to build
#   TERMAGENT_REPO         GitHub repo, default loak7993-code/termagent
#   TERMAGENT_REF          git ref for the source tarball, default main
#   TERMAGENT_JOBS         cargo build jobs
set -eu

REPO="${TERMAGENT_REPO:-loak7993-code/termagent}"
REF="${TERMAGENT_REF:-main}"
SOURCE_DIR="${TERMAGENT_SRC:-}"
PREFIX_DIR="${TERMAGENT_PREFIX:-}"
ADD_PATH=0
BOOTSTRAP=1
DRY_RUN="${TERMAGENT_DRY_RUN:-0}"
ROOT=""
TMP_DIR=""
SUDO=""

usage() {
  sed -n '2,30p' "$0" 2>/dev/null || printf '%s\n' "termagent installer"
}

log() { printf '[termagent] %s\n' "$*"; }
warn() { printf '[termagent] %s\n' "$*" >&2; }
die() { warn "$*"; exit 1; }

sudo_cmd() {
  if [ "$(id -u)" -eq 0 ]; then
    SUDO=""
  elif have sudo; then
    SUDO="sudo"
  else
    die "this step needs root or sudo; install the packages manually and rerun with --no-bootstrap"
  fi
}

run() {
  if [ "$DRY_RUN" = "1" ]; then
    printf '[termagent] would run: %s\n' "$*"
    return 0
  fi
  "$@"
}

have() { command -v "$1" >/dev/null 2>&1; }

cleanup() {
  if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
    rm -rf "$TMP_DIR"
  fi
}
trap cleanup EXIT INT TERM

while [ $# -gt 0 ]; do
  case "$1" in
    --prefix)
      [ $# -ge 2 ] || die "--prefix needs a directory"
      PREFIX_DIR="$2"
      shift 2
      ;;
    --prefix=*)
      PREFIX_DIR="${1#--prefix=}"
      shift
      ;;
    --add-path)
      ADD_PATH=1
      shift
      ;;
    --no-bootstrap)
      BOOTSTRAP=0
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
done

is_termux() {
  [ -n "${TERMUX_VERSION:-}" ] && return 0
  [ -d "/data/data/com.termux/files/usr" ] && return 0
  case "${PREFIX:-}" in
    /data/data/com.termux/*) return 0 ;;
  esac
  return 1
}

TERMUX=0
if is_termux; then TERMUX=1; fi
OS="$(uname -s 2>/dev/null || echo unknown)"

# ---------------------------------------------------------------- source tree
resolve_source() {
  if [ -n "$SOURCE_DIR" ]; then
    [ -f "$SOURCE_DIR/Cargo.toml" ] || die "TERMAGENT_SRC has no Cargo.toml: $SOURCE_DIR"
    ROOT="$SOURCE_DIR"
    return
  fi
  local script_dir=""
  if [ -n "${BASH_SOURCE[0]:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  fi
  if [ -n "$script_dir" ] && [ -f "$script_dir/Cargo.toml" ]; then
    ROOT="$script_dir"
    return
  fi

  log "downloading source ($REPO@$REF)"
  if [ "$DRY_RUN" = "1" ]; then
    log "would download https://codeload.github.com/$REPO/tar.gz/refs/heads/$REF"
    return
  fi
  install_curl
  have tar || die "tar is required to unpack the source"
  TMP_DIR="$(mktemp -d)"
  if ! curl -fsSL "https://codeload.github.com/$REPO/tar.gz/refs/heads/$REF" | tar -xz -C "$TMP_DIR"; then
    die "could not download the source; clone the repo and run ./install.sh"
  fi
  ROOT="$(find "$TMP_DIR" -maxdepth 1 -type d -name 'termagent-*' | head -n 1)"
  [ -n "$ROOT" ] && [ -f "$ROOT/Cargo.toml" ] || die "downloaded source is missing Cargo.toml"
}

# ------------------------------------------------------------------ toolchain
find_cargo() {
  if have cargo; then
    return 0
  fi
  local candidate
  for candidate in "$HOME/.cargo/bin/cargo" "/usr/local/bin/cargo" "/usr/bin/cargo"; do
    if [ -x "$candidate" ]; then
      PATH="$(dirname "$candidate"):$PATH"
      export PATH
      return 0
    fi
  done
  return 1
}

install_curl() {
  if have curl; then return 0; fi
  if [ "$TERMUX" = "1" ]; then
    run pkg install -y curl
  elif [ "$OS" = "Darwin" ]; then
    die "curl is missing; install the Xcode command line tools first"
  elif have apt-get; then
    sudo_cmd
    run $SUDO apt-get update
    run $SUDO apt-get install -y curl
  elif have dnf; then
    sudo_cmd
    run $SUDO dnf install -y curl
  elif have pacman; then
    sudo_cmd
    run $SUDO pacman -S --needed --noconfirm curl
  else
    die "curl is required"
  fi
}

install_rust() {
  if [ "$BOOTSTRAP" != "1" ]; then
    die "cargo is not installed; install Rust or rerun without --no-bootstrap"
  fi
  log "installing the Rust toolchain"

  if [ "$TERMUX" = "1" ]; then
    log "using Termux packages: rust clang make"
    run pkg update -y || true
    run pkg install -y rust clang make perl
    if [ "$DRY_RUN" = "1" ]; then return 0; fi
    find_cargo || die "Termux rust package did not provide cargo"
    return
  fi

  if [ "$OS" = "Darwin" ]; then
    if ! xcode-select -p >/dev/null 2>&1; then
      die "install the Xcode command line tools first: xcode-select --install"
    fi
  elif have apt-get; then
    log "installing build packages with apt"
    sudo_cmd
    run $SUDO apt-get update
    run $SUDO apt-get install -y curl ca-certificates build-essential perl
  elif have dnf; then
    log "installing build packages with dnf"
    sudo_cmd
    run $SUDO dnf install -y curl ca-certificates gcc gcc-c++ make perl
  elif have pacman; then
    log "installing build packages with pacman"
    sudo_cmd
    run $SUDO pacman -S --needed --noconfirm base-devel curl
  else
    warn "unknown package manager; assuming a compiler and curl exist"
  fi

  if [ -x "$HOME/.cargo/bin/rustup" ]; then
    run "$HOME/.cargo/bin/rustup" toolchain install stable --profile minimal
  else
    install_curl
    log "installing rustup"
    if [ "$DRY_RUN" = "1" ]; then
      log "would run the rustup installer from https://sh.rustup.rs"
    else
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
        sh -s -- -y --profile minimal --default-toolchain stable
    fi
  fi

  if [ "$DRY_RUN" = "1" ]; then return 0; fi
  if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
  fi
  PATH="$HOME/.cargo/bin:$PATH"
  export PATH
  find_cargo || die "rustup finished but cargo is still missing"
}

# ---------------------------------------------------------------- build/install
resolve_destination() {
  if [ -n "$PREFIX_DIR" ]; then
    DEST="$PREFIX_DIR/bin"
  elif [ "$TERMUX" = "1" ]; then
    DEST="${PREFIX:-/data/data/com.termux/files/usr}/bin"
  else
    DEST="$HOME/.local/bin"
  fi
}

add_to_path() {
  local rc=""
  case "${SHELL:-}" in
    */zsh) rc="$HOME/.zshrc" ;;
    *) rc="$HOME/.bashrc" ;;
  esac
  [ -f "$rc" ] || rc="$HOME/.profile"
  if grep -qs "termagent installer" "$rc" 2>/dev/null; then
    return
  fi
  if [ "$DRY_RUN" = "1" ]; then
    log "would add $DEST to PATH in $rc"
    return
  fi
  printf '\n# termagent installer\nexport PATH="%s:$PATH"\n' "$DEST" >>"$rc"
  log "added $DEST to PATH in $rc (open a new shell)"
}

main() {
  log "termagent installer ($OS$( [ "$TERMUX" = "1" ] && printf ', termux'))"

  resolve_source
  if [ -z "$ROOT" ] && [ "$DRY_RUN" = "1" ]; then
    resolve_destination
    log "dry run complete"
    return
  fi

  if ! find_cargo; then
    install_rust
  fi
  if [ "$DRY_RUN" = "1" ] && ! have cargo; then
    resolve_destination
    log "dry run complete (toolchain would be installed first)"
    return
  fi
  CARGO="$(command -v cargo)"
  log "cargo: $CARGO"

  resolve_destination

  local jobs="${TERMAGENT_JOBS:-}"
  if [ -z "$jobs" ]; then
    jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || nproc 2>/dev/null || echo 2)"
    if [ "$TERMUX" = "1" ] && [ "$jobs" -gt 2 ]; then
      jobs=2
    fi
  fi
  log "building release (jobs=$jobs, this takes a few minutes on a phone)"
  run env CARGO_BUILD_JOBS="$jobs" "$CARGO" build --release --locked --bin termagent --manifest-path "$ROOT/Cargo.toml"

  if [ "$DRY_RUN" = "1" ]; then
    log "would install to $DEST/termagent"
    log "dry run complete"
    return
  fi

  [ -x "$ROOT/target/release/termagent" ] || die "build finished without target/release/termagent"
  mkdir -p "$DEST"
  cp "$ROOT/target/release/termagent" "$DEST/termagent"
  chmod 755 "$DEST/termagent"

  local version
  version="$("$DEST/termagent" --version)"
  log "installed: $DEST/termagent ($version)"

  case ":$PATH:" in
    *":$DEST:"*) ;;
    *)
      if [ "$ADD_PATH" = "1" ]; then
        add_to_path
      else
        warn "$DEST is not on PATH. Add it with:"
        warn "  export PATH=\"$DEST:\$PATH\""
        warn "or rerun with --add-path"
      fi
      ;;
  esac

  log "start with: termagent"
  log "browser UI: termagent --web  (http://127.0.0.1:6033)"
}

main
