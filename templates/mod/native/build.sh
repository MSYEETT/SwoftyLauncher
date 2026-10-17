#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/build/zig-cache
mkdir -p build
zig cc -target x86_64-windows-gnu -O2 -Wall -Wextra -Iinclude -shared -s \
    -o sample_plugin.dll src/sample_plugin.c
rm -f sample_plugin.lib
echo "built $(pwd)/sample_plugin.dll"
