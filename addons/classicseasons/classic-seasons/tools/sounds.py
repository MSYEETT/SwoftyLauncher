"""Period engine sounds.

A car's model layer (<package>/teams/<folder>/wep/<folder>-carmodel.layer) names its engine sound bundle,
`\\x00\\nbundleName\\x00<len><name>.bdl\\x00`, which the game loads from audio/2025/bundles/<name>.bdl. Each season
team's `engine` in data/seasons.json is its engine maker; data/sounds.json names the bundle of each maker in the game
of that season (an old game's `bndl` version 0x6a bundles play in F1 25 as they are), or a stock F1 25 bundle
("stock"). The name is edited in place, so it keeps the length of the name the car has: the stock name when it has
that length, else cs<yy>_<maker> cut or padded with "_" to it (honda_2024's 10 letters: cs17_merce). Every name a
season's cars wear that is not stock is a copy of the maker's bundle in the season addon's sounds/ (not committed) with
sounds/meta.json; gen.py ships each with file.put to audio/2025/bundles/ and adds its audio.audio_bundle_settings row,
a copy of the maker's 2025 row. build_cars.py names the bundles when it builds a season; run on its own, the tool names
them in the built cars (only the season's own cs_/cx_ package layers) and copies the bundles. A rerun changes nothing.

    python3 tools/sounds.py --games <folder holding the old game folders> --f1-23 <F1 23> [--game <F1 25>]
        [--years 2016-2024] [--backup <folder for the original layers>]
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import erpw
from layout import ROOT, cars_dir, year_dir

DATA = json.loads((ROOT / "data" / "seasons.json").read_text())
SOURCES = json.loads((ROOT / "data" / "sounds.json").read_text())["seasons"]
BUNDLE = re.compile(rb"\x00\nbundleName\x00(.)([a-z0-9_]+)\.bdl\x00")
STOCK_BUNDLES = "audio/2025/bundles"


def bundle(year, maker, length):
    stock = SOURCES[str(year)].get("stock")
    if stock and len(stock.format(maker=maker)) == length:
        return stock.format(maker=maker)
    return f"cs{year % 100:02d}_{maker}"[:length].ljust(length, "_")


def is_stock(year, name):
    stock = SOURCES[str(year)].get("stock")
    return bool(stock) and any(name == stock.format(maker=m) for m in ("ferrari", "honda", "mercedes", "renault"))


def cars(year):
    season = next(s for s in DATA["seasons"] if s["year"] == year)
    out = []
    for team in season["teams"]:
        folder = team["folder"]
        sub = Path("extra") if "car" in team else Path()
        out.append((cars_dir(year) / sub / "teams" / folder / "wep" / f"{folder}.erp", team["engine"]))
    return out


def engines(year):
    season = next(s for s in DATA["seasons"] if s["year"] == year)
    return {team.get("key", team["folder"]): team["engine"] for team in season["teams"]}


def layers(e, year):
    prefixes = (f"eaid://cs_{year}_vehicle_package/", f"eaid://cx_{year}_vehicle_package/")
    return [r for r in e.res if r.name.startswith(prefixes) and r.name.endswith("-carmodel.layer")]


def apply(e, year, maker, backup=None):
    changes = []
    for r in layers(e, year):
        data = e.frag_data(r, 0)
        hits = list(BUNDLE.finditer(data))
        if len(hits) != 1:
            raise SystemExit(f"{r.name}: {len(hits)} bundleName values")
        old = hits[0].group(2).decode()
        new = bundle(year, maker, len(old))
        if new != old:
            if backup:
                backup(r.name, data)
            e.set_frag(r, 0, data[:hits[0].start(2)] + new.encode() + data[hits[0].end(2):])
        changes.append((r.name, old, new))
    if not changes:
        raise SystemExit(f"{year}: a car with no -carmodel.layer")
    return changes


def name_car(path, year, maker, backup_dir):
    e = erpw.Erp(str(path))

    def backup(name, data):
        target = Path(backup_dir) / str(year) / path.parent.relative_to(cars_dir(year)) / name.rsplit("/", 1)[1]
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)

    changes = apply(e, year, maker, backup if backup_dir else None)
    if any(old != new for _, old, new in changes):
        tmp = path.with_suffix(".erp.tmp")
        e.to_file(str(tmp))
        os.replace(tmp, path)
    return str(path), maker, changes


def source(year, maker, games, f1_23, game):
    src = SOURCES[str(year)]
    if "stock" in src:
        return Path(game) / STOCK_BUNDLES / f"{src['stock'].format(maker=maker)}.bdl"
    root = Path(f1_23) if src["game"] == "F1 23" else Path(games) / src["game"]
    return root / src["bundle"].format(maker=maker)


def copy_bundles(year, worn, games, f1_23, game):
    out = year_dir(year) / "sounds"
    meta = []
    keep = set()
    for name, maker in sorted(worn.items()):
        if is_stock(year, name):
            continue
        src = source(year, maker, games, f1_23, game)
        if not src.is_file():
            raise SystemExit(f"{year} {maker}: no {src}")
        target = out / f"{name}.bdl"
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists() or target.read_bytes() != src.read_bytes():
            shutil.copyfile(src, target)
        keep.add(target.name)
        meta.append({"name": name, "maker": maker, "game": SOURCES[str(year)].get("game", "F1 25"),
                     "source": SOURCES[str(year)].get("bundle", STOCK_BUNDLES + "/{maker}.bdl").format(maker=maker),
                     "bytes": target.stat().st_size, "sha256": hashlib.sha256(target.read_bytes()).hexdigest()})
    if out.exists():
        for stale in out.glob("*.bdl"):
            if stale.name not in keep:
                stale.unlink()
    if meta:
        (out / "meta.json").write_text(json.dumps({"bundles": meta}, indent=1) + "\n")
    elif (out / "meta.json").exists():
        (out / "meta.json").unlink()
    return meta


def parse_years(text):
    a, _, b = text.partition("-")
    return range(int(a), int(b or a) + 1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--years", default="2016-2024")
    ap.add_argument("--games", required=True, help="the folder holding the F1 2016 to F1 2022 game folders")
    ap.add_argument("--f1-23", required=True, help="the F1 23 folder")
    ap.add_argument("--game", default="/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
    ap.add_argument("--backup", help="a folder for each layer's original before its first change")
    ap.add_argument("--jobs", type=int, default=6)
    args = ap.parse_args()
    for year in parse_years(args.years):
        todo = cars(year)
        with ProcessPoolExecutor(args.jobs) as pool:
            done = list(pool.map(name_car, [p for p, _ in todo], [year] * len(todo), [m for _, m in todo],
                                 [args.backup] * len(todo)))
        worn = {}
        for path, maker, changes in done:
            for layer, old, new in changes:
                worn[new] = maker
                print(f"{year} {Path(path).parent.parent.name}: {old} -> {new}", flush=True)
        meta = copy_bundles(year, worn, args.games, args.f1_23, args.game)
        print(f"{year}: {len(meta)} bundle(s), {sum(m['bytes'] for m in meta)} bytes", flush=True)


if __name__ == "__main__":
    main()
