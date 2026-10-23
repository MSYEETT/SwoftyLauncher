#!/usr/bin/env python3
"""Build each season's in-car driver archive from fanfreluche's zips.

The zips replace animation_package/scenes/incardriver/2025/incardriver_f1.erp, whose scene maps every car's steering
animation (driver_in_car_<year>_<team>.idf, shipped in the same archive). Each season keeps its own copy under
incardriver/cs<yy>/: the scene `eaid://animation_package/scenes/incardriver/2025/` is renamed byte for byte to
`.../incardriver/cs<yy>/`, which the season's driver animation set loads. Writes the season addon's
cars/incardriver/incardriver_f1.erp.
"""

import argparse
import os
import re
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT.parents[2] / "tools"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import erpw
from build_cars import find_zip, rewrite
from layout import cars_dir

OLD = b"eaid://animation_package/scenes/incardriver/2025/"
MEMBER = "2025_asset_groups/animation_package/scenes/incardriver/2025/incardriver_f1.erp"


def folder(year):
    return f"cs{year % 100:02d}"


def build(year, src_zip):
    with zipfile.ZipFile(src_zip) as z, tempfile.TemporaryDirectory() as tmp:
        hits = [n for n in z.namelist() if n.endswith("/" + MEMBER) or n == MEMBER]
        if not hits:
            return None
        e = erpw.Erp(z.extract(hits[0], tmp))
        new = OLD.replace(b"/2025/", f"/{folder(year)}/".encode())
        pattern = re.compile(re.escape(OLD) + rb"[^\x00]*")
        renamed, rewritten = rewrite(e, pattern, lambda m: new + m.group(0)[len(OLD):])
        scene = f"{new.decode()}incardriver_f1.scene"
        if not any(r.name == scene for r in e.res):
            raise SystemExit(f"{year}: no {scene} after the rename")
        out = cars_dir(year) / "incardriver" / "incardriver_f1.erp"
        out.parent.mkdir(parents=True, exist_ok=True)
        e.to_file(str(out))
        return renamed, rewritten, out.stat().st_size


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--downloads", default=os.path.expanduser("~/Downloads"))
    ap.add_argument("--years", default="2016-2024")
    a = ap.parse_args()
    lo, _, hi = a.years.partition("-")
    for year in range(int(lo), int(hi or lo) + 1):
        r = build(year, find_zip(a.downloads, year))
        print(f"{year}: " + ("no in-car archive" if r is None else f"{r[0]} renamed, {r[1]} rewritten, {r[2]} bytes"),
              flush=True)


if __name__ == "__main__":
    main()
