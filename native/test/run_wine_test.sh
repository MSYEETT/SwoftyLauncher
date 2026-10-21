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
rm -rf "$OUT/game" "$OUT/stages"
mkdir -p "$OUT/game" "$OUT/stages"
"${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -o "$OUT/game/F1_25.exe" fake_game.c -lversion -luser32 &
cp -r ../dist/. "$OUT/game/"
mkdir -p "$OUT/game/swofty/mods/core"
mv "$OUT/game/swofty/addons/core/swofty_gamelog.dll" "$OUT/game/swofty/addons/core/log_channels.tsv" "$OUT/game/swofty/mods/core/"
for m in addons/t_*.cpp; do
    name=$(basename "${m%.cpp}")
    case $name in
    t_chain_b) dest=$OUT/game/swofty/mods ;;
    t_stage_*) dest=$OUT/stages ;;
    t_ntpage | t_int3hook) dest=$OUT ;;
    *) dest=$OUT/game/swofty/addons ;;
    esac
    "${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -I../../packages/sdk/include -I../include -shared -s \
        -o "$dest/$name.dll" "$m" &
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
SG=$OUT/stages/game
cp -r "$OUT/game" "$SG"
rm "$SG"/swofty/addons/t_*.dll "$SG"/swofty/mods/t_*.dll
mv "$OUT"/stages/t_stage_*.dll "$SG/swofty/addons/"
python3 - "$SG/F1_25.exe" "$SG/swofty" <<'EOF'
import json, struct, sys
exe, root = sys.argv[1], sys.argv[2]
b = open(exe, 'rb').read()
pe = struct.unpack_from('<I', b, 0x3c)[0]
nsec, stamp, opt = struct.unpack_from('<H', b, pe + 6)[0], struct.unpack_from('<I', b, pe + 8)[0], struct.unpack_from('<H', b, pe + 20)[0]
secs = [struct.unpack_from('<8sIIII', b, pe + 24 + opt + 40 * i) for i in range(nsec)]
_, _, va, size, raw = next(s for s in secs if s[0].rstrip(b'\0') == b'.xtls')
code = b[raw:raw + size]
def kb(tag):
    at = code.find(b'\x48\xb8' + tag.encode())
    assert at >= 0 and code.find(b'\x48\xb8' + tag.encode(), at + 1) < 0, tag
    return hex(va + at)
core = root + '/addons/core/'
doc = json.load(open(core + 'bindings.json'))
for x in doc['bindings']:
    if 'kb_tag' in x:
        x['rva'] = kb(x.pop('kb_tag'))
        if not x.pop('kb_no_match', False):
            x['match'] = kb(x.pop('kb_match_tag')) if 'kb_match_tag' in x else hex(int(x['rva'], 16) - x['anchor'])
    else:
        x.pop('rva', None)
        x.pop('match', None)
        if isinstance(x.get('string_anchor'), dict):
            x['string_anchor'].pop('func_rva', None)
doc['build'] = {'exe': 'F1_25.exe', 'sha256': 'wine-test', 'pe_timestamp': stamp, 'size': len(b)}
json.dump(doc, open(core + 'bindings.wine-test.json', 'w'), indent=1)
json.dump({'schema': 1, 'profile': 'wine-test', 'addons': [], 'early_plugins': ['t_stage_early.dll']},
          open(root + '/settings.json', 'w'))
EOF
export WINEPREFIX="$OUT/prefix" WINEDLLOVERRIDES="version=n,b;mscoree,mshtml=;winemenubuilder.exe,winedbg.exe=d" WINEDEBUG=-all
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
CACHE=$OUT/game/swofty/cache/scan-cache.tsv
for mode in trigger fallback vm settings settings_local session gamelog_off assets; do
    echo "==== $mode"
    stale=
    [[ $mode == vm ]] && stale=$(python3 - "$CACHE" <<'EOF'
import sys
path = sys.argv[1]
lines = open(path).read().split('\n')
i = next(i for i, l in enumerate(lines) if l.endswith('\t48 81 EC 88 02 00 00 C5 F8 29 B4 24 70 02 00 00 4C 89 8C 24') or l.endswith('C5 F8 29 B4 24 70 02 00 00 4C 89 8C 24'))
rva, section, pattern = lines[i].split('\t')
lines[i] = '\t'.join([format(int(rva, 16) + 1, 'x'), section, pattern])
open(path, 'w').write('\n'.join(lines))
print(pattern)
EOF
)
    [[ $mode == settings ]] && sed -i '1s/\t[0-9a-f]*$/\t0123456789abcdef/' "$CACHE"
    resolve=
    [[ $mode == gamelog_off ]] && resolve=scan
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
import json, os, shutil, sys
root = sys.argv[1]
addon = os.path.join(root, 'addons', 'example.sample-mod')
for rel, data in (('erp/frag.bin', b'frag'), ('lang/extra.lng', b'LNGT'), ('files/sponsor.dds', b'DDS ')):
    os.makedirs(os.path.dirname(os.path.join(addon, rel)), exist_ok=True)
    open(os.path.join(addon, rel), 'wb').write(data)
