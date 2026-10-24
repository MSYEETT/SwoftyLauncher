#!/usr/bin/env python3
"""Team-wear colours for every season team, sampled from its car's paint texture.

Each built car archive has `<folder>_paint_d.tif` (or `_paint_c.tif`), the car's base livery in the car's UV layout. The sampler decodes
it (BC1, BC3 or BC7; `paint_c` when `paint_d` is a placeholder of 8 px or less), keeps the pixels of flat colour (paint panels; metal, carbon, decals and parts are textured), merges colours closer than MERGE in RGB distance and ranks the rest by area:
the three largest are the team's primary, secondary and tertiary team-wear colours. A team's `team_wear` list in
data/seasons.json (hex strings) replaces the sample. Writes data/team_colours.json and, with --sheet, a swatch sheet.

    python3 tools/team_colours.py [--sheet out.png]
"""

import argparse
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "rnd-tyres-thermal" / "tools"))

import erpw
from layout import ROOT, cars_dir, extra_package, package

SIDE = 128
FLAT = 10
MERGE = 48
OUT = ROOT / "data" / "team_colours.json"


BC1, BC3, BC7 = 54, 57, 70


def bc1_block(data, o, four=False):
    c0, c1, bits = struct.unpack_from("<HHI", data, o)
    p0 = (((c0 >> 11) & 31) * 255 // 31, ((c0 >> 5) & 63) * 255 // 63, (c0 & 31) * 255 // 31)
    p1 = (((c1 >> 11) & 31) * 255 // 31, ((c1 >> 5) & 63) * 255 // 63, (c1 & 31) * 255 // 31)
    if c0 > c1 or four:
        pal = (p0, p1, tuple((2 * a + b) // 3 for a, b in zip(p0, p1)), tuple((a + 2 * b) // 3 for a, b in zip(p0, p1)))
    else:
        pal = (p0, p1, tuple((a + b) // 2 for a, b in zip(p0, p1)), (0, 0, 0))
    return pal[(bits >> 10) & 3]


def paint(year, team):
    pkg = extra_package(year) if "car" in team else package(year)
    sub = "extra/teams" if "car" in team else "teams"
    folder = team["folder"]
    e = erpw.Erp(str(cars_dir(year) / sub / folder / "wep" / f"{folder}.erp"))
    found = []
    for name in ("paint_c", "paint_d"):
        try:
            r = e.find(f"eaid://{pkg}/teams/{folder}/textures/{folder}_{name}.tif.image")
        except KeyError:
            continue
        if r is not None:
            fmt, w, h = struct.unpack_from("<III", e.frag_data(r, 0), 8)
            if fmt in (BC1, BC3, BC7):
                found.append((w * h, fmt, w, h, r))
    if not found:
        raise SystemExit(f"{year} {folder}: no BC1, BC3 or BC7 paint texture")
    _, fmt, w, h, r = found[-1] if found[-1][2] > 8 else max(found, key=lambda f: f[0])
    data = e.frag_data(r, 1)
    size = 8 if fmt == BC1 else 16
    side = min(SIDE, w // 4, h // 4)
    bw = w // 4
    picks = [((y * (h // 4)) // side) * bw + (x * bw) // side for y in range(side) for x in range(side)]
    if fmt == BC1:
        px = [bc1_block(data, b * size) for b in picks]
    elif fmt == BC3:
        px = [bc1_block(data, b * size + 8, four=True) for b in picks]
    else:
        from common import bc7_decode
        blocks = b"".join(data[b * size:(b + 1) * size] for b in picks)
        strip = len(picks) * 4
        rgba = bc7_decode(blocks, strip, 4)
        px = [tuple(rgba[(strip + 4 * i + 1) * 4:(strip + 4 * i + 1) * 4 + 3]) for i in range(len(picks))]
    return [px[y * side:(y + 1) * side] for y in range(side)]


def sample(grid):
    def dist(a, b):
        return sum((x - y) ** 2 for x, y in zip(a, b)) ** 0.5
    counts = {}
    n = len(grid)
    for y in range(n):
        for x in range(n):
            c = grid[y][x]
            near = [grid[y + dy][x + dx] for dy, dx in ((0, 1), (1, 0), (0, -1), (-1, 0)) if 0 <= y + dy < n and 0 <= x + dx < n]
            if max(dist(c, o) for o in near) > FLAT:
                continue
            q = tuple(v // 16 * 16 + 8 for v in c)
            counts[q] = counts.get(q, 0) + 1
    clusters = []
    for c, n in sorted(counts.items(), key=lambda kv: -kv[1]):
        for k in clusters:
            if dist(k["c"], c) < MERGE:
                k["n"] += n
                break
        else:
            clusters.append({"c": c, "n": n})
    clusters.sort(key=lambda k: -k["n"])
    return ["#%02x%02x%02x" % k["c"] for k in clusters[:3]]


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--sheet", type=Path)
    a = ap.parse_args(argv[1:])
    data = json.loads((ROOT / "data" / "seasons.json").read_text())
    out = {}
    for season in data["seasons"]:
        year = season["year"]
        for team in season["teams"]:
            key = f"cs{year % 100:02d}_{team.get('key', team['folder'])}"
            colours = team.get("team_wear") or sample(paint(year, team))
            while len(colours) < 3:
                colours.append(colours[-1])
            out[key] = {"name": team["name"], "year": year, "team_wear": colours[:3],
                        "source": "seasons.json" if team.get("team_wear") else "car paint"}
            print(key, out[key]["team_wear"], out[key]["source"], flush=True)
    OUT.write_text(json.dumps(out, indent=1) + "\n")
    if a.sheet:
        from PIL import Image, ImageDraw
        seasons = sorted({v["year"] for v in out.values()})
        cols = max(sum(v["year"] == y for v in out.values()) for y in seasons)
        cw, ch = 130, 70
        sheet = Image.new("RGB", (60 + cols * cw, len(seasons) * ch), (40, 40, 46))
        d = ImageDraw.Draw(sheet)
        for r, y in enumerate(seasons):
            d.text((8, r * ch + 28), str(y), fill=(230, 230, 230))
            for c, (k, v) in enumerate([kv for kv in out.items() if kv[1]["year"] == y]):
                x0 = 60 + c * cw
                for i, hexc in enumerate(v["team_wear"]):
                    d.rectangle((x0 + i * 40, r * ch + 6, x0 + i * 40 + 36, r * ch + 42), fill=hexc)
                d.text((x0, r * ch + 48), v["name"][:20], fill=(230, 230, 230))
        sheet.save(a.sheet)


if __name__ == "__main__":
    main(sys.argv)
