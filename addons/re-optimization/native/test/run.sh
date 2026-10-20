#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"

if command -v zig >/dev/null; then
    ZIG=(zig)
else
    ZIG=(nix run nixpkgs#zig --)
fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
mkdir -p ../build

"${ZIG[@]}" c++ -target x86_64-linux-musl -std=c++23 -O2 -Wall -Wextra -Werror -o ../build/reopt_tests \
    tests.cpp ../src/delta.cpp ../src/overrides.cpp ../src/gpu.cpp
../build/reopt_tests sample_settings.xml
