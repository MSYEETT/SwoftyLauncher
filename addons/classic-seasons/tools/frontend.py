#!/usr/bin/env python3
"""Writes erp/frontend.json and erp/frontend/: Team Select and Driver Select backdrop names for the Classic Seasons
team keys (cs<yy>_<folder>) and for the season drivers that have no stock backdrop, as new textures, materials and
material contexts in main_front_end.erp. Team keys whose name the game already has point at the stock material.
No stock resource changes.

The names are drawn in Saira (SIL Open Font License), downloaded once from google/fonts at a pinned commit.

usage: frontend.py --game <F1 25 folder> [--baseline <launcher baseline>/<build>] [--font <ttf>]
"""
import argparse
import hashlib
import json
import os
import struct
import subprocess
import sys
import unicodedata
import urllib.request
from functools import lru_cache
from pathlib import Path

import lz4.block
import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
REPO = ROOT.parents[1]
sys.path.insert(0, str(REPO / "tools"))

from erp import parse as erp_parse  # noqa: E402

DATA = ROOT / "data/seasons.json"
OUT = ROOT / "erp/frontend"
MFE = "2025_asset_groups/environment_package/second_world/main_front_end/wep/main_front_end.erp"
IDF = "eaid://environment_package/core_assets/objects/idf/main_front_end/"
MATERIALS = "eaid://environment_package/core_assets/objects/materials/main_front_end/names/"
TEX = "eaid://environment_package/core_assets/objects/textures/main_front_end/names/"
NAMES_DIR = "environment_package/core_assets/objects/textures/main_front_end/names/"

FONT_URL = ("https://raw.githubusercontent.com/google/fonts/3cfea32a77637ada0e209f10d9f91e7ff9d2a32f/"
            "ofl/saira/Saira%5Bwdth,wght%5D.ttf")
FONT_SHA256 = "9d050fc5a01c85f74c4257c207d50b55d1e40c37308c642f974a2c5003231dde"
WEIGHT, WIDTH = 800, 125
STRETCH = 1.38
TRACK = 0.09
SPACE = 0.55
RING = 5
SUPERSAMPLE = 2

KINDS = {
    "driver": {"bind": IDF + "mfe_2025_main_menu_driver_select_name.emb?context=", "template_context": "zenith_driverfirst",
               "folder": "f1_driver_names", "prefix": "f1_drivername_", "template": "sonny_hayes",
               "material": "mfe_2025_driver_select_{}_a.material", "side": ("f1_driver_names~", "cs_driver_names~"),
               "id": "cs_driver{:02d}"},
    "team": {"bind": IDF + "mfe_2025_main_menu_vehicle_select_name.emb?context=", "template_context": "mercedes_26",
             "folder": "f1_team_names", "prefix": "f1_teamname_", "template": "mercedes",
             "material": "mfe_2025_team_select_{}_a.material", "side": ("f1_team_names~", "cs_team_names~"),
             "id": "cs_tm_{:02d}"},
}
STOCK_TEAM_NAMES = {
    "ALPINE": "alpine", "ASTON MARTIN": "amaf1", "FERRARI": "ferrari", "HAAS": "haas", "McLAREN": "mclaren",
    "MERCEDES": "mercedes", "WILLIAMS": "williams",
}
DRIVER_NAMES = {"guanyu_zhou": ("ZHOU", "GUANYU")}
BC7_WEIGHTS = np.array([0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64], dtype=np.int64)


class Stock:
    def __init__(self, game, baseline=None):
        self.game = Path(game)
        self.files, self.objects = {}, None
        if baseline:
            self.files = json.loads((Path(baseline) / "baseline.json").read_text())["files"]
            self.objects = Path(baseline).parent / "objects"

    @lru_cache(None)
    def path(self, rel):
        p = self.game / rel
        entry = self.files.get(rel)
        if entry and hashlib.sha256(p.read_bytes()).hexdigest() != entry["sha256"]:
            obj = self.objects / entry["sha256"][:2] / entry["sha256"]
            if not obj.exists():
                sys.exit(f"{rel}: changed in the game folder and no stock copy in the baseline")
            return obj
        return p


class Archive:
    def __init__(self, path):
        self.path = path
        self.meta, res = erp_parse(str(path))
        self.res = {r[0]: r for r in res}

    def fragments(self, name):
        out = []
        with open(self.path, "rb") as f:
            for _fn, off, _size, _flags, comp, packed in self.res[name][3]:
                f.seek(self.meta["data_off"] + off)
                raw = f.read(packed)
                if comp == 0x11:
                    raw = subprocess.run(["zstd", "-dc"], input=raw, capture_output=True, check=True).stdout
                out.append(raw)
        return out


