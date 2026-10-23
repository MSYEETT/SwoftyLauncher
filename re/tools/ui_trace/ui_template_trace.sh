#!/usr/bin/env bash
# ui_template_trace.sh [log file]
# Attaches gdb to the running F1_25.exe (Steam build) and logs UI template load and instantiation
# failures by name while you navigate. Ctrl+C ends the trace; gdb detaches and the game keeps running.
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
LOG=${1:-$PWD/ui_template_trace.$(date +%Y%m%d-%H%M%S).log}
PID=""
for p in $(pgrep -f 'F1_25.exe' 2>/dev/null); do
    if grep -qi '/F1 25/F1_25.exe' "/proc/$p/maps" 2>/dev/null; then PID=$p; break; fi
done
[ -n "$PID" ] || { echo "ui_template_trace: F1_25.exe is not running" >&2; exit 1; }
PRIOR=$(cat /proc/sys/kernel/yama/ptrace_scope)
restore() {
    sudo -n sysctl -w "kernel.yama.ptrace_scope=$PRIOR" >/dev/null 2>&1 || true
    if [ "$(awk '{print $3}' "/proc/$PID/stat" 2>/dev/null)" = "T" ]; then kill -CONT "$PID" 2>/dev/null || true; fi
    echo "ui_template_trace: ptrace_scope=$(cat /proc/sys/kernel/yama/ptrace_scope), game state $(awk '{print $3}' "/proc/$PID/stat" 2>/dev/null), log $LOG" >&2
}
trap restore EXIT
sudo -n sysctl -w kernel.yama.ptrace_scope=0 >/dev/null
echo "ui_template_trace: attaching to F1_25.exe pid $PID, log $LOG" >&2
GDB=$(nix build --no-link --print-out-paths nixpkgs#gdb)/bin/gdb
timeout -s INT 3600 taskset -c 2-7,10,11,14-21 "$GDB" -p "$PID" --batch -x "$HERE/ui_template_trace.gdb" 2>&1 \
    | grep --line-buffered -vE '^\[New LWP|^\[Thread deb|libthread_db|Using host|not compatible with reported target|^0x[0-9a-f]+ in ' \
    | tee "$LOG"
