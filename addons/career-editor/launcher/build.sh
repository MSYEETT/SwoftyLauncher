#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
cargo build --release
mkdir -p ../bin
cp target/release/career-editor-helper ../bin/
if command -v cargo-zigbuild >/dev/null && rustup target list --installed 2>/dev/null | grep -q x86_64-pc-windows-gnu; then
    cargo zigbuild --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/career-editor-helper.exe ../bin/
fi
echo "built $(cd .. && pwd)/bin"
