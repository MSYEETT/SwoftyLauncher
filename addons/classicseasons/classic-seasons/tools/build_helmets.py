#!/usr/bin/env python3
"""Season helmets (data/helmets.json): a mod's helmet textures on F1 25 helmet archives, built into the season addon's
helmets/ (not committed) with helmets/meta.json.

F1 25's helmet shells (Bell HP7, Arai GP-6, Schuberth SF2 Pro, Stilo ST5 and ST6) share one texture layout, so a
helmet texture fits every shell; an entry's base is an F1 25 look whose shell is the brand on the helmet. The base's
helmet archive is relabelled from the base look to cs<yy>-<the base look without its year> (the same length), and its
helmet_d, helmet_n and helmet_s take the mod's textures, every level embedded with no sidecar: helmet_d resized to the
base's full size (the top half of a square texture), helmet_n and helmet_s with the mod's own levels. The helmet group
lists the base archive first, which holds the shell, the wing and the visor parts the copy names.

    python3 tools/build_helmets.py [--downloads ~/Downloads] [--game <F1 25>]
"""

import argparse
import json
import os
import re
import shutil
import struct
import sys
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path[:0] = [str(HERE), str(ROOT.parents[2] / "tools")]

import bcn
import f1_23_suits
from build_real_suits import relabel
from build_suits import put_texture
from layout import year_dir

DATA = json.loads((ROOT / "data" / "helmets.json").read_text())
DRIVERS = "character_package/drivers/male"
DXGI = {72: bcn.BC1[1], 71: bcn.BC1[0], 83: bcn.BC5[0]}
FOURCC = {b"DXT1": bcn.BC1[0], b"ATI2": bcn.BC5[0], b"BC5U": bcn.BC5[0]}


def label_of(entry):
    base = entry["base"]
    if not re.match(r"^20\d\d-", base):
        raise SystemExit(f"{entry['driver']}: base {base} has no year to relabel")
    return f"cs{entry['season'] % 100:02d}-{base[5:]}"


def paths(entry):
    base, label = entry["base"], label_of(entry)
    stem = base.split("_", 1)[1]
    return {"label": label, "stem": stem,
            "target": f"2025_asset_groups/{DRIVERS}/{label}/{stem}_helmet.erp",
            "group": f"asset_groups/{DRIVERS}/{label}/{stem}_helmet.assetgroup",
            "base_erp": f"2025_asset_groups/{DRIVERS}/{base}/{stem}_helmet.erp",
            "template": f"asset_groups/{DRIVERS}/{base}/{stem}_helmet.assetgroup"}


def dds(data):
    h, w, levels = struct.unpack_from("<III", data, 12)[0], struct.unpack_from("<I", data, 16)[0], \
        max(1, struct.unpack_from("<I", data, 28)[0])
    fourcc = data[84:88]
    if fourcc == b"DX10":
        fmt, at = DXGI.get(struct.unpack_from("<I", data, 128)[0]), 148
    else:
        fmt, at = FOURCC.get(fourcc), 128
    if fmt is None:
        raise SystemExit(f"a DDS of format {fourcc} {data[128:132].hex()}")
    out = []
    for size in bcn.level_sizes(fmt, w, h, levels):
        out.append(data[at:at + size])
        at += size
    return fmt, w, h, out


def colour_image(fmt, w, h, levels, size):
    img = bcn.decode(fmt, levels[0], w, h)
    if w == h:
        img = img[:h // 2]
    return np.asarray(Image.fromarray(img).resize(size, Image.LANCZOS))


def build(game, downloads):
    report, made = [], {}
    by_year = {}
    for entry in DATA["helmets"]:
        by_year.setdefault(entry["season"], []).append(entry)
    db = f1_23_suits.Db(game)
    for year, entries in by_year.items():
        out = year_dir(year) / "helmets"
        if out.exists():
            shutil.rmtree(out)
        built = []
        for entry in entries:
            mod = DATA["mods"][entry["mod"]]
            p = paths(entry)
            if p["label"] in made:
                raise SystemExit(f"{p['label']}: the helmet of {entry['driver']} and of {made[p['label']]}")
            made[p["label"]] = entry["driver"]
            with zipfile.ZipFile(Path(downloads) / mod["zip"]) as z:
                read = lambda key: z.read(f"{mod['root']}/{mod[key].format(driver=entry['driver'])}")
                colour, spec, normal = dds(read("colour")), dds(read("spec")), dds(read("normal"))
            base_erp = Path(game) / p["base_erp"]
            e, _ = relabel(base_erp, base_erp, entry["base"], p["label"], None, {}, game, report)
            prefix = f"eaid://{DRIVERS}/{p['label']}/textures/{p['stem']}_helmet_"
            surfaces = f1_23_suits.surfaces(e)
            r = surfaces[prefix + "d.tif.image"]
            fmt25, w25, h25 = f1_23_suits.full_size(e, r)
            head, data, n = bcn.surface_frags(e.frag_data(r, 0), fmt25, colour_image(*colour, (w25, h25)))
            put_texture(e, r.name, head, data, n, fmt25)
            for part, (fmt, w, h, levels) in (("n", normal), ("s", spec)):
                f1_23_suits.put_chain(e, surfaces[prefix + f"{part}.tif.image"], fmt, w, h, levels)
            missing = f1_23_suits.unresolved(e, f1_23_suits.erpw.Erp(str(base_erp)))
            if missing:
                raise SystemExit(f"{p['label']}: names {len(missing)} resource(s) neither archive holds: {missing[:4]}")
            source = f"helmets/{p['label']}/{p['stem']}_helmet.erp"
            (year_dir(year) / source).parent.mkdir(parents=True, exist_ok=True)
            e.to_file(str(year_dir(year) / source))
            built.append({**entry, **p, "source": source, "helmet_colours": db.helmet_colours(entry["base"])})
            print(f"{p['label']}: {year} {entry['driver']} from {mod['name']} on {entry['base']}", flush=True)
        (out / "meta.json").write_text(json.dumps({"helmets": built}, indent=1) + "\n")
    for line in report:
        print(line)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--downloads", default=os.path.expanduser("~/Downloads"))
    ap.add_argument("--game", default="/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
    args = ap.parse_args()
    if not (Path(args.game) / "F1_25.exe").is_file():
        sys.exit(f"--game {args.game} is not the F1 25 folder")
    build(args.game, args.downloads)


if __name__ == "__main__":
    main()