os.makedirs(os.path.join(addon, 'native'))
shutil.copy(os.path.join(root, 'addons', 't_assets.dll'), os.path.join(addon, 'native', 't_assets.dll'))
json.dump({'schema': 1, 'profile': 'wine-test',
           'addons': [{'id': 'example.sample-mod', 'dlls': ['t_assets.dll'], 'options': []}],
           'plugins': ['addons/example.sample-mod/native/t_assets.dll']},
          open(os.path.join(root, 'settings.json'), 'w'))
json.dump({'features': ['file', 'telemetry']}, open(os.path.join(root, 'assets.json'), 'w'))
EOF
    gl_level=; gl_rate=
    [[ $mode == gamelog_off ]] && gl_level=off
    gl_channels=
    [[ $mode == trigger ]] && gl_rate=0
    [[ $mode == trigger || $mode == vm ]] && gl_channels=default,verbose
    SWOFTY_RESOLVE=$resolve SWOFTY_TEST_ASSETS=$([[ $mode == assets ]] && echo 1) SWOFTY_GAME_LOG=$gl_level SWOFTY_GAME_LOG_RATE=$gl_rate SWOFTY_GAME_LOG_CHANNELS=$gl_channels SWOFTY_TEST_REQUESTS=${req:+Z:${req//\//\\}} SWOFTY_TEST_ITEMS=$ITEMS \
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
        once "mixins: a binding bindings.json does not list is named as missing" 'addon t_want\.dll needs wine-test:Fake_Unlisted, which bindings\.json does not list'
        once "mixins: a listed binding that does not match stays unresolved" 'addon t_want\.dll needs wine-test:Fake_Missing, which does not resolve on this build'
        check "addons: SwoftyAddonInit is called" 'addon t_chain_a\.dll: loaded at .*, calling SwoftyAddonInit \(API v8\)'
        check "addons: SWOFTY_MOD exports SwoftyAddonInit" 'addon t_conflict_y\.dll: loaded at .*, calling SwoftyAddonInit \(API v8\)'
        if grep -q "core mixin set 'swofty_core.dll (flow states)'" "$LOG"; then
            once "career: the flow-state site is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(flow states\)': 1 mixin"
            once "career: a missing flow-state site drops only its own set" 'addon swofty_core\.dll \(flow states\) dropped: needs site_FeFlow_SetStateStatus'
            once "career: a missing Team HQ site drops only its own set" 'addon swofty_core\.dll \(Team HQ answer\) dropped: needs site_Career_TeamHqIsMyTeamResult'
            check "career: setup reports the library unavailable" '\[career\] setup: the flow-state site is not installed'
        else
            check "career: a core without the FlowState layout registers no career sites" '\[career\] setup: the flow-state site is not installed'
        fi
        check "career: swofty_core itself still initialises" 'addon swofty_core\.dll: SwoftyAddonInit returned 1 \(ok\)'
        check "addons: a plugin exporting only SwoftyModInit loads" 'addon t_v1\.dll: loaded at .*, API v1 plugin, SwoftyModInit runs after the mixin commit'
        check "addons: every dll in both folders loads" 'addons: done, 13 addon\(s\) loaded'
        for re in 'scan \.xtls \[.*\] -> 1 hit at' 'registry global base\+0x[0-9a-f]+ \(= [0-9a-f]+ now\)' \
            'hook #1: list\(r14\)=0x[0-9a-f]+ r12=0x2 r15=0x1' 'built mod vtable .* from career vtable .* \(slot 4 populate .* slot 5 getAction' \
            'added button_3 \(items 1 -> 2\)' 'populate #1 .* vm=0*[1-9a-f]' "getAction #1 .* route 'mod_menu'"; do
            grep -qE "\[core\] $re|$re" "$OUT/game/swofty/loader.log" && echo "ok  : T000 event: $re" || { echo "FAIL: T000 event: $re"; status=1; }
        done
    fi
    case $mode in
    trigger)
        once "scan cache: the first run starts without one" '\[core\] scan cache: none yet for this exe \(key [0-9a-f]{16}\)'
        check "scan cache: the addresses the scans found are saved" '\[core\] scan cache: [1-9][0-9]* address\(es\) saved to swofty[\]cache[\]scan-cache\.tsv'
        count "resolve: an exe other than the KB build never uses the KB addresses" 0 'at the KB address'
        count "scan cache: nothing is cached yet on the first run" 0 'the address cached for this exe'
        ;;
    fallback)
        check "scan cache: the second run reads the saved addresses" '\[core\] scan cache: [1-9][0-9]* address\(es\) for this exe \(key [0-9a-f]{16}\), each checked'
        check "scan cache: a cached address is checked against the pattern and used without a scan" 'scan \.xtls \[.*\] -> 1 hit at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), the address cached for this exe'
        count "scan cache: with every address cached, no pattern needs a full scan" 0 'scan \.xtls \[.*\] -> 1 hit at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\)[[:space:]]*$'
        count "scan cache: an unchanged cache is not rewritten" 0 'scan cache: .* saved to'
        ;;
    settings)
        once "scan cache: a cache written for another exe is dropped" '\[core\] scan cache: swofty[\]cache[\]scan-cache\.tsv belongs to another exe; starting over'
        check "scan cache: the rebuilt cache is saved" '\[core\] scan cache: [1-9][0-9]* address\(es\) saved'
        ;;
    gamelog_off)
        once "resolve: SWOFTY_RESOLVE=scan turns the KB addresses and the scan cache off" '\[core\] binding resolution: SWOFTY_RESOLVE=scan, every pattern is scanned and the scan cache is off'
        count "resolve: SWOFTY_RESOLVE=scan uses no cached address" 0 'the address cached for this exe'
        ;;
    esac
    if [[ $stale ]]; then
        python3 - "$LOG" "$stale" <<'EOF' && echo "ok  : scan cache: a stale address fails the pattern check, is scanned again and saved" || { echo "FAIL: scan cache: a stale address fails the pattern check, is scanned again and saved"; status=1; }
