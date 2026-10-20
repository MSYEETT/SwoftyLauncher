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

"${CXX[@]}" -O2 -Wall -Wextra -fms-extensions -I"$GAME" -I$SDK -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o career_editor.dll src/*.cpp

rm -f career_editor.lib
echo "built $(pwd)/career_editor.dll"
