#!/usr/bin/env python3
"""Ship the real driver heads of data/heads.json: copy each head's files out of its mod zip into heads/ (not committed)
and write assets/heads.json, the asset ops that put them in the game folder.

The stock database has an entrants.animation.driver_heads.driver_head row for each of these heads, and its
asset_group_path_format and face_morph_asset_group_path_format pick the groups
animation_package/scenes/cinematics/drivers/heads/<label> and .../morph_poses/<label> of the row's label. The stock
manifest lists those groups, so the core never registers an addon's group under them, and a player's game folder may
hold other files at their archive paths. Each head therefore ships under its own label cs_<head> (label()): gen.py adds
a cs_<head> row copied from the stock <head> row, whose model_path,
eaid://[CHARACTER_PACKAGE]/heads/male_driver/<head>/idf/<head>_rig_head_morph_v1, the heads archive holds. The two
archives go to heads/cs_<head>.erp and morph_poses/cs_<head>.erp, and both groups are registered with group.register,
with lando_norris's groups as templates. A head whose textures stream also has .mipmaps files under
character_package/heads/male_driver/<head>/textures/, put at their own paths.

A head with `repair_from` in data/heads.json takes, for each of its materials that names a .sampler resource or a
heads/male_driver/ resource its archive lacks, the same-named material of the donor head, with the donor's texture
folder code (male_driver/serps_<code>/textures, the same length in every More Driver Models head) renamed to the head's.

Every material in F1 25's layout gets resource version 2. F1 25 parses a material of any other version as F1 23's
layout, and More Driver Models labels its F1 25 eye, head and head_cutscene materials version 1, as the repair leaves
the materials it copies: the parse then copies 256 KB from a 6 KB head.material and crashes the session load (memcpy at
F1_25.exe+0x53784bb, called from the material parser 0x143c0dc80). The relabel changes the version and no fragment: the
game decompresses every fragment of compression code 3 to 0x3f as zstd, More Driver Models writes code 0x10, and
erpw.set_frag stores a 0x10 fragment uncompressed, on which the game's decompressor spins and the session load never ends.

The tool fails when --game is not the F1 25 folder or a target exists in it, and when a head's archives name a
.sampler resource (an older format F1 25 1.0.141 has none of) or a resource under heads/male_driver/ they do not hold:
the team picker's head preview crashes in Ne_LinkResolver_ResolveLinks on such a link. A head must be the head of a
driver in data/seasons.json (head_overrides or new_drivers).
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

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT.parents[2] / "tools"))

import erpw
import gfx_material

EAID = re.compile(rb"eaid://[\x21-\x7e]+")
CODE = re.compile(rb"/heads/male_driver/(serps_[A-Za-z0-9]+)/textures/")
OUT = ROOT / "heads"
OPS = ROOT / "assets" / "heads.json"
SCENES = "animation_package/scenes/cinematics/drivers"
TEMPLATE = "lando_norris"
VERSION_25 = struct.pack("<I", 2)
ZSTD_CODES = range(3, 0x40)


def label(head):
    return f"cs_{head}"


def archives(head):
    return [f"2025_asset_groups/{SCENES}/{kind}/{head}.erp" for kind in ("heads", "morph_poses")]


def target(rel, head):
    return rel if rel not in archives(head) else rel.replace(f"/{head}.erp", f"/{label(head)}.erp")


def wanted(rel, head):
    if rel in archives(head):
        return True
    parts = rel.split("/")
    return len(parts) > 4 and parts[:2] == ["character_package", "heads"] and parts[3] == head


def used_heads():
    seasons = json.loads((ROOT / "data" / "seasons.json").read_text())
    return set(seasons["head_overrides"].values()) | {d["head"] for d in seasons["new_drivers"].values()}


def extract(downloads, source, head):
    path = Path(downloads) / source["zip"]
    if not path.exists():
        sys.exit(f"{head}: {path} is missing")
    root = source["root"].rstrip("/") + "/"
    files = []
    with zipfile.ZipFile(path) as z:
        for info in z.infolist():
            if info.is_dir() or not info.filename.startswith(root):
                continue
            rel = info.filename[len(root):]
            if not wanted(rel, head):
                continue
            target = OUT / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            with z.open(info) as src, open(target, "wb") as dst:
                shutil.copyfileobj(src, dst)
            files.append(rel)
    missing = [a for a in archives(head) if a not in files]
    if missing:
        sys.exit(f"{head}: {path.name} has no {', '.join(missing)} under {root}")
    return sorted(files)


def frag(r, i):
    f = r.frags[i]
    return erpw.zstd_d(f.raw) if f.comp in ZSTD_CODES else f.raw


def f1_25_layout(b):
    try:
        gfx_material.parse25(b, strict=False)
    except (ValueError, IndexError, struct.error):
        return False
    return True


def bad_refs(e, r, held):
    out = set()
    for i in range(len(r.frags)):
        for m in EAID.finditer(e.frag_data(r, i)):
            ref = m.group(0).decode()
            if ref.endswith(".sampler") or ("/heads/male_driver/" in ref and ref.split("?")[0] not in held):
                out.add(ref)
    return out


def broken_links(path):
    e = erpw.Erp(str(path))
    held = {r.name.split("?")[0] for r in e.res}
    return sorted(set().union(*(bad_refs(e, r, held) for r in e.res)))


def texture_code(e):
    codes = {m.group(1) for r in e.res for m in [CODE.search(r.name.encode())] if m}
    if len(codes) != 1:
        sys.exit(f"{e.path}: texture folder codes {sorted(codes)}")
    return codes.pop()


def repair(head, donor):
    path = OUT / archives(head)[0]
    e, d = erpw.Erp(str(path)), erpw.Erp(str(OUT / archives(donor)[0]))
    mine, theirs = texture_code(e), texture_code(d)
    if len(mine) != len(theirs):
        sys.exit(f"{head}: texture folder {mine.decode()} and {donor}'s {theirs.decode()} differ in length")
    held = {r.name.split("?")[0] for r in e.res}
    materials = {r.name.rsplit("/", 1)[-1]: r for r in d.res if r.name.endswith(".material")}
    swapped = []
    for r in e.res:
        if not r.name.endswith(".material") or not bad_refs(e, r, held):
            continue
        src = materials.get(r.name.rsplit("/", 1)[-1])
        if src is None:
            sys.exit(f"{head}: {donor} has no {r.name.rsplit('/', 1)[-1]}")
        data = [d.frag_data(src, i).replace(theirs, mine) for i in range(len(src.frags))]
        r.frags = r.frags[:len(data)]
        for i, b in enumerate(data):
            e.set_frag(r, i, b)
        swapped.append(r.name.rsplit("/", 1)[-1])
    e.to_file(str(path))
    return swapped


def relabel(path):
    e = erpw.Erp(str(path))
    wrong = [r for r in e.res if r.type.rstrip(b"\0") == b"GfxMaterialRes" and r.unk[:4] != VERSION_25
             and f1_25_layout(frag(r, 0))]
    for r in wrong:
        r.unk = VERSION_25 + r.unk[4:]
    if wrong:
        e.to_file(str(path))
    return [r.name.rsplit("/", 1)[-1] for r in wrong]


def asset_ops(heads):
    ops = []
    for head, files in heads.items():
        ops += [{"op": "file.put", "source": f"heads/{rel}", "target": target(rel, head)} for rel in files]
        for kind in ("heads", "morph_poses"):
            ops.append({"op": "group.register", "group": f"asset_groups/{SCENES}/{kind}/{label(head)}.assetgroup",
                        "erp": f"2025_asset_groups/{SCENES}/{kind}/{label(head)}.erp",
                        "template": f"asset_groups/{SCENES}/{kind}/{TEMPLATE}.assetgroup"})
    return {"schema_version": 1, "ops": ops}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--downloads", default=os.path.expanduser("~/Downloads"))
    ap.add_argument("--game", default="/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
    args = ap.parse_args()
    data = json.loads((ROOT / "data" / "heads.json").read_text())
    unused = sorted(set(data["heads"]) - used_heads())
    if unused:
        sys.exit(f"no driver in data/seasons.json has the head {', '.join(unused)}")
    if OUT.exists():
        shutil.rmtree(OUT)
    heads = {head: extract(args.downloads, data["sources"][key], head) for head, key in data["heads"].items()}
    for head, donor in data.get("repair_from", {}).items():
        print(f"{head}: materials from {donor}: {', '.join(repair(head, donor))}")
    for head in heads:
        relabelled = [name for rel in archives(head) for name in relabel(OUT / rel)]
        if relabelled:
            print(f"{head}: resource version 2 for {', '.join(relabelled)}")
        bad = [ref for rel in archives(head) for ref in broken_links(OUT / rel)]
        if bad:
            sys.exit(f"{head}: its archives name {len(bad)} resource(s) F1 25 does not have: {', '.join(bad[:3])}")
    game = Path(args.game)
    if not (game / "F1_25.exe").is_file():
        sys.exit(f"--game {game} is not the F1 25 folder")
    stock = [target(rel, head) for head, files in heads.items() for rel in files if (game / target(rel, head)).exists()]
    if stock:
        sys.exit(f"these replace game files: {', '.join(stock)}")
    OPS.parent.mkdir(parents=True, exist_ok=True)
    OPS.write_text(json.dumps(asset_ops(heads), indent=1) + "\n")
    size = sum((OUT / rel).stat().st_size for files in heads.values() for rel in files)
    print(f"heads: {len(heads)} heads, {sum(map(len, heads.values()))} files, {size / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
