#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v wine >/dev/null || ! command -v xvfb-run >/dev/null || ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig nixpkgs#wineWow64Packages.stable nixpkgs#xvfb-run -c "$SELF" "$@"
fi
OUT=${1:-$PWD/../build/wine-test}
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
rm -rf "$OUT/game"
mkdir -p "$OUT/game"
taskset -c 2-7,10,11,14-31 zig cc -target x86_64-windows-gnu -O2 -o "$OUT/game/F1_25.exe" fake_game.c -lversion -luser32
cp -r ../dist/. "$OUT/game/"
export WINEPREFIX="$OUT/prefix" WINEDLLOVERRIDES="version=n,b;mscoree,mshtml=;winemenubuilder.exe=d" WINEDEBUG=-all
status=0
for mode in trigger fallback vm; do
    echo "==== $mode"
    taskset -c 2-7,10,11,14-31 xvfb-run -a wine "$OUT/game/F1_25.exe" $mode || status=1
    echo "---- $OUT/game/swofty/loader.log ($mode)"
    grep -v '  section ' "$OUT/game/swofty/loader.log"
done
exit $status
