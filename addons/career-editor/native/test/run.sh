#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
OUT=${CE_TEST_DIR:-${TMPDIR:-/tmp}}
mkdir -p "$OUT"
g++ -std=c++23 -O1 -Wall -Wextra -Wno-misleading-indentation -include unistd.h -I../src -I../../../../packages/sdk/include \
    -o "$OUT/ce_bridge_harness" bridge_harness.cpp ../src/bridge.cpp
CE_TEST_DIR="$OUT" "$OUT/ce_bridge_harness"
