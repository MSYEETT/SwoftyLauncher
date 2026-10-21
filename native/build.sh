#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"

if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/build/zig-cache

VARIANT=${VARIANT:-}
case $VARIANT in
'') DEFS=() ;;
nomodsettings) DEFS=(-DSWOFTY_NO_MOD_SETTINGS) ;;
hooknoop) DEFS=(-DSWOFTY_MOD_SETTINGS_NOOP) ;;
*)
    echo "build.sh: unknown VARIANT '$VARIANT' (known: nomodsettings, hooknoop)" >&2
    exit 2
    ;;
esac
DIST=dist${VARIANT:+-$VARIANT}
OUT=build${VARIANT:+/$VARIANT}

SDK=../packages/sdk/include
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
CC=(zig cc -target x86_64-windows-gnu)
CXX=(zig c++ -target x86_64-windows-gnu -std=c++23)
SH=third_party/safetyhook
STATIC=(-s)
mkdir -p build "$OUT"
rm -rf "$DIST"
mkdir -p "$DIST/swofty/addons/core"
python3 ../re/tools/bindgen.py --json "$DIST/swofty/addons/core/bindings.json" --symbols "$DIST/swofty/addons/core/symbols.tsv"
python3 - src/core/stage.cpp "$DIST/swofty/addons/core/features.json" <<'EOF'
import json, re, sys
table = open(sys.argv[1]).read().split('FEATURES[] = ', 1)[1].split(';', 1)[0]
features = {name: 'stable' if stable == 'true' else 'experimental' for name, stable in re.findall(r'\{"(\w+)", (true|false)\}', table)}
json.dump({'schema': 1, 'features': features}, open(sys.argv[2], 'w'))
EOF
# Ship a per-build bindings set for the EA App build alongside the Steam one. The core (manager.cpp)
# selects the set whose build.pe_timestamp matches the running exe. The EA exe is fully packed, so its
# patterns cannot be derived from the on-disk file; this template carries the EA build block and the
# runtime code section (.rdata, not Steam's .xtls) and leaves each pattern empty until a decrypted-memory
# dump fills it. See re/db/signatures.ea.json and re/db/subsystems/core.md.
python3 - "$DIST/swofty/addons/core/bindings.json" "$DIST/swofty/addons/core/bindings.ea.json" <<'EOF'
import json, sys
steam = json.load(open(sys.argv[1]))
EA = {"exe": "F1_25.exe", "sha256": "cc506879c1c834aa54d3478b2f2133d2ec243f2f2d313397631145ea618ff7f4",
      "size": 342936448, "pe_timestamp": 1789128892, "image_base": "0x140000000", "distribution": "EA App"}
# EA anchor-function bases: the function holding each anchor string's code xref on
# the EA App build, captured by the loader's EA per-binding diagnostics on the
# shipping exe (decrypted code at the window stage). String anchors stay
# build-agnostic; these RVAs tell the resolver what it must find. In-function
# offsets are kept from the Steam rows and shape-verified at runtime, so a build
# whose code layout differs fails closed instead of hooking the wrong site.
EA_FUNC_RVA = {
    "#customisation_flow.fe_screen_selected_button": 0x1738350,
    "notification_type": 0x1691f60,
    "void game::championship::Weekend::start(const game::EntrantPointers &, const NEbool)": 0xaeedd0,
    "virtual void game::FEHomeViewModel::onCommandRaised(const neon::NeUIString &, const neon::NeUIPrimitiveType &)": 0x1bae090,
    "DevSessionDataContract.ForceStaggeredStart": 0x600160,
}
note = ("EA App build set: string anchors with captured EA function bases resolve at runtime against the "
        "decrypted code (bulk-decrypted at the window stage). In-function offsets come from the Steam rows and "
        "are shape-verified before any hook is installed. Rows without a string anchor (pattern-only) stay "
        "pending until a decrypted-memory dump yields an EA anchor.")
out = []
for b in steam["bindings"]:
    nb = dict(b); nb["status"] = "pending-dump"; nb["section"] = ".rdata"
    nb["steam_rva"] = b.get("rva"); nb["steam_pattern"] = b["pattern"]; nb["pattern"] = ""
    if nb.get("expect") is not None: nb["expect"] = ""
    nb.pop("rva", None); nb.pop("match", None); nb["build"] = EA["sha256"]
    sa = nb.get("string_anchor")
    if isinstance(sa, dict):
        sa = dict(sa)
        f = EA_FUNC_RVA.get(sa.get("string"))
        if f is not None:
            sa["func_rva"] = f
            nb["status"] = "ea-anchored"
        nb["string_anchor"] = sa
    out.append(nb)
json.dump({"schema": 1, "generator": "native/build.sh EA template", "build": EA, "note": note, "bindings": out},
          open(sys.argv[2], "w"), indent=1)
