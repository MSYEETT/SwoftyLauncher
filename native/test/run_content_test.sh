#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
OUT=${1:-$PWD/../build/content-test}
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
mkdir -p "$OUT"
SRC=(../src/core/registry/store.cpp ../src/core/registry/table.cpp ../src/core/registry/validate.cpp ../src/core/registry/schema.cpp ../src/core/registry/engine.cpp)
CXX=(zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness -I../../packages/sdk/include)
pids=()
"${PIN[@]}" "${CXX[@]}" -o "$OUT/lower_tool" content/lower_tool.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" "${CXX[@]}" -o "$OUT/rows_tool" content/rows_tool.cpp ../src/core/apply/ids.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" "${CXX[@]}" -o "$OUT/rows_test" content/rows_test.cpp ../src/core/apply/ids.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -I../../packages/sdk/include -I../include \
    -c ../src/core/apply/content/appliers.cpp -o "$OUT/appliers.windows.o" & pids+=($!)
for p in "${pids[@]}"; do wait "$p"; done
"$OUT/rows_test" ../../packages/sdk/std/registries
python3 content/check_lowering.py "$OUT/lower_tool"