import sys
log, pattern = open(sys.argv[1], errors='replace').read(), sys.argv[2]
full = [l for l in log.split('\n') if f'[{pattern}] -> 1 hit at' in l and 'cached' not in l]
sys.exit(not (len(full) == 1 and 'address(es) saved to' in log))
EOF
    fi
    case $mode in
    trigger)
        check "events: flow-tick hook installed" '\[events\] Vm_Update_FlowStateMachine hooked at base\+0x4423ce0'
        check "events: setup reports the onFrame tick live" '\[events\] setup: live; onFrame armed'
        once "events: first flow tick fires with dt 0" '\[events\] flow tick 1 observed, dt 0\.0'
        once "events: second flow tick fires" '\[events\] flow tick 2 observed'
        once "events: a subscribe in SwoftyAddonInit is queued until the setup" '\[events\] subscribe session\.start by t_events_early\.dll -> ok token [0-9]+, queued until the events setup[[:space:]]*$'
        once "events: an on_frame in SWOFTY_ON_LOAD is queued until the setup" '\[events\] on_frame by t_events_early\.dll -> ok token [0-9]+, queued until the events setup[[:space:]]*$'
        once "events: available() is true and on_frame ok at ON_LOAD, before the setup" '\[t_events_early\] at load: events available 1, on_frame ok \(token [0-9]+\), career available 0'
        once "events: the setup arms the queued registrations and names them" '\[events\] [0-9]+ registration\(s\) queued before setup are armed: .*session\.start token [0-9]+ \(t_events_early\.dll\).*on_frame token [0-9]+ \(t_events_early\.dll\)'
        awk '/\[career\] setup:/ && !a {a = NR} /\[t_events_early\] at load/ && !b {b = NR} /\[events\] setup: live/ && !c {c = NR}
             /queued before setup are armed/ && !d {d = NR} /\[events\] flow tick 1 observed/ && !e {e = NR}
             END {exit !(a && a < b && b < c && c < d && d < e)}' "$LOG" &&
            echo "ok  : events: career setup, then ON_LOAD, then the events setup arms the queue, then the first tick" ||
            { echo "FAIL: events: career setup, then ON_LOAD, then the events setup arms the queue, then the first tick"; status=1; }
        check "gamelog: on with the verbose UI channels from the environment" '\[gamelog\] game log to loader\.log: on \(level warn, from environment\), channels .default,verbose.'
        check "gamelog: overwrite on Ne_Log_Thunk installed" 'wrap Ne_Log_Thunk at .* installed: swofty_gamelog\.dll\(0, overwrite\)'
        check "gamelog: site on Ne_Format_String installed" 'site site_Ne_Format_String_Entry at .* installed: swofty_gamelog\.dll\(0\)'
        check "gamelog: default,verbose opens TEST_WARN and UI_FLOW only" '\[gamelog\] gate g_Ne_LogGate at .*: opened 2 channel hash\(es\): TEST_WARN, UI_FLOW[[:space:]]*$'
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
    fallback)
        check "gamelog: on with the defaults" '\[gamelog\] game log to loader\.log: on \(level warn, from defaults\)'
        check "gamelog: default channels open TEST_WARN and UI_FLOW (UI channels at warn level)" '\[gamelog\] gate g_Ne_LogGate at .*: opened 2 channel hash\(es\): TEST_WARN, UI_FLOW[[:space:]]*$'
        count "gamelog: info messages on a UI channel are not logged by default" 0 'UI_FLOW: fake ui info'
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
        once "plugins: settings.json lists the addon's plugin" 'addons: settings\.json lists 1 plugin\(s\) in addon folders'
        once "plugins: the plugin loads from its addon folder" 'addon t_assets\.dll: loading .*[\]swofty[\]addons[\]example\.sample-mod[\]native[\]t_assets\.dll[[:space:]]*$'
        once "plugins: the loose copy of a listed plugin is skipped" 'addon t_assets\.dll: settings\.json lists it in an addon folder, the copy in .*[\]swofty[\]addons is skipped'
        once "plugins: the plugin initialises" 'addon t_assets\.dll: SwoftyAddonInit returned 1 \(ok\)'
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
        sapi='\[storage\] api: example\.sample-mod \(t_assets\.dll\)'
        check "storage: available with the feature on" '\[t_assets\] storage available 1'
        once "storage: write lands in the addon folder" "$sapi write save/state\.txt \(13 byte\(s\)\) -> ok[[:space:]]*\$"
        once "storage: read returns the file" "$sapi read save/state\.txt \(13 byte\(s\)\) -> ok[[:space:]]*\$"
        once "storage: the round trip returns the bytes written" "\[t_assets\] storage read ok 'round-trip-ok'"
        count "storage: size reports the file length (read-back plus the explicit call)" 2 "$sapi size save/state\.txt -> ok[[:space:]]*\$"
        check "storage: the plugin reads back the size" '\[t_assets\] storage size ok 13'
        once "storage: list enumerates the folder" "$sapi list save -> ok[[:space:]]*\$"
        check "storage: the listing has both files" '\[t_assets\] storage list ok 18 .*log\.txt.*state\.txt|\[t_assets\] storage list ok 18 .*state\.txt.*log\.txt'
        count "storage: append creates then grows a file" 2 "$sapi append save/log\.txt \(4 byte\(s\)\) -> ok"
        check "storage: both appends succeed" '\[t_assets\] storage append ok ok'
        once "storage: a path climbing above the folder is invalid" "$sapi write \.\./escape\.txt \(4 byte\(s\)\) -> invalid: '\.\./escape\.txt' is not a relative path"
        check "storage: climb, absolute, drive and back-climb paths are all rejected" '\[t_assets\] storage escapes invalid invalid invalid invalid'
        once "storage: size of a missing file is invalid" "$sapi size missing\.txt -> invalid: swofty/addons/example\.sample-mod/missing\.txt is missing"
        once "storage: remove deletes the file" "$sapi remove save/state\.txt -> ok[[:space:]]*\$"
        once "storage: remove of a gone file is invalid" "$sapi remove save/state\.txt -> invalid: swofty/addons/example\.sample-mod/save/state\.txt is missing"
        check "storage: the plugin sees remove then missing" '\[t_assets\] storage remove-again invalid'
        once "telemetry: setup wired through ready() when the feature is on" '\[telemetry\] on; frame tick '
        once "telemetry: reads are gated off outside a session" '\[t_assets\] telemetry: available 0, session unavailable, car0 unavailable, car99 invalid'
        once "telemetry: an addon opens the shared feed" '\[t_assets\] telemetry: shared_memory ok name=Local.SwoftyTelemetry'
        once "telemetry: the shared feed mapping is created" '\[telemetry\] shared_memory: Local.SwoftyTelemetry mapped, 6920 bytes'
        eapi='\[events\]'
        once "events: the plugin's session.start subscription is accepted and queued" "$eapi subscribe session\.start by example\.sample-mod -> ok token [0-9]+, queued until the events setup[[:space:]]*\$"
        once "events: the plugin's on_frame callback is accepted" "$eapi on_frame by example\.sample-mod -> ok token"
        once "events: an unknown event name is rejected" "$eapi subscribe not\.an\.event by example\.sample-mod -> invalid[[:space:]]*\$"
        once "events: unsubscribe of a live token succeeds" "$eapi unsubscribe token [0-9]+ -> ok[[:space:]]*\$"
        once "events: unsubscribe of an already-dropped token is invalid" "$eapi unsubscribe token [0-9]+ -> invalid[[:space:]]*\$"
        check "events: the plugin sees ok/ok/invalid/ok/invalid through the wrapper" '\[t_assets\] events subscribe ok on_frame ok unknown invalid unsubscribe ok unsubscribe-again invalid'
        check "db: read reports unavailable until the database pointer resolves on this build" '\[t_assets\] db available 0 get unavailable'
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
check "crash reports armed with a minidump" '\[crash\] crash reports: .*crash-<time>\.txt with a stacks-only minidump'
check "Ne heap error reporting hooked by the pattern" '\[crash\] Ne_ReportMemoryError at base\+0x[0-9a-f]+ hooked'
python3 fake_symbols.py "$OUT/game/F1_25.exe" "$OUT/game/swofty/addons/core/symbols.tsv"
SW=$OUT/game/swofty
# A report parser shared by the crash modes: it checks the newest report parses as the expected
# sections and that the minidump beside it is a real MDMP with streams.
parse_report() {
    python3 - "$SW" "$1" "$2" <<'PY'
import glob, os, struct, sys
root, want_reason, checks = sys.argv[1], sys.argv[2], sys.argv[3].split(',')
reports = sorted(glob.glob(os.path.join(root, 'crash-*.txt')), key=os.path.getmtime)
if not reports:
    print('FAIL: parse: no crash-*.txt was written'); sys.exit(1)
path = reports[-1]
text = open(path, encoding='utf-8', errors='replace').read()
bad = 0
def ok(cond, what):
    global bad
    print(f'{"ok  " if cond else "FAIL"}: {what}')
    bad += not cond
ok(text.startswith('Swofty crash report'), 'parse: report has the header')
ok(want_reason in text, f'parse: reason mentions {want_reason!r}')
sections = {l[3:].strip() for l in text.splitlines() if l.startswith('== ')}
for s in ('Registers', "Stack (unwound with the modules' exception data)", 'Code around rip',
          'Memory at the registers', 'Game state (from the black box)', 'Black box: last events',
          'Swofty probes (hook and callback activity)', 'Loaded modules', 'Threads',
          'loader.log (last 200 lines)', 'Minidump'):
    ok(any(name == s or name.startswith(s + ' (') for name in sections), f'parse: has the {s!r} section')
for c in checks:
    if c:
        ok(c in text, f'parse: contains {c!r}')
dump = path[:-4] + '.dmp'
raw = open(dump, 'rb').read() if os.path.exists(dump) else b''
ok(raw[:4] == b'MDMP', 'parse: a minidump beside it starts with MDMP')
ok(len(raw) > 4096, f'parse: the minidump is non-trivial ({len(raw)} bytes)')
nstreams = struct.unpack_from('<I', raw, 8)[0] if len(raw) >= 12 else 0
ok(0 < nstreams < 64, f'parse: the minidump has {nstreams} stream(s)')
sys.exit(bad != 0)
PY
}
for mode in crash exit heap abort; do
    echo "==== $mode"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    timeout 120 "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe" $mode >/dev/null 2>&1 || code=$?
    grep -F '[crash]' "$LOG"
    if [[ $mode == crash ]]; then
        check "crash: first-chance fault with module, section and KB name" '\[crash\] #1 first-chance .*ACCESS_VIOLATION .* at F1_25\.exe\+0x[0-9a-f]+ \.text Fake_Crash_Here\+0x'
        check "crash: faulting access" '\[crash\]   writing 0x10'
        check "crash: unhandled line" '\[crash\] unhandled ACCESS_VIOLATION .*Fake_Crash_Here'
        check "crash: stack walks past the faulting frame" '\[crash\]   #1 .*F1_25\.exe\+0x'
        check "crash: a report with a minidump is written" '\[crash\] crash report .*crash-.*\.txt: unhandled ACCESS_VIOLATION.*minidump crash-.*\.dmp'
        parse_report "Fake_Crash_Here" "Fake_Crash_Here,writing 0x10" || status=1
        [[ $code != 0 ]] && echo "ok  : crash: process ended with $code" || { echo "FAIL: crash: process exited 0"; status=1; }
    elif [[ $mode == exit ]]; then
        check "exit: ExitProcess(255) with the caller's stack" '\[crash\] ExitProcess\(255\) .*caller stack'
        check "exit: caller frame in the exe" '\[crash\]   #0 .*F1_25\.exe\+0x'
        check "exit: a report is written for the non-zero exit" '\[crash\] crash report .*crash-.*: ExitProcess\(255'
        parse_report "ExitProcess(255" "" || status=1
        [[ $code == 255 ]] && echo "ok  : exit: process ended with 255" || { echo "FAIL: exit: process ended with $code"; status=1; }
    elif [[ $mode == heap ]]; then
        check "heap: the not-fatal heap error is logged with its stats" '\[crash\] Ne heap error 0x50 \(not fatal\).*capacity 65536, used 512, free 65024, largest free 64512'
        check "heap: the fatal heap error 0x16 is logged" '\[crash\] Ne heap error 0x16 \(fatal\).*Failed to find address'
        check "heap: a report is written before the abort" '\[crash\] crash report .*crash-.*: fatal Ne heap error 0x16'
        parse_report "fatal Ne heap error 0x16" "Ne heap,FakeHeap,Failed to find address,capacity 65536,guard 32 bytes,CORRUPT" || status=1
        [[ $code != 0 ]] && echo "ok  : heap: the process aborted ($code)" || { echo "FAIL: heap: process exited 0"; status=1; }
    else
        check "abort: Ne_FatalAbort is recognised" '\[crash\] crash report .*crash-.*: the game aborted itself'
        parse_report "the game aborted itself" "Ne_FatalAbort" || status=1
        [[ $code != 0 ]] && echo "ok  : abort: the process aborted ($code)" || { echo "FAIL: abort: process exited 0"; status=1; }
    fi
