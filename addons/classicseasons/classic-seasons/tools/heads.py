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
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT.parents[2] / "tools"))

import erpw

EAID = re.compile(rb"eaid://[\x21-\x7e]+")
OUT = ROOT / "heads"
OPS = ROOT / "assets" / "heads.json"
SCENES = "animation_package/scenes/cinematics/drivers"
TEMPLATE = "lando_norris"


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


def broken_links(path):
    e = erpw.Erp(str(path))
    held = {r.name.split("?")[0] for r in e.res}
    bad = set()
    for r in e.res:
        for i in range(len(r.frags)):
            for m in EAID.finditer(e.frag_data(r, i)):
                ref = m.group(0).decode()
                if ref.endswith(".sampler") or ("/heads/male_driver/" in ref and ref.split("?")[0] not in held):
                    bad.add(ref)
    return sorted(bad)


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
    for head in heads:
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
