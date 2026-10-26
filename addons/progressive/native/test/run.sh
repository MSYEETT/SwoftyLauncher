#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"

if command -v zig >/dev/null; then
    ZIG=(zig)
else
    ZIG=(nix run nixpkgs#zig --)
fi

OUT=${PG_TEST_DIR:-${TMPDIR:-/tmp}/progressive-test}
mkdir -p "$OUT"
export ZIG_GLOBAL_CACHE_DIR=$OUT/zig-cache ZIG_LOCAL_CACHE_DIR=$OUT/zig-cache

"${ZIG[@]}" c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness -I../src \
    -o "$OUT/lineup_logic_test" lineup_logic_test.cpp
"$OUT/lineup_logic_test"

"${ZIG[@]}" c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness -I../src \
    -o "$OUT/season_logic_test" season_logic_test.cpp
"$OUT/season_logic_test" ../../data/years.json

"${ZIG[@]}" c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness -I../src \
    -o "$OUT/resume_logic_test" resume_logic_test.cpp
"$OUT/resume_logic_test"
