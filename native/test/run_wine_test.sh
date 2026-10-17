#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v wine >/dev/null || ! command -v xvfb-run >/dev/null || ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig nixpkgs#wineWow64Packages.stable nixpkgs#xvfb-run -c "$SELF" "$@"
fi
OUT=${1:-$PWD/../build/wine-test}
ITEMS=
grep -qa '\[items\] ownership check hooked' ../dist/swofty/addons/core/swofty_core.dll && ITEMS=1
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
rm -rf "$OUT/game"
mkdir -p "$OUT/game"
"${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -o "$OUT/game/F1_25.exe" fake_game.c -lversion -luser32 &
cp -r ../dist/. "$OUT/game/"
mkdir -p "$OUT/game/swofty/mods/core"
mv "$OUT/game/swofty/addons/core/swofty_gamelog.dll" "$OUT/game/swofty/addons/core/log_channels.tsv" "$OUT/game/swofty/mods/core/"
for m in addons/t_*.cpp; do
    name=$(basename "${m%.cpp}")
    [[ $name == t_chain_b ]] && dir=mods || dir=addons
    "${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -I../../packages/sdk/include -I../include -shared -s \
        -o "$OUT/game/swofty/$dir/$name.dll" "$m" &
done
wait
cp "$OUT/game/swofty/addons/t_drop.dll" "$OUT/game/swofty/mods/t_drop.dll"
python3 - "$OUT/game/swofty/addons/core/bindings.json" bindings.test.json <<'EOF'
import json, sys
doc = json.load(open(sys.argv[1]))
doc['bindings'] += json.load(open(sys.argv[2]))
json.dump(doc, open(sys.argv[1], 'w'), indent=1)
EOF
python3 - "$OUT/game/F1_25.exe" "$OUT/game/swofty/mods/core/log_channels.tsv" <<'EOF'
import struct, sys
b = open(sys.argv[1], 'rb').read()
stamp = struct.unpack_from('<I', b, struct.unpack_from('<I', b, 0x3c)[0] + 8)[0]
open(sys.argv[2], 'w').write(f'# F1_25.exe fake {stamp}\n# hash\tsites\tchannels\n'
                             '0x7c7c88cb\t1\tTEST_WARN\n0x11111111\t1\tTEST_VERBOSE\n0x22222222\t1\tUI_FLOW\n')
EOF
export WINEPREFIX="$OUT/prefix" WINEDLLOVERRIDES="version=n,b;mscoree,mshtml=;winemenubuilder.exe=d" WINEDEBUG=-all
status=0
LOG=$OUT/game/swofty/loader.log
check() {
    if grep -qE "$2" "$LOG"; then echo "ok  : $1"; else echo "FAIL: $1"; status=1; fi
}
once() {
    local n
    n=$(grep -cE "$2" "$LOG" || true)
    if [[ $n == 1 ]]; then echo "ok  : $1"; else echo "FAIL: $1 (logged $n times)"; status=1; fi
}
count() {
    local n
    n=$(grep -cE "$3" "$LOG" || true)
    if [[ $n == "$2" ]]; then echo "ok  : $1"; else echo "FAIL: $1 (logged $n times, want $2)"; status=1; fi
}
for mode in trigger fallback vm settings settings_local session gamelog_off assets; do
    echo "==== $mode"
    [[ $mode == session ]] && cp session.test.json "$OUT/game/swofty/session.json"
    req=
    [[ $mode == settings ]] && req=$OUT/state/requests/wine-test.json
    [[ $mode == settings_local ]] && req=$OUT/game/swofty/requests.json
    rm -rf "$OUT/state" "$OUT/game/swofty/requests.json"*
    [[ $req ]] && python3 - settings.test.json "$OUT/game/swofty/settings.json" "$mode" "Z:${req//\//\\}" <<'EOF'
import json, sys
doc = json.load(open(sys.argv[1]))
if sys.argv[3] == 'settings':
    doc['requests'] = sys.argv[4]
