"""F1 23's own 2022 and 2023 kit (data/real_suits.json suits with "mod": "f1_23") on F1 25 archives, for
build_real_suits.py.

An entry names its F1 23 source (a team suit <year>-<team> or a driver kit <year>-<team>_<driver>) and its F1 25 base:
the 2024 (else 2025) suit of the same team, for a driver the same driver's, else a teammate's whose helmet shell is the
F1 23 one. The shells' meshes are byte for byte the same in both games, so an F1 23 helmet texture fits the F1 25
shell of its name; an entry without "helmet" has no base with its shell and keeps its look's stock helmet.

The base is relabelled (build_real_suits.relabel), so every material, bind and surface record is F1 25's. A texture of
a kit part (KIT) whose F1 23 counterpart exists takes F1 23's pixels: its whole mip chain, the archive levels and the
`.mipmaps` sidecar levels, all embedded in the archive with no sidecar, in the F1 25 surface header with F1 23's format,
size and level count. F1 23 packs the largest one or two sidecar levels of its bigger textures with compression 7
(a "BD" container no tool here reads); the chain then starts at the largest LZ4 or stored level, so those textures
are half or a quarter of F1 23's full size. A texture whose F1 23 aspect differs from the F1 25 one is resized to the
F1 25 size when the F1 25 format has an encoder. A team texture a driver archive also holds is the team suit's copy,
so the team suit and its drivers hold one resource per name.

Logo meshes (`--logos`): f1-23 takes F1 23's body and glove logo meshes (meshes, buffers, model and render records),
renamed byte for byte from <year>- to cs<yy>-, with the base's binds; base keeps the base's. A driver on a teammate's
base gets his own logo meshes under cs<yy>-<team>_<driver>/idf/<driver>_body_logos, bound by a copy of the base's
logo binds, which the suit and gloves rows name (meta "logos_model", "glove_logos_model").
"""

import re
import struct
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

import bcn
import erpw
import gamedb
from build_suits import put_texture
from carperf import kind

TEAMS = "character_package/team_livery"
DRIVERS = "character_package/drivers/male"
KIT = {"livery_v2", "boots", "glove", "glove_logo", "logo_01", "suit_attachments", "harness_v2", "hans_device_v2", "cap",
       "helmet", "helmet_wing", "helmet_attachments", "visor", "visor_attachments", "balaclava_up"}
PART = re.compile(r"^(.+)_[a-z]+\.tif\.image$")
EAID = re.compile(rb"eaid://[\x21-\x7e]+")
DB = "game_package/gamedatabases/gamedatabases.2025.bin"
COLOURS = [f".colour_{n}_default" for n in range(1, 5)]
READABLE = (0, bcn.LZ4)


def surfaces(e):
    return {r.name: r for r in e.res if kind(r) == "GfxSurfaceRes"}


def chain(e, r, root):
    fmt, w, h, n = bcn.Surface.header(e.frag_data(r, 0))
    emb = e.frag_data(r, 1)
    sizes = bcn.level_sizes(fmt, w, h, n)
    if sum(sizes) > len(emb):
        raise SystemExit(f"{r.name}: {len(emb)} archive bytes for {n} levels of {w}x{h}")
    levels, at = [], 0
    for s in sizes:
        levels.append(emb[at:at + s])
        at += s
    side = []
    f2 = e.frag_data(r, 2) if len(r.frags) > 2 else b""
    if f2 and f2[0]:
        path, entries = bcn.sidecar(f2)
        if entries:
            full = Path(root) / path
            if not full.exists():
                raise SystemExit(f"{r.name}: F1 23 has no {path}")
            k = len(entries)
            want = bcn.level_sizes(fmt, w << k, h << k, k)
            for lv, size in zip(entries, want):
                if lv[1] not in READABLE:
                    side = []
                    continue
                data = bcn.read_level(full, lv)
                if len(data) != size:
                    raise SystemExit(f"{path}: a level of {len(data)} bytes, not {size}")
                side.append(data)
    return fmt, w << len(side), h << len(side), side + levels


def full_size(e, r):
    fmt, w, h, _ = bcn.Surface.header(e.frag_data(r, 0))
    f2 = e.frag_data(r, 2) if len(r.frags) > 2 else b""
    k = len(bcn.sidecar(f2)[1]) if f2 and f2[0] else 0
    return fmt, w << k, h << k


def put_chain(e, r, fmt, w, h, levels):
    head = bytearray(e.frag_data(r, 0))
    struct.pack_into("<I", head, 8, fmt)
    struct.pack_into("<2I", head, 12, w, h)
    struct.pack_into("<I", head, 24, len(levels))
    put_texture(e, r.name, bytes(head), b"".join(levels), len(levels), fmt)


