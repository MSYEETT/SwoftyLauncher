#!/usr/bin/env bash
# Builds the recorder swofty dev pack runs: dist/swofty_record.exe and its dist/swofty_core.dll.
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
OUT=${1:-$PWD/dist}
mkdir -p "$OUT" "$PWD/../../../native/build/zig-cache"
export ZIG_GLOBAL_CACHE_DIR=$PWD/../../../native/build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../../../native/build/zig-cache
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
"${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -Wall -Wextra -Wno-nullability-completeness -I../include -shared -s -o "$OUT/swofty_core.dll" core_stub.c &
"${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -Wno-nullability-completeness -I../include -s \
    -o "$OUT/swofty_record.exe" recorder.cpp -lshell32 &
wait
rm -f "$OUT"/*.lib "$OUT"/*.pdb
echo "built $OUT/swofty_record.exe"