else:
    doc['mods'] = doc.pop('addons')
    for item in doc['owned_items']:
        item['mod'] = item.pop('addon')
json.dump(doc, open(sys.argv[2], 'w'), indent=1)
EOF
    [[ $mode == assets ]] && python3 - "$OUT/game/swofty" <<'EOF'
import json, os, sys
root = sys.argv[1]
addon = os.path.join(root, 'addons', 'example.sample-mod')
for rel, data in (('erp/frag.bin', b'frag'), ('lang/extra.lng', b'LNGT'), ('files/sponsor.dds', b'DDS ')):
    os.makedirs(os.path.dirname(os.path.join(addon, rel)), exist_ok=True)
    open(os.path.join(addon, rel), 'wb').write(data)
json.dump({'schema': 1, 'profile': 'wine-test',
           'addons': [{'id': 'example.sample-mod', 'dlls': ['t_assets.dll'], 'options': []}]},
          open(os.path.join(root, 'settings.json'), 'w'))
json.dump({'features': ['file']}, open(os.path.join(root, 'assets.json'), 'w'))
EOF
    gl_level=; gl_rate=
    [[ $mode == gamelog_off ]] && gl_level=off
    [[ $mode == trigger ]] && gl_rate=0
    SWOFTY_TEST_ASSETS=$([[ $mode == assets ]] && echo 1) SWOFTY_GAME_LOG=$gl_level SWOFTY_GAME_LOG_RATE=$gl_rate SWOFTY_TEST_REQUESTS=${req:+Z:${req//\//\\}} SWOFTY_TEST_ITEMS=$ITEMS \
        "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe" $mode || status=1
    rm -rf "$OUT/game/swofty/session.json" "$OUT/game/swofty/settings.json" "$OUT/game/swofty/assets.json" \
        "$OUT/game/swofty/addons/example.sample-mod"
    echo "---- $LOG ($mode)"
    grep -v '  section ' "$LOG"
    if [[ $mode == trigger ]]; then
        check "addons: core folder lists swofty_core" 'addons: swofty[\]addons[\]core: 1 dll\(s\)[[:space:]]*$'
        check "addons: legacy core folder still loads" 'addons: swofty[\]mods[\]core: 1 dll\(s\) \(legacy folder\)'
        check "addons: legacy plugin folder still loads" 'addons: swofty[\]mods: 2 dll\(s\) \(legacy folder\)'
        once "addons: a dll in both folders loads from addons only" 'addon t_drop\.dll: also in .*[\]swofty[\]mods, skipped'
        check "addons: SwoftyAddonInit is called" 'addon t_chain_a\.dll: loaded at .*, calling SwoftyAddonInit \(API v4\)'
        check "addons: SWOFTY_MOD exports SwoftyAddonInit" 'addon t_conflict_y\.dll: loaded at .*, calling SwoftyAddonInit \(API v4\)'
        check "addons: a plugin exporting only SwoftyModInit loads" 'addon t_v1\.dll: loaded at .*, API v1 plugin, SwoftyModInit runs after the mixin commit'
        check "addons: every dll in both folders loads" 'addons: done, 12 addon\(s\) loaded'
        for re in 'scan \.xtls \[.*\] -> 1 hit at' 'registry global base\+0x[0-9a-f]+ \(= [0-9a-f]+ now\)' \
            'hook #1: list\(r14\)=0x[0-9a-f]+ r12=0x2 r15=0x1' 'built mod vtable .* from career vtable .* \(slot 4 populate .* slot 5 getAction' \
            'added button_3 \(items 1 -> 2\)' 'populate #1 .* vm=0*[1-9a-f]' "getAction #1 .* route 'mod_menu'"; do
            grep -qE "\[core\] $re|$re" "$OUT/game/swofty/loader.log" && echo "ok  : T000 event: $re" || { echo "FAIL: T000 event: $re"; status=1; }
        done
    fi
    case $mode in
    trigger)
        check "gamelog: on with the defaults" '\[gamelog\] game log to loader\.log: on \(level warn, from defaults\)'
        check "gamelog: overwrite on Ne_Log_Thunk installed" 'wrap Ne_Log_Thunk at .* installed: swofty_gamelog\.dll\(0, overwrite\)'
        check "gamelog: site on Ne_Format_String installed" 'site site_Ne_Format_String_Entry at .* installed: swofty_gamelog\.dll\(0\)'
        check "gamelog: default channels open TEST_WARN and UI_FLOW only" '\[gamelog\] gate g_Ne_LogGate at .*: opened 2 channel hash\(es\): TEST_WARN, UI_FLOW[[:space:]]*$'
        count "gamelog: gated warning logged 100 times, then the repeat cap" 100 '\[game\] TEST_WARN \(warn\): fake gate message from site_a[[:space:]]*$'
        once "gamelog: repeat cap note for the warning" '\[game\] \(format "fake gate message from %s" logged 100 times; further repeats are suppressed\)'
        count "gamelog: info on a UI channel from five threads, intact lines, capped at 100" 100 '\[game\] UI_FLOW: fake ui info from site_c[[:space:]]*$'
        count "gamelog: closed channel never reaches the log" 0 'TEST_VERBOSE|site_b'
        once "gamelog: route error from Ne_Format_String" '\[game\] UI_ROUTE \(error\): Screen swofty_settings failed to open because of 3[[:space:]]*$'
        once "gamelog: Solution-style error from Ne_Format_String" '\[game\] UI_ROUTE \(error\): Failed to find screen swofty_settings\. Solution: Make sure the the screen is loaded into the runtime[[:space:]]*$'
        once "gamelog: later route error" '\[game\] UI_ROUTE \(error\): No screen called late_screen exists[[:space:]]*$'
        count "gamelog: ordinary formats are not logged" 0 'button_7'
        count "gamelog: rate limit off (SWOFTY_GAME_LOG_RATE=0)" 0 'dropped by the rate limit'
        ;;
    vm)
        check "gamelog: default rate limit drops the burst and reports it" '\[game\] \([0-9]+ message\(s\) dropped by the rate limit of 100 per second\)'
        ;;
    session)
        check "gamelog: declared in session.json, its overwrite installs" 'wrap Ne_Log_Thunk at .* installed: swofty_gamelog\.dll\(0, overwrite\)'
        ;;
    gamelog_off)
        check "gamelog: SWOFTY_GAME_LOG=off" '\[gamelog\] game log to loader\.log: off \(level off, from environment\)'
        count "gamelog: off writes no game lines" 0 '\[game\] '
        ;;
    esac
    if [[ $mode == assets ]]; then
        api='\[assets\] api: example\.sample-mod \(t_assets\.dll\)'
        check "assets: available follows the features" '\[t_assets\] available: group 1, erp 1, db 1, lang 1, file 1, sound 0'
        once "assets: register_group during plugin init" "$api register_group asset_groups/t_assets/t\.assetgroup -> ok[[:space:]]*\$"
        once "assets: put_resource during plugin init" "$api put_resource eaid://t_assets/t\.tif\.image -> ok[[:space:]]*\$"
        once "assets: db_set by key is late at the window stage" "$api db_set entrants\.teams\.team\.t_assets = 1 -> late: the core loaded at the window stage"
        once "assets: db_set by hash is late at the window stage" "$api db_set 0xf01d03c7a1b2c3d4 tag 0 = 1 -> late"
        once "assets: lang_set during plugin init" "$api lang_set eng lng_t_assets -> ok[[:space:]]*\$"
        once "assets: lang_table during plugin init" "$api lang_table eng lang/extra\.lng -> ok[[:space:]]*\$"
        once "assets: overlay_file with the file feature on" "$api overlay_file t_assets/sponsor\.dds files/sponsor\.dds -> ok[[:space:]]*\$"
        once "assets: a path outside the addon folder is invalid" "$api lang_table eng \.\./escape\.lng -> invalid"
        once "assets: put_resource after plugin init is late" "$api put_resource eaid://t_assets/late\.tif\.image -> late: its point has passed"
        once "assets: the group joins the declared list" '\[assets\]   asset_groups/t_assets/t\.assetgroup -> 2025_asset_groups/t_assets/t\.erp'
        once "assets: the resource joins the declared list" '\[assets\] resources: 1 declared in swofty/assets\.json'
        once "assets: the string joins a table" "\[assets\] lang: table for 'eng': 1 string\(s\) from assets\.json strings"
        once "assets: the file joins the overlay" '\[assets\] overlay:   t_assets/sponsor\.dds -> /game/swofty/addons/example\.sample-mod/files/sponsor\.dds \(addon example\.sample-mod\)'
        continue
    fi
    count "$mode: no SDK calls, no api lines" 0 '\[assets\] api:'
    if [[ $ITEMS && $req ]]; then
        once "items: owned_items summary" '\[items\] settings\.json: 1 mod item\(s\) to report as owned'
        once "items: key without 0x skipped" "\\[items\\] skipping owned_items entry key 'f01d03c7'"
        once "items: ownership check hooked" '\[items\] ownership check hooked at base\+0x[0-9a-f]+'
        once "items: mod key logged once" '\[items\] reporting mod item 0xf01d03c7 \(se_livery 22, rmdesigns\.alphatauri-orlen-livery\) as owned'
    elif [[ $ITEMS ]]; then
        check "items: no owned_items, nothing hooked" '\[items\] no "owned_items" in settings\.json'
        count "items: nothing reported" 0 '\[items\] reporting'
    fi
    if [[ -z $req ]]; then
        check "$mode: no settings.json, settings stay at their code defaults" '\[settings\] no settings\.json'
        [[ ! -e $OUT/state && ! -e $OUT/game/swofty/requests.json ]] && echo "ok  : $mode: no requests file written" ||
            { echo "FAIL: $mode: a requests file was written"; status=1; }
        continue
    fi
    [[ $mode == settings ]] && where='Z:.*[\]state[\]requests[\]wine-test\.json' || where='Z:.*[\]game[\]swofty[\]requests\.json'
    check "$mode: settings.json parsed" "\[settings\] Z:.*settings\.json: profile 'wine-test', 4 addon\(s\), 8 option\(s\), 46 command\(s\); requests file $where[[:space:]]*\$"
    check "$mode: core's wrap sits in the FeHome_Handle_Command chain" 'wrap FeHome_Handle_Command at .* installed: t_chain_c\.dll\(10\) > swofty_core\.dll\(0\) > t_chain_a\.dll\(0\) > t_chain_b\.dll\(0\)[[:space:]]*$'
    for re in 'example\.sample-mod\.verbose = ON' 'example\.sample-mod\.greeting_count = 9' 'example\.sample-mod\.offset = 0\.75' \
        'example\.sample-mod\.flavour = Replica' 'example\.sample-mod\.mismatched = 4' 'community\.grid-2026\.team_livery = Tier 1 sponsor' \
        'example\.sample-mod\.verbose = OFF' 'community\.grid-2026: disabled from the next launch' 'restart requested: relaunch = true' \
        'restart: WM_CLOSE posted to 1 main window\(s\)'; do
        check "$mode: logged [settings] $re" "\[settings\] $re[[:space:]]*\$"
    done
    once "$mode: type mismatch logged once by core" '\[settings\] t_settings\.dll \(example\.sample-mod\): mismatched is a slider option, the code reads a toggle; the code default is used'
    once "$mode: undeclared setting logged once by core" '\[settings\] t_settings\.dll \(example\.sample-mod\): no option undeclared in settings\.json; the code default is used'
    once "$mode: another mod's option is not visible to this mod" '\[settings\] t_settings\.dll \(example\.sample-mod\): no option audi_name in settings\.json'
    check "$mode: SDK logs the mismatch at init" 'addon settings: mismatched is not a toggle option of this addon in swofty\.json; the code default is used'
    check "$mode: watcher calls logged by the mod" '\[t_settings\] on_change verbose -> false \(call 2\)'
    grep -q 'writing requests file' "$LOG" && { echo "FAIL: $mode: a requests write failed"; status=1; }
    python3 - "$req" "$mode" <<'EOF' || status=1
