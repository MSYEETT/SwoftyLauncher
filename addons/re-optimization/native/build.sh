#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"

if command -v zig >/dev/null; then
    ZIG=(zig)
else
    ZIG=(nix run nixpkgs#zig --)
fi

export ZIG_GLOBAL_CACHE_DIR=$PWD/build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/build/zig-cache
mkdir -p build

SDK=../../../packages/sdk/include
PIN=()
command -v taskset >/dev/null && PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)

"${PIN[@]}" "${ZIG[@]}" c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -I$SDK -D__USE_MINGW_ANSI_STDIO=1 \
    -shared -s -o re_optimization.dll \
    src/re_optimization.cpp src/delta.cpp src/overrides.cpp src/gpu.cpp

rm -f re_optimization.lib re_optimization.pdb
echo "built $(pwd)/re_optimization.dll"
