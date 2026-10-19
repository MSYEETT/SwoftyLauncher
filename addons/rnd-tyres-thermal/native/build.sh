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
SH=$REPO/native/third_party/safetyhook
CC=("${ZIG[@]}" cc -target x86_64-windows-gnu)
CXX=("${ZIG[@]}" c++ -target x86_64-windows-gnu -std=c++23)

[[ build/Zydis.o -nt $SH/Zydis.c ]] || "${CC[@]}" -O2 -I$SH -c $SH/Zydis.c -o build/Zydis.o
[[ build/safetyhook.o -nt $SH/safetyhook.cpp ]] || "${CXX[@]}" -O2 -I$SH -c $SH/safetyhook.cpp -o build/safetyhook.o

"${CXX[@]}" -O2 -Wall -Wextra -I$SDK -I$SH -D__USE_MINGW_ANSI_STDIO=1 -shared -s \
    -o rnd_departments.dll src/rnd_departments.cpp src/rnd_fifth_pie.cpp src/rnd_capacity.cpp src/rnd_dept_tags.cpp src/rnd_tyres_icon.cpp src/rnd_upgrade_types.cpp build/safetyhook.o build/Zydis.o

rm -f rnd_departments.lib
echo "built $(pwd)/rnd_departments.dll"
