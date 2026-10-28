#!/usr/bin/env bash
# The test agent in a fake game: with the run's token in the environment and in session.json it arms,
# answers the commands in commands.jsonl through the flow tick and reports unbound commands as
# unavailable; with a different token it refuses; without SWOFTY_TEST it does nothing.
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v wine >/dev/null || ! command -v xvfb-run >/dev/null || ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig nixpkgs#wineWow64Packages.stable nixpkgs#xvfb-run -c "$SELF" "$@"
fi
. ./game_lock.sh
DIST=${DIST:-dist}
OUT=${1:-$PWD/../build/wine-testagent}
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
rm -rf "$OUT/game"
mkdir -p "$OUT/game"
"${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -o "$OUT/game/F1_25.exe" fake_testagent.c -lversion -luser32
cp -r "../$DIST/." "$OUT/game/"
export WINEPREFIX="$OUT/prefix" WINEDLLOVERRIDES="version=n,b;mscoree,mshtml=;winemenubuilder.exe,winedbg.exe=d" WINEDEBUG=-all
LOG=$OUT/game/swofty/loader.log
RUN=wt-1
BOX=$OUT/game/swofty/test/$RUN
RES=$BOX/results.jsonl
TOKEN=0123456789abcdef0123456789abcdef
status=0
check() {
    if grep -qE "$2" "${3:-$LOG}"; then echo "ok  : $1"; else echo "FAIL: $1"; status=1; fi
}
none() {
    if grep -qE "$2" "${3:-$LOG}" 2>/dev/null; then echo "FAIL: $1"; status=1; else echo "ok  : $1"; fi
}
mailbox() {
    rm -rf "$OUT/game/swofty/test"
    mkdir -p "$BOX"
    printf '{"v":1,"run":"%s","token":"%s","created_ms":%s,"dlls":[]}' "$RUN" "$1" "$(date +%s%3N)" >"$BOX/session.json"
    cat >"$BOX/commands.jsonl" <<'EOC'
{"seq":1,"cmd":"hello"}
{"seq":2,"cmd":"route","args":{"route":"career"}}
{"seq":3,"cmd":"read","args":{"path":"agent.ticks"}}
{"seq":4,"cmd":"read","args":{"path":"car[0].chassis.mass"}}
{"seq":5,"cmd":"set","args":{"path":"car[player].engine.max_torque","value":500}}
{"seq":6,"cmd":"wait","args":{"event":"race.start"},"timeout_ms":300}
{"seq":7,"cmd":"skip","args":{"to":"session"}}
{"seq":8,"cmd":"advance","args":{"days":7}}
{"seq":9,"cmd":"save","args":{"what":"now"}}
{"seq":10,"cmd":"pad","args":{"press":"a","ms":40}}
{"seq":11,"cmd":"bogus"}
EOC
}
run() {
    rm -f "$LOG"
    (cd "$OUT/game" && T_SECONDS=${T_SECONDS:-8} timeout 120 "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe") || status=1
}
for mode in armed refused off; do
    unset SWOFTY_TEST SWOFTY_TEST_TOKEN
    echo "==== $mode"
    case $mode in
    armed) mailbox "$TOKEN"; export SWOFTY_TEST=$RUN SWOFTY_TEST_TOKEN=$TOKEN ;;
    refused) mailbox fedcba9876543210fedcba9876543210; export SWOFTY_TEST=$RUN SWOFTY_TEST_TOKEN=$TOKEN ;;
    off) mailbox "$TOKEN" ;;
    esac
    run
    case $mode in
    armed)
        check "the agent arms on the matching token" "\[test\] agent armed for run $RUN: 0 test DLL\(s\)"
        check "the hello line names the run" '"event":"hello".*"run":"wt-1"' "$RES"
        check "hello answers" '"seq":1,"status":"ok","value":\{"v":1,' "$RES"
        check "route answers unavailable with the reason" '"seq":2,"status":"unavailable","error":"Vm_Fire_Route, Ne_Make_StrFromCStr or FeHome_Handle_Command did not resolve' "$RES"
        check "read agent.ticks answers from the flow tick" '"seq":3,"status":"ok","value":\{"value":[1-9][0-9]*,"type":"u64"\}' "$RES"
        check "a car read off the KB build is unavailable" '"seq":4,"status":"unavailable","error":"the vehicle table' "$RES"
        check "a car set off the KB build is unavailable" '"seq":5,"status":"unavailable","error":"the vehicle table' "$RES"
        check "a wait times out" '"seq":6,"status":"timeout"' "$RES"
        check "skip off the KB build is unavailable" '"seq":7,"status":"unavailable","error":"MessageSetAdvanceDestination' "$RES"
        check "advance is unavailable" '"seq":8,"status":"unavailable","error":"the Team HQ advance view model' "$RES"
        check "save now is unavailable" '"seq":9,"status":"unavailable","error":"no save trigger' "$RES"
        check "the pad answers" '"seq":10,"status":"(ok|unavailable)"' "$RES"
        check "an unknown command is an error" '"seq":11,"status":"error","error":"unknown command' "$RES"
        n=$(grep -c '"seq":' "$RES" || true)
        [[ $n == 11 ]] && echo "ok  : every command answered once" || { echo "FAIL: $n answers, want 11"; status=1; }
        ;;
    refused)
        check "a different token is refused in loader.log" "\[test\] agent refused: the token in session.json does not match SWOFTY_TEST_TOKEN"
        check "and in results.jsonl" '"event":"refused","why":"the token in session.json does not match' "$RES"
        none "nothing is answered" '"seq":' "$RES"
        none "nothing arms" '\[test\] agent armed'
        ;;
    off)
        none "no test agent line without SWOFTY_TEST" '\[test\]'
        none "no results without SWOFTY_TEST" '.' "$RES"
        ;;
    esac
done
exit $status