import json, os, sys
path, mode = sys.argv[1], sys.argv[2]
mod, grid = 'example.sample-mod', 'community.grid-2026'
steps = [('o', mod, 'verbose', True), ('o', mod, 'greeting_count', 9), ('o', mod, 'offset', 0.75), ('o', mod, 'flavour', 'replica'),
         ('o', mod, 'mismatched', 4), ('o', grid, 'team_livery', 'tier01'), ('o', mod, 'verbose', False), ('e', grid, False),
         None, None, ('r',)]
want = {'schema': 1, 'profile': 'wine-test', 'seq': 0, 'options': {}, 'enabled': {}, 'relaunch': False}
bad = 0
for i, s in enumerate(steps, 1):
    if s and s[0] == 'o':
        want['options'].setdefault(s[1], {})[s[2]] = s[3]
    elif s and s[0] == 'e':
        want['enabled'][s[1]] = s[2]
    elif s:
        want['relaunch'] = True
    want['seq'] += bool(s)
    try:
        got = json.load(open(f'{path}.{i}'))
    except (OSError, ValueError) as e:
        got = repr(e)
    ok = isinstance(got, dict) and json.dumps(got, sort_keys=True) == json.dumps(want, sort_keys=True) and list(got) == list(want)
    bad += not ok
    print(f'{"ok  " if ok else "FAIL"}: {mode}: requests file after command {i} ({"not ours" if not s else s[0]}): seq {want["seq"]}, relaunch {want["relaunch"]}')
    if not ok:
        print(f'      got  {got}\n      want {want}')
