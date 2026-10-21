#!/usr/bin/env python3
import argparse
import glob
import json
import os
import struct
import subprocess
import sys
from concurrent.futures import ProcessPoolExecutor
from functools import lru_cache
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
REPO = ROOT.parents[1]
sys.path[:0] = [str(HERE), str(REPO / "tools"), str(REPO / "addons/rnd-tyres-thermal/tools")]

import bc7enc
import grid
from erp import parse as erp_parse
from erpw import Erp
from pkg import Pkg
from season import slots
from stock import Stock, fragments

UI = "2025_asset_groups/ui_package/"
DATABASES = UI + "databases.erp"
OVERRIDES = "eaid://ui_package/databases/overrides.uidb"
ARRAY = "eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.{}.texarray"
BC7 = 70
FONT = "DejaVu Sans:style=Condensed Bold"
ARRAYS = {
    "team_logos.f1_2026": "row",
    "team_logos.driver_career_2024_team_select": "row",
    "teams.mp_team_select_logos": "row",
    "teams.badges_broadcast": "row",
    "broadcast_ui_generic.grid_team_logos": "row",
    "broadcast_ui_generic.team": "row",
    "teams.fanzone_expressions": "row",
    "teams.fanzone_expressions_large": "row",
    "teams.fanzone_room": "row",
    "teams.teams_f1_2024_gp_voting": "row",
    "teams.driver_select_team_logos": "livery",
    "teams.career_team_select_logo_small": "select",
    "osd.timing_tower.team_badges_hud": "tower",
    "osd.timing_tower.team_badges_broadcast": "tower",
}
OUT = ROOT / "erp/badges"


def key(kind, row):
    return f"team_select_{row}" if kind == "select" else row


def carriers(game, names):
    found = {n: [] for n in names}
    for e in sorted(glob.glob(str(game / UI / "*.erp")) + glob.glob(str(game / UI / "screens/*.erp"))):
        _, res = erp_parse(e)
        for r in res:
            if r[0] in found:
                found[r[0]].append(os.path.relpath(e, game))
    return found


def overrides_list(root, path):
    node = root["overrides"]["Contents"]
    parts = path.split(".")
    for p in parts[:-1]:
        node = node[p]["Contents"]
    return [e["Name"].value for e in node[parts[-1]]["Contents"].items]


def rgb(hexcolour):
    return tuple(int(hexcolour[i:i + 2], 16) for i in (1, 3, 5))


def luminance(c):
    return (0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]) / 255


@lru_cache(None)
def font_file():
    return subprocess.run(["fc-match", "-f", "%{file}", FONT], capture_output=True, text=True, check=True).stdout


def text_mask(text, cap):
    font = ImageFont.truetype(font_file(), 300)
    l, t, r, b = font.getbbox(text)
    im = Image.new("L", (r - l + 20, b - t + 20), 0)
    ImageDraw.Draw(im).text((10 - l, 10 - t), text, font=font, fill=255)
    im = im.crop(im.getbbox())
    h_cap = font.getbbox("H")[3] - font.getbbox("H")[1]
    scale = cap / h_cap
    return im.resize((max(1, round(im.width * scale)), max(1, round(im.height * scale))), Image.LANCZOS)


