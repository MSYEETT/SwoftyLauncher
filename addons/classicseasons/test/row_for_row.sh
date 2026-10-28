#!/usr/bin/env bash
# row_for_row.sh <year> <lower_tool> [work dir]: build one season's database three ways with the launcher CLI into
# scratch data dirs (never the game folder): a = all its build operations in one file (0.6.x), a15 = the split files
# a launcher before 1.6 builds, b = the hand-written operations plus the operations the core lowers from its records.
# Then compare each against a, rows matched by label in the tables the core fills in id order.
set -euo pipefail
YEAR=$1 TOOL=$2 W=${3:-/mnt/gaming/swofty-std16/w11/rows}
REPO=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd)
CS=$REPO/addons/classicseasons
CLI=${CLI:-$HOME/SwoftyLauncher/bin/swofty}
GAME=${GAME:-/mnt/gaming/SteamLibrary/steamapps/common/F1 25}
BASE=${BASE:-$HOME/.local/share/swofty}
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
ID=swofty.classic-seasons-$YEAR
mkdir -p "$W"
STOCK=$(find "$BASE/baseline/objects" -name "$(python3 -c "
import json,glob
for p in glob.glob('$BASE/baseline/*/baseline.json'):
    f=json.load(open(p))['files'].get('game_package/gamedatabases/gamedatabases.2025.bin')
    if f: print(f['sha256']); break")" | head -1)
addon() {
    local dir=$1 src=$2; shift 2
    rm -rf "$dir"; mkdir -p "$dir/db"
    for f in "$@"; do cp "$f" "$dir/db/"; done
    python3 - "$src" "$dir" "$@" <<'PY'
import json, sys
from pathlib import Path
src, dst, files = sys.argv[1], Path(sys.argv[2]), sys.argv[3:]
m = json.loads(Path(src).read_text())
m["operations"] = {"database": ["db/" + Path(f).name for f in files]}
m.pop("icon", None)
m["kind"] = ["database"]
(dst / "swofty.json").write_text(json.dumps(m, indent=1))
PY
}
build() {
    local name=$1 season=$2 D=$W/data-$1
    rm -rf "$D"; mkdir -p "$D/baseline"
    cp -al "$BASE/baseline/." "$D/baseline/" 2>/dev/null || cp -a "$BASE/baseline/." "$D/baseline/"
    export SWOFTY_USAGE=0
    "${PIN[@]}" "$CLI" --data-dir "$D" addons install "$W/core" > "$D.log" 2>&1
    "${PIN[@]}" "$CLI" --data-dir "$D" addons install "$season" >> "$D.log" 2>&1
    for id in swofty.classic-seasons "$ID"; do "${PIN[@]}" "$CLI" --data-dir "$D" --profile cmp addons enable "$id" >> "$D.log" 2>&1; done
    "${PIN[@]}" "$CLI" --data-dir "$D" --profile cmp build --game "$GAME" >> "$D.log" 2>&1
}
addon "$W/core" "$CS/classic-seasons/swofty.json" "$CS/classic-seasons/db/core.json"
python3 - "$CS/classic-seasons-$YEAR/db" "$W/all.json" <<'PY'
import json, sys
from pathlib import Path
d = Path(sys.argv[1])
s, r = json.loads((d / "season.json").read_text()), json.loads((d / "records.json").read_text())
Path(sys.argv[2]).write_text(json.dumps({"schema_version": 1, "fields": {**r["fields"], **s["fields"]}, "ops": s["ops"] + r["ops"]}))
PY
mv "$W/all.json" "$W/season.json"
addon "$W/a" "$CS/classic-seasons-$YEAR/swofty.json" "$W/season.json"
addon "$W/a15" "$CS/classic-seasons-$YEAR/swofty.json" "$CS/classic-seasons-$YEAR/db/season.json" "$CS/classic-seasons-$YEAR/db/records.json"
"$TOOL" "$REPO/packages/sdk/std/registries" "$STOCK" "$W/lowered.json" "$ID" "$CS/classic-seasons-$YEAR/registry/content.json" > "$W/lowered.log"
addon "$W/b" "$CS/classic-seasons-$YEAR/swofty.json" "$CS/classic-seasons-$YEAR/db/season.json" "$W/lowered.json"
for s in a a15 b; do build $s "$W/$s"; done
LABELS=(--by-label entrants.vehicles.vehicle_N --by-label entrants.vehicles.vehicle_packages.vehicle_package_N
        --by-label audio.audio_bundle_settings.audio_bundle_settings_N --by-label car_setup.presets.vehicle_package_setups_N)
echo "== $YEAR: the 1.5 path (split files) against one file"; python3 "$CS/test/rows.py" "$W/data-a" "$W/data-a15" || true
echo "== $YEAR: hand-written operations plus the core's records against 0.6.x, by index"; python3 "$CS/test/rows.py" "$W/data-a" "$W/data-b" --show 3 || true
echo "== $YEAR: the same, rows matched by label"; python3 "$CS/test/rows.py" "$W/data-a" "$W/data-b" "${LABELS[@]}" --show 40 || true
