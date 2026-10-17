#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"

if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/build/zig-cache

SDK=../packages/sdk/include
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
CC=(zig cc -target x86_64-windows-gnu)
CXX=(zig c++ -target x86_64-windows-gnu -std=c++23)
SH=third_party/safetyhook
STATIC=(-s)
mkdir -p build

"${PIN[@]}" "${CC[@]}" -O2 -Wall -Wextra -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared \
    -o build/version.dll src/loader/version.c src/loader/version.def "${STATIC[@]}" &

[[ build/Zydis.o -nt $SH/Zydis.c ]] || "${PIN[@]}" "${CC[@]}" -O2 -I$SH -c $SH/Zydis.c -o build/Zydis.o &
[[ build/safetyhook.o -nt $SH/safetyhook.cpp ]] || "${PIN[@]}" "${CXX[@]}" -O2 -I$SH -c $SH/safetyhook.cpp -o build/safetyhook.o &
wait

"${PIN[@]}" "${CXX[@]}" -O2 -Wall -Wextra -I$SDK -I$SH -shared -o build/swofty_core.dll \
    src/core/swofty_core.cpp build/safetyhook.o build/Zydis.o "${STATIC[@]}"

rm -rf dist
mkdir -p dist/swofty/mods/core
cp build/version.dll dist/version.dll
cp build/swofty_core.dll dist/swofty/mods/core/swofty_core.dll

for f in dist/version.dll dist/swofty/mods/core/swofty_core.dll; do
    echo "== $f"
    objdump -p "$f" | grep -E '^(Magic|Characteristics)|file format|DLL Name|^\s+\[ *[0-9]+\] \+base' || true
    objdump -f "$f" | grep -E 'architecture'
    md5sum "$f"
done