def swap(e, r, e23, r23, root23, report):
    fmt, w, h, levels = chain(e23, r23, root23)
    fmt25, w25, h25 = full_size(e, r)
    if (w, h) != (w25, h25) and w * h25 != h * w25 and fmt25 in bcn.BC1 + bcn.BC3 and fmt in bcn.BC1 + bcn.BC3 + bcn.BC7:
        img = bcn.decode(fmt, levels[0], w, h)
        img = np.asarray(Image.fromarray(img).resize((w25, h25), Image.LANCZOS))
        head, data, n = bcn.surface_frags(e.frag_data(r, 0), fmt25, img)
        put_texture(e, r.name, head, data, n, fmt25)
        report.append(f"{r.name.rsplit('/', 1)[-1]}: F1 23 {w}x{h} resized to {w25}x{h25}")
        return
    put_chain(e, r, fmt, w, h, levels)


def prefixes(entry, label, stem):
    src = entry["source"]
    if "driver" not in entry:
        return [(f"eaid://{TEAMS}/{label}/textures/{label}_", f"eaid://{TEAMS}/{src}/textures/{src}_")]
    team23 = src[:len(src) - len(entry["driver"]) - 1]
    team = label[:len(label) - len(stem) - 1]
    return [(f"eaid://{DRIVERS}/{label}/textures/{stem}_", f"eaid://{DRIVERS}/{src}/textures/{entry['driver']}_"),
            (f"eaid://{TEAMS}/{team}/textures/{team}_", f"eaid://{TEAMS}/{team23}/textures/{team23}_")]


def source_of(name, pairs):
    for ours, theirs in pairs:
        if name.startswith(ours):
            m = PART.match(name[len(ours):])
            if m and m.group(1) in KIT:
                return theirs + name[len(ours):]
    return None


def swap_textures(e, entry, label, stem, sources, root23, report):
    pairs = prefixes(entry, label, stem)
    found = [(a, surfaces(a)) for a in sources]
    swapped = 0
    for name, r in surfaces(e).items():
        want = source_of(name, pairs)
        if want is None:
            continue
        hit = next(((a, s[want]) for a, s in found if want in s), None)
        if hit is None:
            report.append(f"{label}: F1 23 has no {want.rsplit('/', 1)[-1]}, keeps F1 25's")
            continue
        swap(e, r, hit[0], hit[1], root23, report)
        swapped += 1
    return swapped


def logo_groups(e, prefix):
    out = {}
    for r in e.res:
        if r.name.startswith(prefix) and ".idf?" in r.name:
            out.setdefault(r.name.split(".idf?")[0], []).append(r)
    return out


def take_logos(e, e23, entry, label, stem):
    src = entry["source"]
    yy = label[:4]
    if "driver" not in entry:
        old, new = src, label
        theirs = [f"eaid://{TEAMS}/{src}/idf/{src}_body_logos", f"eaid://{TEAMS}/{src}/idf/{src}_glove_logos"]
    else:
        drv = entry["driver"]
        old, new = src, yy + src[4:]
        theirs = [f"eaid://{DRIVERS}/{src}/idf/{drv}_body_logos", f"eaid://{DRIVERS}/{src}/idf/{drv}_glove_logos"]
    if len(old) != len(new):
        raise SystemExit(f"{label}: {old} and {new} differ in length")
    ob, nb = old.encode(), new.encode()
    names = {r.name for r in e.res}
    taken = {}
    for prefix in theirs:
        for group, rs in logo_groups(e23, prefix).items():
            ours = group.replace(old, new)
            if "driver" not in entry and f"{ours}.idf?model" not in names:
                continue
            copies = []
            for r in rs:
                c = erpw.Res()
                c.name, c.type, c.unk, c.hash = r.name.replace(old, new), r.type, r.unk, r.hash
                c.frags = []
                for fr in r.frags:
                    f = erpw.Frag()
                    f.name, f.off, f.size, f.flags, f.comp, f.packed, f.raw = fr.name, fr.off, fr.size, fr.flags, fr.comp, fr.packed, fr.raw
                    c.frags.append(f)
                for i in range(len(c.frags)):
                    d = e23.frag_data(r, i)
                    if ob in d:
                        e.set_frag(c, i, d.replace(ob, nb))
                copies.append(c)
            taken[ours] = copies
    if "driver" in entry:
        part = {"body_logos": f"eaid://{DRIVERS}/{label}/idf/{stem}_body_logos",
                "glove_logos": f"eaid://{DRIVERS}/{label}/idf/{stem}_glove_logos"}
        meta = {}
        for key, base_group in part.items():
            ours = next(g for g in taken if g.endswith(f"_{key}"))
            binds = [r for r in e.res if r.name.startswith(base_group + ".emb?")]
            if not binds:
                raise SystemExit(f"{label}: no binds for {base_group}")
            if ours != base_group:
                e.res = [r for r in e.res if not r.name.startswith(base_group + ".")]
                for b in binds:
                    b.name = ours + b.name[len(base_group):]
                    e.res.append(b)
            meta["logos_model" if key == "body_logos" else "glove_logos_model"] = ours[len("eaid://character_package/"):]
    else:
        meta = {}
    for group, copies in taken.items():
        e.res = [r for r in e.res if not (r.name.startswith(group + ".idf?"))]
        e.res += copies
    return meta, sum(len(c) for c in taken.values())


