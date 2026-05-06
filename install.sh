#!/usr/bin/env sh
# install.sh — build and install gol into a prefix on your PATH.
#
# Usage:
#   ./install.sh                 # installs to /usr/local/bin (may sudo)
#   PREFIX=$HOME/.local ./install.sh
#   ./install.sh --uninstall

set -eu

PREFIX="${PREFIX:-/usr/local}"
BINDIR="$PREFIX/bin"
ACTION="install"

for arg in "$@"; do
    case "$arg" in
        -u|--uninstall) ACTION="uninstall" ;;
        -h|--help)
            cat <<EOF
install.sh — build and install gol

Options:
  -u, --uninstall   remove gol from \$PREFIX/bin
  -h, --help        show this help

Environment:
  PREFIX            install prefix (default: /usr/local)
EOF
            exit 0
            ;;
        *) printf 'unknown argument: %s\n' "$arg" >&2; exit 2 ;;
    esac
done

SUDO=""
if [ ! -w "$BINDIR" ] && [ "$(id -u)" -ne 0 ]; then
    if command -v sudo >/dev/null 2>&1; then
        SUDO="sudo"
    else
        printf 'error: cannot write to %s and sudo is not available\n' "$BINDIR" >&2
        printf 'hint:  re-run with PREFIX=$HOME/.local ./install.sh\n' >&2
        exit 1
    fi
fi

if [ "$ACTION" = "uninstall" ]; then
    $SUDO rm -f "$BINDIR/gol"
    printf 'removed %s/gol\n' "$BINDIR"
    exit 0
fi

if ! command -v cargo >/dev/null 2>&1; then
    printf 'error: cargo not found. install rust from https://rustup.rs\n' >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

printf 'building release binary...\n'
cargo build --release --quiet

$SUDO install -d "$BINDIR"
$SUDO install -m 0755 "$SCRIPT_DIR/target/release/gol" "$BINDIR/gol"

printf 'installed gol to %s/gol\n' "$BINDIR"
printf 'try:    gol --help\n'
