#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
mkdir -p ../bin
if command -v cargo-zigbuild >/dev/null; then
    cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.31
    cp target/x86_64-unknown-linux-gnu/release/driver-manager-helper ../bin/
else
    cargo build --release
    cp target/release/driver-manager-helper ../bin/
fi
if command -v cargo-zigbuild >/dev/null && rustup target list --installed 2>/dev/null | grep -q x86_64-pc-windows-gnu; then
    cargo zigbuild --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/driver-manager-helper.exe ../bin/
fi
echo "built $(cd .. && pwd)/bin"
