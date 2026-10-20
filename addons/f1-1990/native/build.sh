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

REPO=../../..
SDK=$REPO/packages/sdk/include
if [[ -z ${GAME_INCLUDE:-} ]]; then
    DEV=build/dev
    $REPO/re/tools/py $REPO/re/tools/bindgen.py --dev --no-exe --json $DEV/bindings.json --include $DEV/include/swofty \
        --rust $DEV/game.rs --symbols $DEV/symbols.tsv >/dev/null
    GAME=$DEV/include
else
    GAME=$GAME_INCLUDE
fi
CXX=("${ZIG[@]}" c++ -target x86_64-windows-gnu -std=c++23)
SRC=(src/grid_best_of.cpp)
[[ ${CONTRACT_SKIP:-0} == 1 ]] && SRC+=(src/contract_skip.cpp)

"${CXX[@]}" -O2 -Wall -Wextra -I"$GAME" -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o f1_1990.dll "${SRC[@]}"

rm -f f1_1990.lib
echo "built $(pwd)/f1_1990.dll"
