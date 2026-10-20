#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
mkdir -p ../build
${CXX:-g++} -std=c++20 -O1 -Wall -Wextra -I../src -o ../build/grid_rank_test grid_rank_test.cpp
../build/grid_rank_test
