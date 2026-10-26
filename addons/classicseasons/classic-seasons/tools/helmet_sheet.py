#!/usr/bin/env python3
"""Contact sheet of the helmet_d every season driver's look wears, decoded from the archive the look's helmet loads:
the core's helmets/ (build_helmets.py) and suits/ (build_real_suits.py), else the stock F1 25 archive. Run after
gen.py; each tile names the driver and the helmet label.

    python3 tools/helmet_sheet.py --game <F1 25> --out <png> [--extra <label>=<archive>,...]
"""

import argparse
import json
import sys
from pathlib import Path

from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path[:0] = [str(HERE), str(ROOT.parents[2] / "tools")]

import bcn
import erpw
import f1_23_suits
from build_suits import font
from layout import helmets, real_suits, year_dir

LOOKS = "entrants.driver_customisations.folder[_metadata_=official_driver_customisations]"
DRIVERS = "character_package/drivers/male"
TILE = (384, 192)


def archives(game):
    out = {m["label"]: ROOT / m["source"] for m in helmets().values()}
    for m in real_suits():
        if m.get("helmet"):
            out[m["label"]] = ROOT / m["helmet_source"]
    return out


def archive_of(label, known, game):
    if label in known:
        return known[label]
    root = Path(game) / "2025_asset_groups" / DRIVERS
    hits = sorted((root / label).glob("*_helmet.erp")) or \
        sorted((root / label.replace("classic-", "classic-custom-", 1)).glob("*_helmet.erp"))
    if not hits:
        raise SystemExit(f"{label}: no helmet archive")
    return hits[0]


def helmet_d(path, game):
    e = erpw.Erp(str(path))
    hits = [r for n, r in f1_23_suits.surfaces(e).items() if n.endswith("_helmet_d.tif.image")]
    if len(hits) != 1:
        raise SystemExit(f"{path}: {len(hits)} helmet_d surfaces")
    r = hits[0]
    frags = [e.frag_data(r, i) for i in range(len(r.frags))]
    fmt, w, h, n = bcn.Surface.header(frags[0])
    if w >= 2 * TILE[0] or len(frags) < 3 or not frags[2] or not frags[2][0]:
        sizes = bcn.level_sizes(fmt, w, h, n)
        k = 0
        while k + 1 < n and (w >> (k + 1)) >= TILE[0]:
            k += 1
        at = sum(sizes[:k])
        return bcn.decode(fmt, frags[1][at:at + sizes[k]], max(1, w >> k), max(1, h >> k))
    return bcn.top_level(frags, [ROOT / "suits" / "sidecars", game])


def tile(img):
    im = Image.fromarray(img).convert("RGB")
    return im.resize(TILE, Image.LANCZOS)


def looks():
    out = []
    for year in range(2016, 2100):
        path = year_dir(year) / "db" / "season.json"
        if not path.exists():
            break
        for o in json.loads(path.read_text())["ops"]:
            if o["op"] == "add-row" and o["table"] == LOOKS:
                out.append((year, o["set"]["linked_driver_stat_id"], o["set"]["helmet"]))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", default="/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
    ap.add_argument("--out", required=True)
    ap.add_argument("--extra", default="", help="<label>=<archive>,...: tiles of other helmets in a last row")
    args = ap.parse_args()
    known = archives(args.game)
    rows = {}
    for year, driver, label in looks():
        rows.setdefault(year, []).append((driver, label, archive_of(label, known, args.game)))
    if args.extra:
        rows["extra"] = [(lab, lab, Path(p)) for lab, p in (x.split("=", 1) for x in args.extra.split(","))]
    cols = max(len(v) for v in rows.values())
    per = (cols + 1) // 2
    cw, ch = TILE[0] + 8, TILE[1] + 44
    lines = sum((len(v) + per - 1) // per for v in rows.values())
    sheet = Image.new("RGB", (120 + per * cw, lines * ch + 10), (30, 31, 35))
    d = ImageDraw.Draw(sheet)
    cache = {}
    y = 10
    for year, items in rows.items():
        d.text((10, y + 80), str(year), fill=(235, 235, 235), font=font(28))
        for i, (driver, label, path) in enumerate(items):
            if i and i % per == 0:
                y += ch
            x = 120 + (i % per) * cw
            if path not in cache:
                cache[path] = tile(helmet_d(path, args.game))
            sheet.paste(cache[path], (x, y + 40))
            d.text((x, y + 2), driver, fill=(235, 235, 235), font=font(16))
            d.text((x, y + 21), label, fill=(150, 190, 255), font=font(14))
        y += ch
        print(year, len(items), flush=True)
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    sheet.save(args.out, optimize=True)


if __name__ == "__main__":
    main()
