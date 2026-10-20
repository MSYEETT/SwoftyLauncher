#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
mkdir -p ../build
${CXX:-g++} -std=c++20 -O1 -Wall -Wextra -I../src -o ../build/grid_rank_test grid_rank_test.cpp
../build/grid_rank_test
${CXX:-g++} -std=c++20 -O1 -Wall -Wextra -I../src -o ../build/contract_target_test contract_target_test.cpp
../build/contract_target_test
${CXX:-g++} -std=c++20 -O1 -Wall -Wextra -I../src -o ../build/save_name_test save_name_test.cpp
../build/save_name_test
