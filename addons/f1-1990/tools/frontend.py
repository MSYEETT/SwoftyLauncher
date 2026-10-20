#!/usr/bin/env python3
import argparse
import json
import os
import re
import struct
import subprocess
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import lz4.block
from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
REPO = ROOT.parents[1]
sys.path[:0] = [str(HERE), str(REPO / "tools"), str(REPO / "addons/rnd-tyres-thermal/tools")]

from erp import parse as erp_parse
from pkg import Pkg
import bc7enc
import grid
from season import Season, slots

MFE = "2025_asset_groups/environment_package/second_world/main_front_end/wep/main_front_end.erp"
PERSISTENT = "2025_asset_groups/ui_package/persistent.erp"
SCREEN_GROUPS = "eaid://ui_package/screen_groups/screen_groups.uicontentgroup"
IDF = "eaid://environment_package/core_assets/objects/idf/main_front_end/"
TEX = "eaid://environment_package/core_assets/objects/textures/main_front_end/names/"
NAMES_DIR = "environment_package/core_assets/objects/textures/main_front_end/names/"
DRIVER_BIND = IDF + "mfe_2025_main_menu_driver_select_name.emb?context="
TEAM_BIND = IDF + "mfe_2025_main_menu_vehicle_select_name.emb?context="
SIDE_PREFIX = {"f1_driver_names~": "f1990_drv_names~", "f2_driver_names~": "f1990_drv_names~",
               "f1_team_names~": "f1990_tmnames~", "f2_team_names~": "f1990_tmnames~"}
SPARE_DRIVERS = ["zenith_driverfirst", "zenith_driversecond", "callie_mayer", "aiden_jackson", "jack_doohan", "yuki_tsunoda"]
SPARE_TEAMS = []
HIDDEN_TABS = ("header_f2", "header_icon")
FONT = "DejaVu Sans:style=Bold"
STRETCH = 1.25
OUT = ROOT / "erp/frontend"


class Archive:
    def __init__(self, path):
        self.path = path
        self.meta, res = erp_parse(str(path))
        self.res = {r[0]: r for r in res}

    def fragments(self, name):
        out = []
        with open(self.path, "rb") as f:
            for fn, off, size, flags, comp, packed in self.res[name][3]:
                f.seek(self.meta["data_off"] + off)
                raw = f.read(packed)
                if comp == 0x11:
                    raw = subprocess.run(["zstd", "-dc"], input=raw, capture_output=True, check=True).stdout
                out.append(raw)
        return out

    def names(self, prefix):
        return [n for n in self.res if n.startswith(prefix)]


def font_path():
    out = subprocess.run(["fc-match", "-f", "%{file}", FONT], capture_output=True, text=True, check=True).stdout
    return out


def line_mask(text, cap, outline):
    font = ImageFont.truetype(font_path(), 400)
    l, t, r, b = font.getbbox(text)
    im = Image.new("L", (r - l + 40, b - t + 40), 0)
    ImageDraw.Draw(im).text((20 - l, 20 - t), text, font=font, fill=255)
    cap_px = font.getbbox("H")[3] - font.getbbox("H")[1]
    im = im.crop(im.getbbox())
    scale = cap / cap_px
    im = im.resize((max(1, round(im.width * scale * STRETCH)), max(1, round(im.height * scale))), Image.LANCZOS)
    if outline:
        padded = Image.new("L", (im.width + 8, im.height + 8), 0)
        padded.paste(im, (4, 4))
        im = padded
        grown = im.filter(ImageFilter.MaxFilter(7))
        shrunk = im.filter(ImageFilter.MinFilter(7))
        im = ImageChops.subtract(grown, shrunk)
    return im


def fit(mask, width):
    return mask if mask.width <= width else mask.resize((width, mask.height), Image.LANCZOS)


def driver_image(w, h, first, last):
    s = 2
    W, H = w * s, h * s
    alpha = Image.new("L", (W, H), 0)
    right = W - round(0.049 * W)
    avail = right - round(0.03 * W)
    cap = round(0.33 * H)
    for text, top, outline in ((first, 0.148, False), (last, 0.518, True)):
        m = fit(line_mask(text, cap, outline), avail)
        alpha.paste(m, (right - m.width, round(top * H)), m)
    return alpha.resize((w, h), Image.LANCZOS)