done
echo "==== ntpage"
cp "$OUT/t_ntpage.dll" "$SW/mods/core/ntpage_hook.dll"
rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
code=0
timeout 120 "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe" exit >"$OUT/ntpage.out" 2>&1 || code=$?
rm -f "$SW/mods/core/ntpage_hook.dll"
grep -F '[ntpage]' "$OUT/ntpage.out" || true
grep -F '[crash]' "$LOG"
if grep -q '^\[ntpage\] ntdll!NtTerminateProcess now exports' "$OUT/ntpage.out"; then
    echo "ok  : ntpage: the export points at a stub on the page of NtProtectVirtualMemory"
else
    echo "FAIL: ntpage: the injected hook did not move the export"
    status=1
fi
check "ntpage: hooking a function on that ntdll page finishes the crash init" '\[crash\] crash log armed: .*ExitProcess hooked, TerminateProcess hooked'
check "ntpage: NtTerminateProcess is hooked there" '\[crash\] crash reports: .*NtTerminateProcess hooked'
check "ntpage: the exit still gets a report" '\[crash\] crash report .*crash-.*: ExitProcess\(255'
[[ $code == 255 ]] && echo "ok  : ntpage: process ended with 255" || { echo "FAIL: ntpage: process ended with $code"; status=1; }
for skip in 0 1; do
    echo "==== int3$([[ $skip == 1 ]] && echo ', unowned on the writer')"
    cp "$OUT/t_int3hook.dll" "$SW/mods/core/int3_hook.dll"
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    code=0
    T_INT3_SKIP_WRITER=$skip timeout 120 "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe" crash >"$OUT/int3.out" 2>&1 || code=$?
    rm -f "$SW/mods/core/int3_hook.dll"
    grep -F '[int3]' "$OUT/int3.out" || true
    grep -F '[crash]' "$LOG"
    if grep -q '^\[int3\] breakpoints planted on 2 GetModuleFileName' "$OUT/int3.out"; then
        echo "ok  : int3: an injected hook serves int3 breakpoints on GetModuleFileNameW and GetModuleFileNameA"
    else
        echo "FAIL: int3: the injected hook planted no breakpoints"
        status=1
    fi
    check "int3: a report with a minidump is written" '\[crash\] crash report .*crash-.*\.txt: unhandled ACCESS_VIOLATION.*minidump crash-.*\.dmp'
    if [[ $skip == 0 ]]; then
        if grep -qE 'crash report writer (faulted|hit)' "$LOG" || grep -q 'cut short' "$SW"/crash-*.txt; then
            echo "FAIL: int3: the writer took the hook's breakpoint for a fault of its own"
            status=1
        else
            echo "ok  : int3: the writer leaves the hook's breakpoints to the hook"
        fi
        parse_report "Fake_Crash_Here" "Fake_Crash_Here,writing 0x10,exe          Z:" || status=1
    else
        check "int3: the writer logs the unowned breakpoint and goes on" '\[crash\] the crash report writer hit BREAKPOINT \(0x80000003\) at .* in the header of swofty.crash-.*; it goes on with the next step'
        parse_report "Fake_Crash_Here" "[the header is cut short: the writer hit BREAKPOINT (0x80000003) at ,Fake_Crash_Here,writing 0x10" || status=1
    fi
    [[ $code != 0 ]] && echo "ok  : int3: process ended with $code" || { echo "FAIL: int3: process exited 0"; status=1; }