def badge(team, w, h):
    _, short, code, colours, *_ = grid.TEAM_INFO[team]
    primary, secondary = rgb(colours[0]), rgb(colours[1])
    ink = (255, 255, 255) if luminance(primary) < 0.6 else (26, 26, 26)
    if secondary == ink or abs(luminance(secondary) - luminance(primary)) < 0.08:
        secondary = rgb(colours[2])
    tiny = min(w, h) <= 32
    use_code = min(w, h) <= 64
    s = max(1, min(8, round(480 / max(w, h))))
    W, H = w * s, h * s
    shape = Image.new("L", (W, H), 0)
    margin = 0 if tiny else round(0.04 * min(W, H))
    radius = round((0.18 if tiny else 0.10) * min(W, H))
    ImageDraw.Draw(shape).rounded_rectangle((margin, margin, W - 1 - margin, H - 1 - margin), radius, fill=255)
    art = Image.new("RGB", (W, H), primary)
    band = round((0.22 if tiny else 0.15) * H)
    ImageDraw.Draw(art).rectangle((0, H - margin - band, W, H), fill=secondary)
    top, bottom = margin, H - margin - band
    label = code if use_code else ("McLAREN" if team == "mclaren" else short.upper())
    avail_w = (W - 2 * margin) * (0.86 if tiny else 0.84)
    lines = [label]
    cap = (bottom - top) * (0.62 if tiny else 0.50 if use_code else 0.42)
    mask = text_mask(label, cap)
    if not use_code and mask.width > avail_w and " " in label:
        lines = label.split(" ", 1)
        cap = (bottom - top) * 0.30
    masks = [text_mask(t, cap) for t in lines]
    masks = [m if m.width <= avail_w else m.resize((round(avail_w), m.height), Image.LANCZOS) for m in masks]
    gap = round(cap * 0.35)
    total = sum(m.height for m in masks) + gap * (len(masks) - 1)
    y = top + (bottom - top - total) // 2
    for m in masks:
        art.paste(Image.new("RGB", m.size, ink), ((W - m.width) // 2, y), m)
        y += m.height + gap
    rgba = art.convert("RGBA")
    rgba.putalpha(shape)
    rgba = rgba.resize((w, h), Image.LANCZOS)
    covered = rgba.getchannel("A").point(lambda v: 255 if v else 0)
    return Image.composite(rgba, Image.new("RGBA", (w, h), primary + (0,)), covered)


def encode(job):
    team, w, h = job
    im = badge(team, w, h)
    return team, w, h, bc7enc.encode_rgba(im.tobytes(), w, h), im


def build(game, baseline, sheet):
    stock = Stock(game, baseline)
    db = stock.path(DATABASES)
    root = Pkg(fragments(db, OVERRIDES)[0]).root
    teams = [(s.team, s.team_row) for s in slots()]
    names = [ARRAY.format(p) + suffix for p in ARRAYS for suffix in (".image", "")]
    where = carriers(game, names)

    plans, jobs = {}, set()
    for path, kind in ARRAYS.items():
        image, view = ARRAY.format(path) + ".image", ARRAY.format(path)
        if not where[image] or where[image] != where[view]:
            sys.exit(f"{path}: carriers {where[image]} / {where[view]}")
        frags = None
        for rel in where[image]:
            got = fragments(stock.path(rel), image) + fragments(stock.path(rel), view)
            if frags is None:
                frags = got
            elif got != frags:
                sys.exit(f"{path}: {rel} carries a different stock array")
        head, data, view0 = frags
        fmt, w, h = struct.unpack_from("<III", head, 8)
        mips, count = struct.unpack_from("<II", head, 24)
        names_ = overrides_list(root, path)
        if fmt != BC7 or mips != 1 or len(data) != count * w * h or len(names_) != count \
                or struct.unpack_from("<I", view0, 0x14)[0] != count:
            sys.exit(f"{path}: unexpected stock array (format {fmt}, {w}x{h}, {mips} mips, {count} slices)")
        append = [(key(kind, grid.team_row(team)), team) for team, _ in teams]
        taken = [k for k, _ in append if k in names_]
        if taken:
            sys.exit(f"{path}: already lists {taken}")
        plans[path] = (where[image], head, data, view0, w, h, count, append, names_)
        jobs |= {(t, w, h) for _, t in append}

    with ProcessPoolExecutor(max_workers=int(os.environ.get("JOBS", "8"))) as pool:
        encoded = {(t, w, h): (px, im) for t, w, h, px, im in pool.map(encode, sorted(jobs))}

    if OUT.exists():
        for p in OUT.iterdir():
            p.unlink()
    OUT.mkdir(parents=True, exist_ok=True)
    grown, ops, edits, report = {}, [], [], []
    for path, (rels, head, data, view0, w, h, count, append, names_) in plans.items():
        image, view = ARRAY.format(path) + ".image", ARRAY.format(path)
        n = len(append)
        head2 = bytearray(head)
        struct.pack_into("<I", head2, 28, count + n)
        view2 = bytearray(view0)
        struct.pack_into("<I", view2, 0x14, count + n)
        grown[path] = (bytes(head2), data + b"".join(encoded[(t, w, h)][0] for _, t in append), bytes(view2))
        for rel in rels:
            for res in (image, view):
                ops.append({"op": "put-resource", "erp": rel, "resource": res, "source": "erp/badges/badges.erpk"})
        at = "overrides.Contents." + ".Contents.".join(path.split(".")) + ".Contents"
        for k, team in append:
            edits.append({"op": "clone-entry", "at": at, "src": f"[Name={names_[-1]}]", "as": k,
                          "set": {"Value": f"ui_package/art/textures/f1990/badges/{k}.tif"}})
        report.append(f"{path}: {w}x{h}, {count} + {n} slices, {len(rels)} archive(s)")

    write_archive(stock, plans, grown, OUT / "badges.erpk")
    ops.append({"op": "patch-pkg", "erp": DATABASES, "resource": OVERRIDES, "fragment": 0, "edits": edits})
    (ROOT / "erp/badges.json").write_text(json.dumps({"schema_version": 1, "ops": ops}, indent=1) + "\n")
    if sheet:
        contact_sheet(plans, encoded, sheet)
    return report


def write_archive(stock, plans, grown, out):
    keep, arc = [], None
    for path, (rels, *_rest) in plans.items():
        e = Erp(str(stock.path(rels[0])))
        image, view = ARRAY.format(path) + ".image", ARRAY.format(path)
        r_image, r_view = e.find(image), e.find(view)
        h2, d2, v2 = grown[path]
        e.set_frag(r_image, 0, h2)
        e.set_frag(r_image, 1, d2)
        e.set_frag(r_view, 0, v2)
        keep += [r_image, r_view]
        if arc is None:
            arc = e
    arc.res = keep
    arc.to_file(str(out))


def contact_sheet(plans, encoded, out):
    from common import bc7_decode
    rows = []
    for path, (rels, head, data, view0, w, h, count, append, names_) in plans.items():
        teams = [t for _, t in append]
        zoom = max(1, min(4, 96 // max(w, h))) if max(w, h) < 96 else 1
        scale = min(1.0, 180 / max(w, h)) if zoom == 1 else zoom
        cells = []
        for t in teams:
            px = encoded[(t, w, h)][0]
            im = Image.frombytes("RGBA", (w, h), bc7_decode(px, w, h))
            cells.append(im.resize((max(1, round(w * scale)), max(1, round(h * scale))),
                                   Image.NEAREST if zoom > 1 else Image.LANCZOS))
            if zoom > 1:
                cells.append(im)
        rows.append((f"{path}  {w}x{h}" + (f"  (x{zoom} and actual size)" if zoom > 1 else ""), cells))
    pad, label_h = 8, 22
    width = max(sum(c.width + pad for c in cells) + pad for _, cells in rows)
    height = sum(label_h + max(c.height for c in cells) + pad for _, cells in rows) + pad
    sheet = Image.new("RGBA", (width, height), (70, 70, 74, 255))
    d = ImageDraw.Draw(sheet)
    y = pad
    for label, cells in rows:
        d.text((pad, y), label, fill=(240, 240, 240, 255))
        y += label_h
        x = pad
        for c in cells:
            sheet.alpha_composite(c, (x, y))
            x += c.width + pad
        y += max(c.height for c in cells) + pad
    sheet.convert("RGB").save(out)


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", required=True, type=Path)
    ap.add_argument("--baseline", type=Path, help="launcher baseline folder holding baseline.json (stock copies)")
    ap.add_argument("--sheet", type=Path, help="write a contact sheet PNG of the encoded badges")
    a = ap.parse_args(argv[1:])
    for line in build(a.game, a.baseline, a.sheet):
        print(line)


if __name__ == "__main__":
    main(sys.argv)