text = open(path).read()
exact = ('{\n  "schema": 1,\n  "profile": "wine-test",\n  "seq": 9,\n  "options": { "community.grid-2026": { "team_livery": "tier01" }, '
         '"example.sample-mod": { "verbose": false, "greeting_count": 9, "offset": 0.75, "flavour": "replica", "mismatched": 4 } },\n'
         '  "enabled": { "community.grid-2026": false },\n  "relaunch": true\n}\n')
ok = text == exact and not os.path.exists(path + '.tmp')
bad += not ok
print(f'{"ok  " if ok else "FAIL"}: {mode}: final requests file is byte-exact in the section 5 layout, no .tmp left behind')
if not ok:
    print(text)
sys.exit(bad != 0)
EOF
done
check "crash log armed at attach" '\[crash\] crash log armed'
check "clean exit logged" '\[crash\] ExitProcess\(0\)'
python3 fake_symbols.py "$OUT/game/F1_25.exe" "$OUT/game/swofty/addons/core/symbols.tsv"
for mode in crash exit; do
    echo "==== $mode"
    code=0
    timeout 120 "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe" $mode >/dev/null 2>&1 || code=$?
    grep -F '[crash]' "$LOG"
    if [[ $mode == crash ]]; then
        check "crash: first-chance fault with module, section and KB name" '\[crash\] #1 first-chance .*ACCESS_VIOLATION .* at F1_25\.exe\+0x[0-9a-f]+ \.text Fake_Crash_Here\+0x'
        check "crash: faulting access" '\[crash\]   writing 0x10'
        check "crash: unhandled line" '\[crash\] unhandled ACCESS_VIOLATION .*Fake_Crash_Here'
        check "crash: stack walks past the faulting frame" '\[crash\]   #1 .*F1_25\.exe\+0x'
        [[ $code != 0 ]] && echo "ok  : crash: process ended with $code" || { echo "FAIL: crash: process exited 0"; status=1; }
    else
        check "exit: ExitProcess(255) with the caller's stack" '\[crash\] ExitProcess\(255\) .*caller stack'
        check "exit: caller frame in the exe" '\[crash\]   #0 .*F1_25\.exe\+0x'
        [[ $code == 255 ]] && echo "ok  : exit: process ended with 255" || { echo "FAIL: exit: process ended with $code"; status=1; }
    fi
done
exit $status
