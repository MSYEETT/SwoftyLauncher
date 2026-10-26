#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v wine >/dev/null || ! command -v xvfb-run >/dev/null || ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig nixpkgs#wineWow64Packages.stable nixpkgs#xvfb-run -c "$SELF" "$@"
fi
DIST=${DIST:-dist}
OUT=${1:-$PWD/../build/wine-net-trace}
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
rm -rf "$OUT/game"
mkdir -p "$OUT/game"
"${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -o "$OUT/game/F1_25.exe" fake_net.c -lversion -luser32 -lws2_32
cp -r "../$DIST/." "$OUT/game/"
export WINEPREFIX="$OUT/prefix" WINEDLLOVERRIDES="version=n,b;mscoree,mshtml=;winemenubuilder.exe,winedbg.exe=d" WINEDEBUG=-all
status=0
LOG=$OUT/game/swofty/net-trace.log
check() {
    if grep -qE "$2" "$LOG"; then echo "ok  : $1"; else echo "FAIL: $1"; status=1; fi
}
count() {
    local n
    n=$(grep -cE "$3" "$LOG" || true)
    if [[ $n == "$2" ]]; then echo "ok  : $1"; else echo "FAIL: $1 (logged $n times, want $2)"; status=1; fi
}
run() {
    rm -f "$LOG" "$OUT/game/ports.txt"
    (cd "$OUT/game" && timeout 120 "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe") || status=1
}
for mode in off 1 inline; do
    echo "==== SWOFTY_NET_TRACE=$mode"
    if [[ $mode == off ]]; then
        unset SWOFTY_NET_TRACE
        run
        [[ ! -e $LOG ]] && echo "ok  : no net-trace.log without the variable" || { echo "FAIL: net-trace.log written while off"; status=1; }
        continue
    fi
    export SWOFTY_NET_TRACE=$mode
    run
    if [[ ! -s $LOG ]]; then
        echo "FAIL: no net-trace.log"
        status=1
        continue
    fi
    read -r RPORT SPORT <"$OUT/game/ports.txt" || true
    if [[ $mode == 1 ]]; then
        check "header names iat mode and 16 data bytes" '^# swofty net trace: .* mode iat, first 16 data bytes'
        check "the fake's WSARecvFrom import slot is hooked in the IAT" '^# iat table 1 slot exe\+0x[0-9a-f]+ \(name\) -> WSARecvFrom$'
        check "WSASendTo is not imported, so it is hooked inline" '^# inline hook on ws2_32!WSASendTo'
        check "WSAEventSelect is not imported, so it is hooked inline" '^# inline hook on ws2_32!WSAEventSelect'
    else
        check "header names inline mode" '^# swofty net trace: .* mode inline'
        check "WSARecvFrom is hooked inline" '^# inline hook on ws2_32!WSARecvFrom'
        count "no IAT slot is touched in inline mode" 0 '^# iat table'
    fi
    check "every call is hooked" '^# ready: .*not hooked: none$'
    check "WSASocketW logged with family, type and overlapped flag" ' WSASocketW #1 sock=0x[0-9a-f]+ af=2 type=2 proto=17 .*af=2 type=2 proto=17 flags=0x1 ret=0 err=0 from=exe\+0x'
    check "socket logged" ' socket #1 sock=0x[0-9a-f]+ af=2 type=2 proto=17 '
    check "setsockopt logged with the value bytes" ' setsockopt #1 .* level=0xffff name=0x1002 len=4 val=00000100 ret=0'
    check "ioctlsocket FIONBIO logged" ' ioctlsocket #1 .* cmd=0x8004667e arg=1 ret=0'
    check "bind logged with the address" " bind #1 .* addr=127\.0\.0\.1:0 len=16 ret=0 "
    check "the failed bind keeps its error" " bind #1 .* addr=127\.0\.0\.1:$RPORT len=16 ret=-1 err=10048 "
    check "getsockname logged with the bound port" " getsockname #1 sock=.* addr=127\.0\.0\.1:$RPORT ret=0"
    check "WSACreateEvent logged" ' WSACreateEvent #1 .* event=0x[0-9a-f]+ ret=0 '
    check "the overlapped WSARecvFrom logged pending with its buffers" " WSARecvFrom #1 sock=.* local=127\.0\.0\.1:$RPORT .* cap=1264 from_buf=0x[0-9a-f]+ fromlen=16 .* ov=0x[0-9a-f]+ ev=0x[0-9a-f]+ .* ret=-1 err=997 "
    check "sendto logged with the destination and data" " sendto #1 sock=.* to=127\.0\.0\.1:$RPORT len=16 flags=0x0 data=$(printf swofty-net-trace | xxd -p) ret=16 err=0 from=exe\+0x"
    check "WSAWaitForMultipleEvents logged the signalled event" ' WSAWaitForMultipleEvents #1 .* n=1 events=0x[0-9a-f]+ all=0 timeout=5000 signalled=0x[0-9a-f]+ ret=0 '
    check "WSAGetOverlappedResult logged bytes, data and the from address" " WSAGetOverlappedResult #1 .* bytes=16 flags=0x0 buf=0x[0-9a-f]+ data=$(printf swofty-net-trace | xxd -p) from_buf=0x[0-9a-f]+ from=127\.0\.0\.1:$SPORT ret=1 err=0 "
    check "WSASendTo logged" " WSASendTo #1 .* to=127\.0\.0\.1:$RPORT nbufs=1 len0=16 sent=16 data=$(printf via-WSASendTo!!! | xxd -p) .* ret=0 "
    check "recvfrom logged with the from address" " recvfrom #1 .* cap=64 flags=0x0 from=127\.0\.0\.1:$SPORT data=$(printf via-WSASendTo!!! | xxd -p) ret=16 "
    check "connect logged" " connect #1 .* addr=127\.0\.0\.1:$RPORT len=16 ret=0 "
    check "send logged with the connected peer" " send #1 sock=.* peer=127\.0\.0\.1:$RPORT len=16 flags=0x0 data=$(printf connected-send!! | xxd -p) ret=16 "
    check "recv logged" " recv #1 .* data=$(printf connected-send!! | xxd -p) ret=16 "
    check "WSAEventSelect logged" ' WSAEventSelect #1 .* event=0x[0-9a-f]+ mask=0x1 ret=0 '
    check "closesocket logged the socket it closed" " closesocket #[0-9]+ - sock=0x[0-9a-f]+ af=2 type=2 proto=17 local=127\.0\.0\.1:$RPORT .* ret=0 "
    check "the failed closesocket keeps WSAENOTSOCK" ' closesocket #[0-9]+ .* ret=-1 err=10038 '
    count "450 sendto calls on one socket log the first 200, then #300 and #400" 202 ' sendto #[0-9]+ sock=.* data=72617465 '
    check "the 300th call carries its count" ' sendto #300 .* data=72617465 '
done
[[ $status == 0 ]] && echo "net trace test: ALL PASSED" || { echo "net trace test: FAILED"; tail -40 "$LOG" 2>/dev/null || true; }
exit $status
