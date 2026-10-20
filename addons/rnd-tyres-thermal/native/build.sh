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
GAME=${GAME_INCLUDE:-$REPO/native/include}
CXX=("${ZIG[@]}" c++ -target x86_64-windows-gnu -std=c++23)

"${CXX[@]}" -O2 -Wall -Wextra -I$GAME -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o rnd_departments.dll src/rnd_departments.cpp src/rnd_fifth_pie.cpp src/rnd_capacity.cpp src/rnd_dept_tags.cpp src/rnd_tyres_icon.cpp src/rnd_upgrade_types.cpp

rm -f rnd_departments.lib
echo "built $(pwd)/rnd_departments.dll"
