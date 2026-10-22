#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
mkdir -p ../bin
if command -v cargo-zigbuild >/dev/null; then
    cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17
    cp target/x86_64-unknown-linux-gnu/release/career-editor-helper ../bin/
else
    echo "cargo-zigbuild not found: the Linux helper links against this system's glibc and may not run elsewhere" >&2
    cargo build --release
    cp target/release/career-editor-helper ../bin/
fi
if command -v cargo-zigbuild >/dev/null && rustup target list --installed 2>/dev/null | grep -q x86_64-pc-windows-gnu; then
    cargo zigbuild --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/career-editor-helper.exe ../bin/
fi
echo "built $(cd .. && pwd)/bin"
