#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v wine >/dev/null || ! command -v xvfb-run >/dev/null || ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig nixpkgs#wineWow64Packages.stable nixpkgs#xvfb-run -c "$SELF" "$@"
fi
DIST=${DIST:-dist}
OUT=${1:-$PWD/../build/wine-career-ui}
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
rm -rf "$OUT/game"
mkdir -p "$OUT/game"
"${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -o "$OUT/game/F1_25.exe" fake_career_ui.c -lversion -luser32
cp -r "../$DIST/." "$OUT/game/"
"${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -I../../packages/sdk/include -shared -s \
    -o "$OUT/game/swofty/addons/t_career_ui.dll" career_ui/t_career_ui.cpp
python3 - "$OUT/game/swofty/addons/core/bindings.json" <<'EOB'
import json, sys
tag = lambda t: '48 B8 53 57 46 54 ' + ' '.join(f'{ord(c):02X}' for c in t)
func = lambda i, t: {'id': i, 'kind': 'func', 'status': 'experimental', 'section': '.xtls', 'pattern': tag(t),
                     'anchor': 0, 'resolve': {'kind': 'addr'}}
data = lambda i, t: {'id': i, 'kind': 'data', 'status': 'experimental', 'section': '.xtls',
                     'pattern': tag(t) + ' 48 8D 05 ?? ?? ?? ??', 'anchor': 13, 'resolve': {'kind': 'riprel', 'insn': 10, 'len': 7}}
mine = [func('Game_EntrantsUi_BuildCharacterRef', 'CREF'), func('Game_EntrantsUi_OnCharacterListLoaded', 'CLST'),
        func('Game_SpeechSystem_PostLine', 'SPCH'), func('Game_CareerWizard_StartsInF1', 'STF1'),
        func('FeTile_Route_CareerItem', 'ROUT'), func('Game_CareerWizard_PushNextStep', 'PUSH'),
        func('Game_DriverSelectHeader_SetTabState', 'TSTA'), func('Ne_Make_StrFromCStr', 'NSTR'),
        {'id': 'site_Game_CareerWizardConfirm_NextRoute', 'kind': 'site', 'status': 'experimental', 'section': '.xtls',
         'pattern': tag('CONF'), 'anchor': 0, 'resolve': {'kind': 'addr'}},
        {'id': 'site_Game_DriverSelectHeader_BuildTabs', 'kind': 'call', 'status': 'experimental', 'section': '.xtls',
         'pattern': tag('TABS') + ' E8 ?? ?? ?? ??', 'anchor': 10, 'resolve': {'kind': 'addr'}, 'expect': 'E8'},
        data('g_Game_Manager', 'GMGR'), data('k_Career_CareerSetupComponentDesc', 'DESC')]
doc = json.load(open(sys.argv[1]))
ids = {b['id'] for b in mine}
doc['bindings'] = [b for b in doc['bindings'] if b['id'] not in ids] + mine
json.dump(doc, open(sys.argv[1], 'w'), indent=1)
EOB
export WINEPREFIX="$OUT/prefix" WINEDLLOVERRIDES="version=n,b;mscoree,mshtml=;winemenubuilder.exe,winedbg.exe=d" WINEDEBUG=-all
status=0
timeout 120 "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe" || status=1
LOG=$OUT/game/swofty/loader.log
check() {
    if grep -qE "$2" "$LOG"; then echo "ok  : $1"; else echo "FAIL: $1"; status=1; fi
}
check "the character hooks are core mixin sets of their own" "core mixin set 'swofty_core\.dll \(character refs\)': 2 mixin"
check "every wizard hook is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(wizard driver tabs\)': 2 mixin"
check "characters setup armed both parts" '\[characters\] setup: character refs armed, voices armed'
check "wizard setup armed every part" '\[wizard\] setup: start in F1 armed, tile route armed, steps and options armed, confirm route armed, driver tabs armed'
check "refresh rebuilt the list with one stand-in" '\[characters\] refresh; character list rebuilt \(1 stand-in ref\(s\)\)'
check "the group's strings were applied or reported stock" "\[characters\] (strings: cleared 0, set 2 for 'f1_career_1990'|2 string\(s\) for 'f1_career_1990' stay stock)"
check "the agent line was logged as muted" '\[characters\] agent line muted'
check "the tile route swap was logged" '\[wizard\] f1_career_1990 tile opens career_wizard_driver_career_1990 instead of career_wizard_driver_career_to_f1'
[[ $status == 0 ]] && echo "career ui test: ALL PASSED" || { echo "career ui test: FAILED"; tail -60 "$LOG"; }
exit $status
