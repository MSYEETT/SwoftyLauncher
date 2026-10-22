#!/usr/bin/env python3
"""Give each season with its own common.erp its own tyre compounds archive.

A vehicle package finds its shared tyre models through its tyre compounds group: with the stock
f1_2025_vehicle_package/tyrecompounds group the seasons' cars draw the stock 18-inch tyres around their 13-inch rims.
The build copies the game's tyrecompounds.erp, renames its compounds file byte for byte to
`eaid://cs_<year>_vehicle_package/tyrecompounds/`, and writes cars/<year>/tyrecompounds/tyrecompounds.erp.
"""

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT.parent.parent / "tools"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import erpw
from build_cars import SOURCE, package, rewrite

STOCK = f"2025_asset_groups/{SOURCE}/tyrecompounds/tyrecompounds.erp"


def build(year, game):
    meta = ROOT / "cars" / str(year) / "meta.json"
    if not meta.exists() or json.loads(meta.read_text())["common"] != "own":
        return None
    e = erpw.Erp(str(game / STOCK))
    old = f"eaid://{SOURCE}/".encode()
    new = f"eaid://{package(year)}/".encode()
    rewrite(e, re.compile(re.escape(old) + rb"[^\x00]*"), lambda m: new + m.group(0)[len(old):])
    out = ROOT / "cars" / str(year) / "tyrecompounds" / "tyrecompounds.erp"
    out.parent.mkdir(parents=True, exist_ok=True)
    e.to_file(str(out))
    return [r.name for r in e.res]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", default="/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
    ap.add_argument("--years", default="2016-2024")
    a = ap.parse_args()
    lo, _, hi = a.years.partition("-")
    for year in range(int(lo), int(hi or lo) + 1):
        names = build(year, Path(a.game))
        print(f"{year}: " + ("stock common, skipped" if names is None else ", ".join(names)), flush=True)


if __name__ == "__main__":
    main()