def team_image(w, h, name):
    s = 2
    W, H = w * s, h * s
    alpha = Image.new("L", (W, H), 0)
    m = fit(line_mask(name, round(0.80 * H), True), round(0.93 * W))
    alpha.paste(m, ((W - m.width) // 2, round(0.098 * H)), m)
    return alpha.resize((w, h), Image.LANCZOS)


def blank_image(w, h):
    return Image.new("L", (w, h), 0)


def encode_texture(job):
    key, kind, args, hdr, f2, f1_len = job
    _, _, fmt, w0, h0, _, erp_levels = struct.unpack_from("<7I", hdr)
    ln = f2[0]
    path = f2[1:1 + ln].rstrip(b"\0").decode()
    at = 1 + ln + 1
    side_n = struct.unpack_from("<I", f2, at)[0]
    w, h = w0 << side_n, h0 << side_n
    alpha = {"driver": driver_image, "team": team_image, "blank": blank_image}[kind](w, h, *args)
    levels = []
    for i in range(side_n + erp_levels):
        lw, lh = w >> i, h >> i
        a = alpha.resize((lw, lh), Image.BOX) if i else alpha
        rgba = Image.merge("RGBA", (Image.new("L", (lw, lh), 255),) * 3 + (a,))
        levels.append(bc7enc.encode(rgba.tobytes(), lw, lh))
    f1 = b"".join(levels[side_n:])
    assert len(f1) == f1_len, (key, len(f1), f1_len)
    folder, file = path.rsplit("/", 1)
    prefix = next(p for p in SIDE_PREFIX if file.startswith(p))
    new_path = f"{folder}/{SIDE_PREFIX[prefix]}{file[len(prefix):]}"
    assert len(new_path) == len(path)
    side, entries, off = b"", [], 0
    for lvl in levels[:side_n]:
        packed = lz4.block.compress(lvl, store_size=False, mode="high_compression")
        entries.append(struct.pack("<BBQQQ", 0, 3, off, len(packed), len(lvl)))
        side += packed
        off += len(packed)
    new_f2 = bytearray(f2)
    new_f2[1:1 + len(new_path)] = new_path.encode()
    for i, e in enumerate(entries):
        new_f2[at + 4 + 26 * i:at + 4 + 26 * (i + 1)] = e
    return key, f1, bytes(new_f2), new_path, side, alpha


def texture(arc, bind, kind):
    material = re.search(rb"eaid://[^\0]+\.material", arc.fragments(bind)[0]).group().decode()
    tex = re.search(rb"names/(f[12]_%s_names)/(f[12]_%sname_[a-z0-9_]+)\.tif" % (kind.encode(), kind.encode()),
                    arc.fragments(material)[0])
    return f"{tex.group(1).decode()}/{tex.group(2).decode()}"


def team_label(team, short):
    return "McLAREN" if team == "mclaren" else short.upper()


def build(game, preview=None):
    arc = Archive(game / MFE)
    season = Season.load()
    _, _, report = grid.build(season)
    rows = report["rows"]
    drivers = {}
    for name in arc.names(DRIVER_BIND):
        drivers[name[len(DRIVER_BIND):]] = texture(arc, name, "driver")
    teams = {}
    for name in arc.names(TEAM_BIND):
        teams[name[len(TEAM_BIND):]] = texture(arc, name, "team")

    ops, jobs, binds = [], [], {}
    used = {drivers[l] for l in rows.values() if l in drivers and l != "lewis_hamilton"}
    blank = drivers["default"]
    spares = [c for c in SPARE_DRIVERS if drivers[c] not in used and drivers[c] != blank]
    spares += sorted(c for c, t in drivers.items()
                     if t.startswith("f2_") and t not in used and c not in spares)
    labels = {}
    for did, label in rows.items():
        d = season.drivers[did]
        name = (d.first.upper(), d.last.upper())
        if label in drivers and label != "lewis_hamilton":
            labels[label] = (drivers[label], name, None)
            continue
        size = len(arc.fragments(DRIVER_BIND + label)[0]) if label in drivers else None
        spare = next(c for c in spares if size is None or len(arc.fragments(DRIVER_BIND + c)[0]) == size)
        spares.remove(spare)
        labels[label] = (drivers[spare], name, spare)
    for label, (tex, name, spare) in labels.items():
        jobs.append((tex, "driver", name))
        if spare:
            binds[DRIVER_BIND + label] = arc.fragments(DRIVER_BIND + spare)[0]
    jobs.append((blank, "blank", ()))

    team_used = {teams[s.team_row] for s in slots() if s.team_row in teams}
    team_spares = [c for c in SPARE_TEAMS if c in teams and teams[c] not in team_used]
    team_spares += sorted(c for c, t in teams.items() if t.startswith("f2_") and t not in team_used
                          and c not in team_spares)
    seen = set()
    for s in slots():
        short = grid.TEAM_INFO[s.team][1]
        if s.team_row in teams:
            tex = teams[s.team_row]
        else:
            spare = next(c for c in team_spares if teams[c] not in seen and teams[c] not in team_used)
            tex = teams[spare]
            binds[TEAM_BIND + s.team_row] = arc.fragments(TEAM_BIND + spare)[0]
        seen.add(tex)
        jobs.append((tex, "team", (team_label(s.team, short),)))

    work = []
    for tex, kind, args in jobs:
        res = TEX + tex + ".tif.image"
        hdr, f1, f2 = arc.fragments(res)
        work.append((tex, kind, args, hdr, f2, len(f1)))
    with ProcessPoolExecutor(max_workers=int(os.environ.get("JOBS", "8"))) as pool:
        results = list(pool.map(encode_texture, work))

    if OUT.exists():
        for p in sorted(OUT.rglob("*"), reverse=True):
            p.unlink() if p.is_file() else p.rmdir()
    (OUT / "mipmaps").mkdir(parents=True)
    for tex, f1, f2, side_path, side, alpha in results:
        stem = tex.replace("/", "__")
        (OUT / f"{stem}.f1").write_bytes(f1)
        (OUT / f"{stem}.f2").write_bytes(f2)
        (OUT / "mipmaps" / side_path[len(NAMES_DIR):]).write_bytes(side)
        res = TEX + tex + ".tif.image"
        for frag in (1, 2):
            ops.append({"op": "replace-resource", "erp": MFE, "resource": res, "fragment": frag,
                        "source": f"erp/frontend/{stem}.f{frag}"})
        if preview:
            preview.mkdir(parents=True, exist_ok=True)
            alpha.save(preview / f"{stem}.png")
    ops.append({"op": "add-mipmaps", "source": "erp/frontend/mipmaps/", "target": NAMES_DIR})
    for name, data in sorted(binds.items()):
        stem = "bind__" + name.rsplit("/", 1)[1].replace(".emb?context=", "__")
        (OUT / stem).write_bytes(data)
        if name in arc.res:
            ops.append({"op": "replace-resource", "erp": MFE, "resource": name, "fragment": 0,
                        "source": f"erp/frontend/{stem}"})
        else:
            ops.append({"op": "add-resource", "erp": MFE, "resource": name, "type": "GfxMatBindRes",
                        "sources": [f"erp/frontend/{stem}"]})

    groups = Archive(game / PERSISTENT)
    raw = groups.fragments(SCREEN_GROUPS)[0]
    pkg = Pkg(raw)
    assert pkg.to_bytes() == raw
    removed = hide_tabs(pkg)
    edited = pkg.to_bytes()
    assert removed == len(HIDDEN_TABS)
    check_tabs(raw, edited)
    (OUT / "screen_groups.uicontentgroup").write_bytes(edited)
    ops.append({"op": "replace-resource", "when": {"hide_f2_icon_tabs": True}, "erp": PERSISTENT,
                "resource": SCREEN_GROUPS, "fragment": 0, "source": "erp/frontend/screen_groups.uicontentgroup"})
    (ROOT / "erp/frontend.json").write_text(json.dumps({"schema_version": 1, "ops": ops}, indent=1) + "\n")
    return labels, results, binds


def field(obj, key):
    return next(v for k, v in obj.items if k.value == key)


def hide_tabs(pkg):
    removed = 0
    for group in field(pkg.root, "ContentGroups").items:
        if field(group, "Name").value == "driver_career_driver_select_header":
            screens = field(group, "ContentScreens")
            keep = [s for s in screens.items if field(s, "TabName").value not in HIDDEN_TABS]
            removed += len(screens.items) - len(keep)
            screens.items = keep
    return removed


def check_tabs(raw, edited):
    a, b = Pkg(raw), Pkg(edited)
    hide_tabs(a)
    from pkg import text
    assert text(a.root) == text(b.root)
    assert Pkg(edited).to_bytes() == edited


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", required=True, type=Path)
    ap.add_argument("--preview", type=Path)
    a = ap.parse_args(argv[1:])
    labels, results, binds = build(a.game, a.preview)
    for label, (tex, name, spare) in sorted(labels.items()):
        print(f"{label:22} {' '.join(name):28} {tex}" + (f" (context from {spare})" if spare else ""))
    print(f"{len(results)} textures, {len(binds)} contexts, erp/frontend.json written")


if __name__ == "__main__":
    main(sys.argv)