done
echo "==== stages"
LOG=$SG/swofty/loader.log
SWOFTY_EARLY_STAGE=1 "${PIN[@]}" xvfb-run -a wine "$SG/F1_25.exe" stages || status=1
echo "---- $LOG (stages)"
grep -v '  section ' "$LOG"
check "stages: early stage on from the environment" 'early stage: on \(SWOFTY_EARLY_STAGE=1\)'
check "stages: early plugin list read from settings.json" 'early stage: plugin t_stage_early\.dll has "early": true'
check "stages: early trigger fired inside the game's CreateMutexW" "trigger: KERNEL32!CreateMutexW from .* \(\.text\) name 'Codemasters F1 25' on thread .*loading addons now \(early stage\)"
once "stages: the window-stage plugin waits for the window" 'addon t_stage_late\.dll: loads at the window stage'
check "stages: core picks the wine-test bindings set by the fake exe's PE timestamp" '\[core\] bindings: 3 set\(s\) present, selected bindings\.wine-test\.json by PE timestamp [0-9]+'
check "stages: the fake exe is the analysed build of that set" '\[core\] bindings\.wine-test\.json: [0-9]+ binding\(s\) for build wine-test, this exe \(rva checked\)'
check "stages: core's own wrap still installs with the Steam rvas left out of the set" 'wrap FeHome_Handle_Command at .* installed: swofty_core\.dll\(0\)'
once "stages: the early site on the entry of Fake_Stage_Site installs" '\[core\] site wine-test:site_Stage_Entry at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\) installed: t_stage_early\.dll\(0\)[[:space:]]*$'
once "stages: the early wrap on the entry of Fake_Stage_Wrap installs" '\[core\] wrap wine-test:Fake_Stage_Wrap_Early at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\) installed: t_stage_early\.dll\(0\)[[:space:]]*$'
check "stages: window stage after the early load" 'stage: window \(USER32!RegisterClassExW .*\), addons were loaded at the early stage'
once "stages: the early site binds at its KB address" '\[core\] bind wine-test:site_Stage_Entry -> 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), site experimental via pattern at the KB address'
once "stages: the early wrap binds at its KB address" '\[core\] bind wine-test:Fake_Stage_Wrap_Early -> 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), func experimental via pattern at the KB address'
count "stages: the hooked Fake_Stage_Site is matched under its hook without a scan" 0 'scan \.xtls \[48 B8 53 57 46 54 53 49 54 45 B8 07 00 00 00 C3\]'
count "stages: the hooked Fake_Stage_Wrap is matched under its hook without a scan" 0 'scan \.xtls \[48 B8 53 57 46 54 57 52 41 50 B8 09 00 00 00 C3\]'
count "stages: the early patterns that check out at their KB address are never scanned" 0 'scan \.xtls \[48 B8 53 57 46 54 (53 49 54 45|57 52 41 50)\]'
once "stages: Fake_Stage_Plain's KB address holds other bytes" '\[core\] binding wine-test:Fake_Stage_Plain: the pattern does not match at the KB address 0x[0-9a-f]+; scanning \.xtls'
once "stages: Fake_Stage_Plain is then scanned" 'scan \.xtls \[48 B8 53 57 46 54 50 4C 41 4E B8 05 00 00 00 C3\] -> 1 hit at'
once "stages: Fake_Stage_Plain binds by the scan" '\[core\] bind wine-test:Fake_Stage_Plain -> 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), func experimental via pattern[[:space:]]*$'
for id in Fake_Stage_Wrong Fake_Stage_Site_Wrong; do
    once "stages: wine-test:$id does not match at its KB address" "\[core\] binding wine-test:$id: the pattern does not match at the KB address"
