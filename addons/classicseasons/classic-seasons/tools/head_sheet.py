#!/usr/bin/env python3
"""Contact sheet of the face map (head_d) of every driver whose head data/seasons.json sets: a shipped head from
heads/ (tools/heads.py), or a stock generic head from the game folder.

usage: head_sheet.py --out <png> [--game <F1 25>] [--baseline <launcher baseline>/<build>]
"""

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))

import bcn
import heads
from heads import erpw
from stock import Stock

TILE = 320
COLS = 5


def drivers():
    seasons = json.loads((heads.ROOT / "data" / "seasons.json").read_text())
    wears = {**seasons["head_overrides"], **{k: d["head"] for k, d in seasons["new_drivers"].items()}}
    years = {}
    for season in seasons["seasons"]:
        for team in season["teams"]:
            for driver, _ in team["drivers"]:
                years.setdefault(driver, []).append(season["year"])
    return sorted(((d, heads.worn(h), years.get(d, [])) for d, h in wears.items()), key=lambda x: (x[2][:1], x[0]))


def archive(head, stock):
    if head in heads.FACES:
        return heads.OUT / f"2025_asset_groups/{heads.SCENES}/heads/{heads.label(head)}.erp"
    if head in heads.shipped():
        return heads.OUT / heads.archives(head)[0]
    return stock.path(f"2025_asset_groups/{heads.SCENES}/heads/{head}.erp")


def face(path, game):
    e = erpw.Erp(str(path))
    best = None
    for r in e.res:
        if not r.name.endswith("_head_d.tif.image") or "/heads/male_" not in r.name:
            continue
        img = bcn.top_level([heads.frag(r, i) for i in range(len(r.frags))], [heads.OUT, game])
        if best is None or img.shape[0] > best[1].shape[0]:
            best = (r.name.rsplit("/", 1)[-1], img)
    if best is None:
        sys.exit(f"{path}: no head_d surface")
    return best


def span(years):
    if not years:
        return "-"
    return str(years[0]) if len(years) == 1 else f"{years[0]}-{years[-1]}" if years == list(
        range(years[0], years[-1] + 1)) else ", ".join(map(str, years))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--game", default="/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
    ap.add_argument("--baseline", default=heads.BASELINE)
    a = ap.parse_args()
    baseline = Path(a.baseline).expanduser()
    stock = Stock(a.game, baseline if (baseline / "baseline.json").is_file() else None)
    rows = drivers()
    sheet = Image.new("RGB", (TILE * COLS, (TILE + 34) * ((len(rows) + COLS - 1) // COLS)), (24, 24, 24))
    draw = ImageDraw.Draw(sheet)
    for n, (driver, head, years) in enumerate(rows):
        name, img = face(archive(head, stock), a.game)
        x, y = TILE * (n % COLS), (TILE + 34) * (n // COLS)
        sheet.paste(Image.fromarray(np.asarray(img)[..., :3]).resize((TILE, TILE), Image.LANCZOS), (x, y + 34))
        label = heads.label(head) if head in heads.shipped() else head
        draw.text((x + 4, y + 3), f"{driver} ({span(years)})", fill=(255, 255, 255))
        draw.text((x + 4, y + 18), f"{label}: {name} {img.shape[1]}px", fill=(200, 200, 120))
        print(f"{driver:20} {span(years):22} {label:22} {name} {img.shape[1]}x{img.shape[0]}")
    a.out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(a.out)


if __name__ == "__main__":
    main()
