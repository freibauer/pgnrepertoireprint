#!/usr/bin/env bash
# Builds pgnrepertoireprint for Windows and Linux into dist/.
#
# One-time setup:
#   rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-gnu
#   sudo apt install gcc-mingw-w64-x86-64
#
# Usage: ./build.sh [windows|linux|all]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
mkdir -p dist

build_windows() {
    RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/pgnrepertoireprint.exe dist/pgnrepertoireprint.exe
    echo "==> dist/pgnrepertoireprint.exe"
}

build_linux() {
    cargo build --release --target x86_64-unknown-linux-gnu
    cp target/x86_64-unknown-linux-gnu/release/pgnrepertoireprint dist/pgnrepertoireprint-linux
    chmod +x dist/pgnrepertoireprint-linux
    echo "==> dist/pgnrepertoireprint-linux"
}

case "${1:-all}" in
    windows) build_windows ;;
    linux)   build_linux ;;
    all)     build_windows; build_linux ;;
    *) echo "Usage: $0 [windows|linux|all]" >&2; exit 1 ;;
esac
