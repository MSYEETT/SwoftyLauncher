#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"

if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/build/zig-cache

VARIANT=${VARIANT:-}
case $VARIANT in
'') DEFS=() ;;
nomodsettings) DEFS=(-DSWOFTY_NO_MOD_SETTINGS) ;;
hooknoop) DEFS=(-DSWOFTY_MOD_SETTINGS_NOOP) ;;
*)
    echo "build.sh: unknown VARIANT '$VARIANT' (known: nomodsettings, hooknoop)" >&2
    exit 2
    ;;
esac
DIST=dist${VARIANT:+-$VARIANT}
OUT=build${VARIANT:+/$VARIANT}

SDK=../packages/sdk/include
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
CC=(zig cc -target x86_64-windows-gnu)
CXX=(zig c++ -target x86_64-windows-gnu -std=c++23)
SH=third_party/safetyhook
STATIC=(-s)
mkdir -p build "$OUT"
rm -rf "$DIST"
mkdir -p "$DIST/swofty/mods/core"
python3 ../re/tools/bindgen.py --json "$DIST/swofty/mods/core/bindings.json" --symbols "$DIST/swofty/mods/core/symbols.tsv"

"${PIN[@]}" "${CC[@]}" -O2 -Wall -Wextra -I$SDK -Iinclude -D__USE_MINGW_ANSI_STDIO=1 -shared \
    -o "$OUT/version.dll" src/loader/version.c src/loader/version.def "${STATIC[@]}" &

[[ build/Zydis.o -nt $SH/Zydis.c ]] || "${PIN[@]}" "${CC[@]}" -O2 -I$SH -c $SH/Zydis.c -o build/Zydis.o &
[[ build/safetyhook.o -nt $SH/safetyhook.cpp ]] || "${PIN[@]}" "${CXX[@]}" -O2 -I$SH -c $SH/safetyhook.cpp -o build/safetyhook.o &
wait

"${PIN[@]}" "${CXX[@]}" -O2 -Wall -Wextra "${DEFS[@]}" -I$SDK -Iinclude -I$SH -shared -o "$OUT/swofty_core.dll" \
    src/core/manager.cpp src/core/crash.cpp src/core/settings.cpp src/core/swofty_core.cpp src/core/ui.cpp src/core/ui_vm.cpp src/core/items.cpp build/safetyhook.o \
    build/Zydis.o "${STATIC[@]}"

"${PIN[@]}" "${CXX[@]}" -O2 -Wall -Wextra -I$SDK -Iinclude -shared -o "$OUT/swofty_gamelog.dll" src/gamelog/gamelog.cpp "${STATIC[@]}"

cp "$OUT/version.dll" "$DIST/version.dll"
cp "$OUT/swofty_core.dll" "$DIST/swofty/mods/core/swofty_core.dll"
cp "$OUT/swofty_gamelog.dll" "$DIST/swofty/mods/core/swofty_gamelog.dll"
cp src/gamelog/log_channels.tsv "$DIST/swofty/mods/core/log_channels.tsv"

for f in "$DIST/version.dll" "$DIST/swofty/mods/core/swofty_core.dll" "$DIST/swofty/mods/core/swofty_gamelog.dll"; do
    echo "== $f"
    objdump -p "$f" | grep -E '^(Magic|Characteristics)|file format|DLL Name|^\s+\[ *[0-9]+\] \+base' || true
    objdump -f "$f" | grep -E 'architecture'
    md5sum "$f"
done
md5sum "$DIST/swofty/mods/core/bindings.json" "$DIST/swofty/mods/core/symbols.tsv" "$DIST/swofty/mods/core/log_channels.tsv"