done
count "stages: the ambiguous pattern carries no KB address" 0 'binding wine-test:Fake_Stage_Ambiguous: the pattern does not match at the KB address'
for id in Fake_Stage_Site Fake_Stage_Wrap; do
    rva=$(python3 -c 'import json, sys; print(next(b["rva"] for b in json.load(open(sys.argv[1]))["bindings"] if b["id"] == sys.argv[2]))' \
        "$SG/swofty/addons/core/bindings.wine-test.json" "wine-test:$id" || true)
    addr=$(grep -oE "\[core\] bind wine-test:$id -> 0x[0-9a-f]+ \(base\+$rva\)" "$LOG" | grep -oE '0x[0-9a-f]+' | head -n1 || true)
    once "stages: wine-test:$id binds at its KB rva $rva" "\[core\] bind wine-test:$id -> 0x[0-9a-f]+ \(base\+$rva\), func experimental via pattern"
    once "stages: wine-test:$id matches the bytes under the early hook at its KB address" \
        "\[core\] binding wine-test:$id: its bytes carry an installed hook; the pattern matches the bytes under it at the KB address ${addr:-none}[[:space:]]*\$"
done
awk '/site wine-test:site_Stage_Entry at .* installed/ && !a {a = NR} /stage: window/ && !b {b = NR} /its bytes carry an installed hook/ && !c {c = NR}
     END {exit !(a && a < b && b < c)}' "$LOG" && echo "ok  : stages: the fallback runs at the window stage, after the early hooks installed" ||
    { echo "FAIL: stages: the fallback runs at the window stage, after the early hooks installed"; status=1; }
