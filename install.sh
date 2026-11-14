#!/usr/bin/env sh
# Installs termagent into $TERMAGENT_PREFIX/bin (default: ~/.local/bin).
# Builds from source with cargo. No sudo, no system directories.
set -eu

PREFIX="${TERMAGENT_PREFIX:-$HOME/.local}"
cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required: https://rustup.rs" >&2
  echo "on Termux: pkg install rust" >&2
  exit 1
fi

if [ "$(id -u)" -eq 0 ]; then
  echo "note: termagent does not need root; installing for the root user anyway" >&2
fi

echo "building termagent (release)"
cargo build --release --bin termagent

mkdir -p "$PREFIX/bin"
install -m 755 target/release/termagent "$PREFIX/bin/termagent"
echo "installed $PREFIX/bin/termagent"
case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *) echo "add $PREFIX/bin to PATH to run 'termagent'" ;;
esac