def font_path(override=None):
    if override:
        return Path(override)
    cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "swofty-addon-tools/Saira[wdth,wght].ttf"
    if not cache.exists() or hashlib.sha256(cache.read_bytes()).hexdigest() != FONT_SHA256:
        data = urllib.request.urlopen(FONT_URL, timeout=60).read()
        if hashlib.sha256(data).hexdigest() != FONT_SHA256:
            sys.exit(f"{FONT_URL}: unexpected sha256")
        cache.parent.mkdir(parents=True, exist_ok=True)
        cache.write_bytes(data)
    return cache


@lru_cache(None)
def font(path):
    f = ImageFont.truetype(str(path), 400)
    f.set_variation_by_axes([WEIGHT, WIDTH])
    return f


@lru_cache(None)
def glyph(path, c):
    f = font(path)
    size = round(f.size * 2)
    im = Image.new("L", (size, size), 0)
    ImageDraw.Draw(im).text((size // 4, round(size * 0.75)), c, font=f, fill=255, anchor="ls")
    l, _, r, _ = im.getbbox()
    return im.crop((l, 0, r, size))


def text_fill(path, text):
    cap = glyph(path, "H").getbbox()
    cap_px = cap[3] - cap[1]
    parts, x = [], 0
    for c in text:
        if c == " ":
            x += SPACE * cap_px
            continue
        g = glyph(path, c)
        parts.append((round(x), g))
        x += g.width + TRACK * cap_px
    im = Image.new("L", (round(x) + 1, parts[0][1].height), 0)
    for px, g in parts:
        im.paste(g, (px, 0), g)
    im = im.crop(im.getbbox())
    return im.resize((round(im.width * STRETCH), im.height), Image.LANCZOS)


def place(fill, scale, outline):
    w, h = max(1, round(fill.width * scale)), max(1, round(fill.height * scale))
    m = fill.resize((w, h), Image.LANCZOS)
    if not outline:
        return m
    pad = RING
    padded = Image.new("L", (w + 2 * pad, h + 2 * pad), 0)
    padded.paste(m, (pad, pad))
    ring = ImageChops.subtract(padded.filter(ImageFilter.MaxFilter(RING)), padded.filter(ImageFilter.MinFilter(RING)))
    return ring.crop((pad - RING // 2, pad - RING // 2, pad + w + RING // 2, pad + h + RING // 2))


def team_image(path, w, h, name):
    s = SUPERSAMPLE
    W, H = w * s, h * s
    fill = text_fill(path, name)
    scale = min(0.965 * W / fill.width, 0.84 * H / fill.height)
    m = place(fill, scale, True)
    alpha = Image.new("L", (W, H), 0)
    alpha.paste(m, ((W - m.width) // 2, (H - m.height) // 2), m)
    return alpha.resize((w, h), Image.LANCZOS)


def driver_image(path, w, h, first, last):
    s = SUPERSAMPLE
    W, H = w * s, h * s
    fills = [text_fill(path, first), text_fill(path, last)]
    widest = max(f.width for f in fills)
    scale = min(0.90 * W / widest, 0.33 * H / fills[0].height)
    solid = 0 if fills[0].width < fills[1].width * 0.97 else 1
    right = round(0.951 * W)
    alpha = Image.new("L", (W, H), 0)
    for i, f in enumerate(fills):
        m = place(f, scale, i != solid)
        y = round(0.476 * H) - m.height if i == 0 else round(0.521 * H)
        alpha.paste(m, (right - m.width, y), m)
    return alpha.resize((w, h), Image.LANCZOS)


def bc7_alpha(alpha):
    h, w = alpha.shape
    blocks = alpha.reshape(h // 4, 4, w // 4, 4).transpose(0, 2, 1, 3).reshape(-1, 16).astype(np.int64)
    lo, hi = blocks.min(1), blocks.max(1)
    p0, p1 = lo & 1, hi & 1
    q0, q1 = lo >> 1, hi >> 1
    pal = ((64 - BC7_WEIGHTS) * lo[:, None] + BC7_WEIGHTS * hi[:, None] + 32) >> 6
    idx = np.abs(blocks[:, :, None] - pal[:, None, :]).argmin(2)
    flip = idx[:, 0] >= 8
    idx[flip] = 15 - idx[flip]
    q0, q1 = np.where(flip, q1, q0), np.where(flip, q0, q1)
    p0, p1 = np.where(flip, p1, p0), np.where(flip, p0, p1)
    c0, c1 = 127, 127
    u = np.uint64
    lo_bits = np.full(len(blocks), 1 << 6, dtype=u)
    for shift, v in ((7, c0), (14, c1), (21, c0), (28, c1), (35, c0), (42, c1)):
        lo_bits |= u(v << shift)
    lo_bits |= q0.astype(u) << u(49)
    lo_bits |= q1.astype(u) << u(56)
    lo_bits |= p0.astype(u) << u(63)
    hi_bits = p1.astype(u)
    hi_bits |= idx[:, 0].astype(u) << u(1)
    for i in range(1, 16):
        hi_bits |= idx[:, i].astype(u) << u(4 + 4 * (i - 1))
    return np.stack([lo_bits, hi_bits], 1).astype("<u8").tobytes()


def same_length(data, old, new):
    old, new = old.encode(), new.encode()
    if len(old) != len(new) or old not in data:
        sys.exit(f"cannot swap {old!r} for {new!r}")
    return data.replace(old, new)


def encode(path, kind, new_id, args, hdr, f1_len, f2):
    k = KINDS[kind]
    _, _, _fmt, w0, h0, _, erp_levels = struct.unpack_from("<7I", hdr)
    at = 1 + f2[0] + 1
    side_n = struct.unpack_from("<I", f2, at)[0]
    w, h = w0 << side_n, h0 << side_n
    alpha = (driver_image if kind == "driver" else team_image)(path, w, h, *args)
    levels = []
    for i in range(side_n + erp_levels):
        lw, lh = w >> i, h >> i
        a = alpha.resize((lw, lh), Image.BOX) if i else alpha
        levels.append(bc7_alpha(np.asarray(a)))
    f1 = b"".join(levels[side_n:])
    assert len(f1) == f1_len
    old_side = f"{k['side'][0]}{k['prefix']}{k['template']}"
    new_side = f"{k['side'][1]}{k['prefix']}{new_id}"
    f2 = bytearray(same_length(f2, old_side, new_side))
    side, off = b"", 0
    for i, lvl in enumerate(levels[:side_n]):
        packed = lz4.block.compress(lvl, store_size=False, mode="high_compression")
        f2[at + 4 + 26 * i:at + 4 + 26 * (i + 1)] = struct.pack("<BBQQQ", 0, 3, off, len(packed), len(lvl))
        side += packed
        off += len(packed)
    side_path = bytes(f2[1:1 + f2[0]]).rstrip(b"\0").decode()
    return f1, bytes(f2), side_path, side


def plain(text):
    return "".join(c for c in unicodedata.normalize("NFKD", text) if not unicodedata.combining(c)).upper()


def team_name(name):
    return "McLAREN" if name == "McLaren" else plain(name)


def driver_name(label, new_drivers):
    if label in new_drivers:
        return plain(new_drivers[label]["first"]), plain(new_drivers[label]["last"])
    if label in DRIVER_NAMES:
        return DRIVER_NAMES[label]
    first, _, last = label.partition("_")
    return plain(first.replace("-", " ")), plain(last.replace("-", " ").replace("_", " "))


def rows(data):
    teams, drivers = [], []
    for season in data["seasons"]:
        for team in season["teams"]:
            teams.append((f"cs{season['year'] % 100:02d}_{team['folder']}", team_name(team["name"])))
            for label, _number in team["drivers"]:
                if label not in drivers:
                    drivers.append(label)
    return teams, drivers


def stock_bind(arc, material):
    pre = KINDS["team"]["bind"]
    target = (MATERIALS + material).encode()
    for name in sorted(arc.res):
        if name.startswith(pre) and arc.res[name][1] == "GfxMatBindRes":
            data = arc.fragments(name)[0]
            if target + b"\0" in data:
                return name, data
    sys.exit(f"no stock context points at {material}")


def build(stock, font_file):
    arc = Archive(stock.path(MFE))
    data = json.loads(DATA.read_text())
    teams, labels = rows(data)
    templates = {}
    for kind, k in KINDS.items():
        tex = f"{TEX}{k['folder']}/{k['prefix']}{k['template']}.tif"
        templates[kind] = {
            "image": arc.fragments(tex + ".image"), "srv": arc.fragments(tex)[0],
            "material": arc.fragments(MATERIALS + k["material"].format(k["template"]))[0],
            "bind": arc.fragments(k["bind"] + k["template_context"])[0],
        }

    new_textures, team_binds, report = [], [], {"team_stock": {}, "team_new": {}, "driver_stock": [], "driver_new": {}}
    team_ids = {}
    for key, name in teams:
        if name in STOCK_TEAM_NAMES:
            context, bind = stock_bind(arc, KINDS["team"]["material"].format(STOCK_TEAM_NAMES[name]))
            team_binds.append((key, bind))
            report["team_stock"].setdefault(name, {"context": context.split("=", 1)[1], "keys": []})["keys"].append(key)
            continue
        if name not in team_ids:
            team_ids[name] = KINDS["team"]["id"].format(len(team_ids) + 1)
            new_textures.append(("team", team_ids[name], (name,)))
        report["team_new"].setdefault(name, {"id": team_ids[name], "keys": []})["keys"].append(key)
        team_binds.append((key, None))

    driver_rows = []
    for label in labels:
        if KINDS["driver"]["bind"] + label in arc.res:
            report["driver_stock"].append(label)
            continue
        new_id = KINDS["driver"]["id"].format(len(driver_rows) + 1)
        names = driver_name(label, data["new_drivers"])
        driver_rows.append((label, new_id))
        new_textures.append(("driver", new_id, names))
        report["driver_new"][label] = {"id": new_id, "name": " ".join(names)}

    if OUT.exists():
        for p in sorted(OUT.rglob("*"), reverse=True):
            p.unlink() if p.is_file() else p.rmdir()
    (OUT / "mipmaps").mkdir(parents=True)
    for kind in KINDS:
        (OUT / f"{kind}.head").write_bytes(templates[kind]["image"][0])

    ops = []
    for kind, new_id, args in new_textures:
        k, t = KINDS[kind], templates[kind]
        hdr, f1, f2 = t["image"]
        f1, f2, side_path, side = encode(font_file, kind, new_id, args, hdr, len(f1), f2)
        old_tex, new_tex = f"{k['prefix']}{k['template']}", f"{k['prefix']}{new_id}"
        files = {
            "f1": f1, "f2": f2,
            "srv": same_length(t["srv"], old_tex, new_tex),
            "material": same_length(t["material"], old_tex, new_tex),
        }
        for ext, blob in files.items():
            (OUT / f"{new_id}.{ext}").write_bytes(blob)
        (OUT / "mipmaps" / side_path[len(NAMES_DIR):]).write_bytes(side)
        tex = f"{TEX}{k['folder']}/{new_tex}.tif"
        src = lambda ext: f"erp/frontend/{new_id}.{ext}"
        ops += [
            {"op": "add-resource", "erp": MFE, "resource": tex + ".image", "type": "GfxSurfaceRes",
             "sources": [f"erp/frontend/{kind}.head", src("f1"), src("f2")]},
            {"op": "add-resource", "erp": MFE, "resource": tex, "type": "GfxSRVResource", "sources": [src("srv")]},
            {"op": "add-resource", "erp": MFE, "resource": MATERIALS + k["material"].format(new_id),
             "type": "GfxMaterialRes", "sources": [src("material")]},
        ]

    def new_bind(kind, new_id):
        k = KINDS[kind]
        return same_length(templates[kind]["bind"], k["material"].format(k["template"]), k["material"].format(new_id))

    names_by_key = dict(teams)
    for key, bind in team_binds:
        if bind is None:
            bind = new_bind("team", team_ids[names_by_key[key]])
        (OUT / f"{key}.bind").write_bytes(bind)
        ops.append({"op": "add-resource", "erp": MFE, "resource": KINDS["team"]["bind"] + key, "type": "GfxMatBindRes",
                    "sources": [f"erp/frontend/{key}.bind"]})
    for label, new_id in driver_rows:
        (OUT / f"{label}.bind").write_bytes(new_bind("driver", new_id))
        ops.append({"op": "add-resource", "erp": MFE, "resource": KINDS["driver"]["bind"] + label,
                    "type": "GfxMatBindRes", "sources": [f"erp/frontend/{label}.bind"]})
    ops.append({"op": "add-mipmaps", "source": "erp/frontend/mipmaps/", "target": NAMES_DIR})
    (ROOT / "erp/frontend.json").write_text(json.dumps({"schema_version": 1, "ops": ops}, indent=1) + "\n")
    return report


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", required=True, type=Path)
    ap.add_argument("--baseline", type=Path)
    ap.add_argument("--font", type=Path)
    a = ap.parse_args(argv[1:])
    report = build(Stock(a.game, a.baseline), font_path(a.font))
    for name, r in report["team_stock"].items():
        print(f"team   {name:16} stock {r['context']:12} {len(r['keys'])} keys")
    for name, r in report["team_new"].items():
        print(f"team   {name:16} new   {r['id']:12} {len(r['keys'])} keys")
    print(f"driver stock {len(report['driver_stock'])}: {' '.join(report['driver_stock'])}")
    for label, r in report["driver_new"].items():
        print(f"driver {label:20} new   {r['id']:12} {r['name']}")


if __name__ == "__main__":
    main(sys.argv)