def unresolved(e, stock):
    names = {r.name.split("?")[0] for r in e.res} | {r.name.split("?")[0] for r in stock.res}
    for r in stock.res:
        for i in range(len(r.frags)):
            if kind(r) == "GfxSurfaceRes" and i == 2:
                continue
            names.update(m.group(0).decode().split("?")[0] for m in EAID.finditer(stock.frag_data(r, i)))
    missing = set()
    for r in e.res:
        for i in range(len(r.frags)):
            if kind(r) == "GfxSurfaceRes" and i == 2:
                continue
            for m in EAID.finditer(e.frag_data(r, i)):
                ref = m.group(0).decode().split("?")[0]
                if ref not in names and not ref.startswith("eaid://shader_package/"):
                    missing.add(ref)
    return sorted(missing)


class Db:
    def __init__(self, game):
        self.recs = {r.key: r for r in gamedb.read((Path(game) / DB).read_bytes())}
        self.by_value = {}
        for r in self.recs.values():
            self.by_value.setdefault(r.value, []).append(r.key)

    def helmet_colours(self, base):
        for key in self.by_value.get(base.encode(), []):
            state = gamedb.fnv1a64_unwind(key, "._metadata_")
            if gamedb.fnv1a64(".attachments_model", state) in self.recs:
                vals = [self.recs.get(gamedb.fnv1a64(f, state)) for f in COLOURS]
                return [(v.value.decode() if v and v.value else "") or "0" for v in vals]
        raise SystemExit(f"{base}: no helmet row")


def f1_23_archives(f1_23, entry):
    root = Path(f1_23) / "2023_asset_groups"
    src = entry["source"]
    if "driver" not in entry:
        team = erpw.Erp(str(root / TEAMS / src / f"{src}.erp"))
        return {"suit": [team], "helmet": []}
    team23 = src[:len(src) - len(entry["driver"]) - 1]
    folder = root / DRIVERS / src
    suit = erpw.Erp(str(folder / f"{entry['driver']}.erp"))
    team = erpw.Erp(str(root / TEAMS / team23 / f"{team23}.erp"))
    helmet = erpw.Erp(str(folder / f"{entry['driver']}_helmet.erp"))
    return {"suit": [suit, team], "helmet": [helmet, suit, team]}


def f1_23_texture(sources, name, root):
    for e in sources:
        hit = surfaces(e).get(name)
        if hit is not None:
            fmt, w, h, levels = chain(e, hit, root)
            return bcn.decode(fmt, levels[0], w, h)
    return None


def tile(img, size):
    out = Image.new("RGBA", (size, size), (70, 72, 80, 255))
    if img is not None:
        im = Image.fromarray(img)
        im.thumbnail((size, size), Image.LANCZOS)
        out.alpha_composite(im, ((size - im.width) // 2, (size - im.height) // 2))
    return out.convert("RGB")


def sheet(title, rows, out, small):
    from build_suits import font, shrink
    vw, vh, t = 440, 640, 300
    parts = ("livery_v2_d", "logo_01_d", "glove_d", "helmet_d")
    width = 2 * vw + 2 * len(parts) * t + 80
    img = Image.new("RGB", (width, 70 + len(rows) * (vh + 60)), (30, 31, 35))
    d = ImageDraw.Draw(img)
    d.text((20, 18), f"{title} (front, back, then F1 23 source and built: " + ", ".join(parts) + ")",
           fill=(235, 235, 235), font=font(26))
    for n, (caption, views, pairs) in enumerate(rows):
        y = 70 + n * (vh + 60)
        d.text((20, y + 4), caption, fill=(220, 220, 220), font=font(20))
        y += 40
        img.paste(Image.fromarray(views[0]), (20, y))
        img.paste(Image.fromarray(views[1]), (20 + vw, y))
        x = 40 + 2 * vw
        for k, part in enumerate(parts):
            src, built = pairs.get(part, (None, None))
            for j, im in enumerate((src, built)):
                d.text((x, y), ("F1 23 " if j == 0 else "built ") + part, fill=(170, 200, 255), font=font(14))
                img.paste(tile(im, t), (x, y + 20))
                x += t + (8 if j == 0 else 24)
    img.save(out, optimize=True)
    shrink(img).save(small, optimize=True)
