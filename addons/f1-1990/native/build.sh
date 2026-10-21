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
SRC=(src/plugin.cpp src/grid_best_of.cpp src/save_separation.cpp src/contract_skip.cpp)

"${CXX[@]}" -O2 -Wall -Wextra -I"$GAME" -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o f1_1990.dll "${SRC[@]}"

"${CXX[@]}" -O2 -Wall -Wextra -I"$GAME" -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o f1_1990_tyres.dll src/fq_tyres.cpp

"${CXX[@]}" -O2 -Wall -Wextra -I"$GAME" -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o f1_1990_replay.dll src/replay_holders.cpp

"${CXX[@]}" -O2 -Wall -Wextra -I"$GAME" -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o f1_1990_ui.dll src/career_menu_ui.cpp

"${CXX[@]}" -O2 -Wall -Wextra -I"$GAME" -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o f1_1990_voice.dll src/agent_voice.cpp

"${CXX[@]}" -O2 -Wall -Wextra -I"$GAME" -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o f1_1990_bspec.dll src/bspec.cpp

rm -f f1_1990.lib f1_1990_tyres.lib f1_1990_replay.lib f1_1990_ui.lib f1_1990_voice.lib f1_1990_bspec.lib
echo "built $(pwd)/f1_1990.dll $(pwd)/f1_1990_tyres.dll $(pwd)/f1_1990_replay.dll $(pwd)/f1_1990_ui.dll $(pwd)/f1_1990_voice.dll $(pwd)/f1_1990_bspec.dll"