once "stages: the window-stage plugin that binds both hooked functions initialises" 'addon t_stage_late\.dll: SwoftyAddonInit returned 1 \(ok\)'
for id in Fake_Stage_Wrong Fake_Stage_Site_Wrong Fake_Stage_Ambiguous; do
    once "stages: wine-test:$id is refused" "\[core\] binding wine-test:$id: signature did not match exactly once"
    once "stages: t_stage_refused needs wine-test:$id" "\[core\] addon t_stage_refused\.dll needs wine-test:$id, which does not resolve on this build"
done
check "stages: the ambiguous pattern hits more than once" 'scan \.xtls \[48 B8 53 57 46 54\] -> ([2-9]|[1-9][0-9]+) hits'
count "stages: only the two bindings under early hooks take the fallback" 2 'its bytes carry an installed hook'
once "stages: the plugin with wrong or ambiguous patterns fails its init" 'addon t_stage_refused\.dll: SwoftyAddonInit returned 0 \(FAILED\)'
once "stages: the exe matches the KB build's timestamp and size, so the KB addresses are used" '\[core\] binding resolution: [0-9]+ of [0-9]+ pattern\(s\) carry their KB address'
echo "==== stages, exe size differs from the KB build"
python3 - "$SG/swofty/addons/core/bindings.wine-test.json" <<'EOF'
import json, sys
doc = json.load(open(sys.argv[1]))
doc['build']['size'] += 1
json.dump(doc, open(sys.argv[1], 'w'), indent=1)
EOF
rm -rf "$SG/swofty/cache"
SWOFTY_EARLY_STAGE=1 "${PIN[@]}" xvfb-run -a wine "$SG/F1_25.exe" stages || status=1
once "stages: an exe of another size than the KB build never uses the KB addresses" '\[core\] binding resolution: the exe is [0-9]+ bytes and the KB build [0-9]+; the KB addresses are not used, every pattern is scanned'
count "stages: with another size, nothing binds at a KB address" 0 'via pattern at the KB address|does not match at the KB address'
once "stages: with another size, the early site is found by a scan" '\[core\] bind wine-test:site_Stage_Entry -> 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), site experimental via pattern[[:space:]]*$'
exit $status