EOF

"${PIN[@]}" "${CC[@]}" -O2 -Wall -Wextra -I$SDK -Iinclude -D__USE_MINGW_ANSI_STDIO=1 -shared \
    -o "$OUT/version.dll" src/loader/version.c src/loader/version.def "${STATIC[@]}" &

[[ build/Zydis.o -nt $SH/Zydis.c ]] || "${PIN[@]}" "${CC[@]}" -O2 -I$SH -c $SH/Zydis.c -o build/Zydis.o &
ZS=third_party/zstd
ZOBJ=()
mkdir -p build/zstd
for c in $ZS/common/*.c $ZS/compress/*.c; do
    o=build/zstd/$(basename "${c%.c}").o
    ZOBJ+=("$o")
    [[ $o -nt $c ]] || "${PIN[@]}" "${CC[@]}" -O2 -DXXH_NAMESPACE=ZSTD_ -DZSTD_LEGACY_SUPPORT=0 -DDEBUGLEVEL=0 -I$ZS -c "$c" -o "$o" &
done
[[ build/safetyhook.o -nt $SH/safetyhook.cpp ]] || "${PIN[@]}" "${CXX[@]}" -O2 -I$SH -c $SH/safetyhook.cpp -o build/safetyhook.o &
wait

# GAME_INCLUDE points the core at other generated game headers (bindgen --dev output) ahead of the
# shipped ones in include/. The career library and the gamedb string decode compile out, and report
# unavailable at runtime, when the headers lack the KB structs they read (src/core/layout.hpp).
GAME=()
[[ -n ${GAME_INCLUDE:-} ]] && GAME=(-I"$GAME_INCLUDE")
"${PIN[@]}" "${CXX[@]}" -O2 -Wall -Wextra "${DEFS[@]}" -I$SDK "${GAME[@]}" -Iinclude -I$SH -I$ZS -shared -o "$OUT/swofty_core.dll" \
    src/core/manager.cpp src/core/crash.cpp src/core/settings.cpp src/core/swofty_core.cpp src/core/ui.cpp src/core/ui_vm.cpp src/core/items.cpp \
    src/core/stage.cpp src/core/assets.cpp src/core/resources.cpp src/core/gamedb.cpp src/core/overlay.cpp src/core/events.cpp src/core/hud.cpp src/core/telemetry.cpp src/core/dbread.cpp src/core/storage.cpp src/core/career.cpp src/core/career_content.cpp src/core/career_start.cpp src/core/popup.cpp src/core/sdk.cpp src/core/perf.cpp src/core/lang_override.cpp src/core/characters.cpp src/core/career_wizard.cpp build/safetyhook.o \
    build/Zydis.o "${ZOBJ[@]}" "${STATIC[@]}"

"${PIN[@]}" "${CXX[@]}" -O2 -Wall -Wextra -I$SDK -Iinclude -shared -o "$OUT/swofty_gamelog.dll" src/gamelog/gamelog.cpp "${STATIC[@]}"

cp "$OUT/version.dll" "$DIST/version.dll"
cp "$OUT/swofty_core.dll" "$DIST/swofty/addons/core/swofty_core.dll"
cp "$OUT/swofty_gamelog.dll" "$DIST/swofty/addons/core/swofty_gamelog.dll"
cp src/gamelog/log_channels.tsv "$DIST/swofty/addons/core/log_channels.tsv"

for f in "$DIST/version.dll" "$DIST/swofty/addons/core/swofty_core.dll" "$DIST/swofty/addons/core/swofty_gamelog.dll"; do
    echo "== $f"
    objdump -p "$f" | grep -E '^(Magic|Characteristics)|file format|DLL Name|^\s+\[ *[0-9]+\] \+base' || true
    objdump -f "$f" | grep -E 'architecture'
    md5sum "$f"
done
md5sum "$DIST/swofty/addons/core/bindings.json" "$DIST/swofty/addons/core/symbols.tsv" "$DIST/swofty/addons/core/log_channels.tsv" \
    "$DIST/swofty/addons/core/features.json"

# Bundled addons ship next to the loader in a release (addons/ beside loader/) and the launcher installs them
# switched off. They are packaged into bundled/, never into dist/, because dist/ is copied into the game folder.
if [[ -z $VARIANT ]]; then
    rm -rf bundled
    mkdir -p bundled
    for addon in ../addons/re-optimization; do
        if [[ ! -f $addon/native/build.sh || ! -f $addon/swofty.json ]]; then
            echo "build.sh: $addon has no native/build.sh or swofty.json; not bundled" >&2
            continue
        fi
        "${PIN[@]}" bash "$addon/native/build.sh"
        python3 ../tools/pack_addon.py "$addon" bundled
    done
    md5sum bundled/*.swoftyaddon 2>/dev/null || true
fi
