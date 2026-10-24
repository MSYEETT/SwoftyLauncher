#!/usr/bin/env python3
"""Ship the real driver heads of data/heads.json: copy each head's files out of its mod zip into heads/ (not committed)
and write assets/heads.json, the asset ops that put them in the game folder.

The stock database already has an entrants.animation.driver_heads.driver_head row for each of these labels, and the
driver rows' head field names it. The row's asset_group_path_format and face_morph_asset_group_path_format pick the
asset groups animation_package/scenes/cinematics/drivers/heads/<head> and .../morph_poses/<head>, and model_path names
eaid://[CHARACTER_PACKAGE]/heads/male_driver/<head>/idf/<head>_rig_head_morph_v1, which the heads archive holds. The
stock game ships neither archive. A head whose textures stream also has .mipmaps files under
character_package/heads/male_driver/<head>/textures/.

Each head goes in as files the game does not have: the tool fails when a target exists in --game. Its two groups are
registered with group.register (the core skips a group the stock manifest already lists), with lando_norris's groups
as templates. A head label must be the head of a driver in data/seasons.json (head_overrides or new_drivers).
"""

import argparse
import json
import os
import shutil
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "heads"
OPS = ROOT / "assets" / "heads.json"
SCENES = "animation_package/scenes/cinematics/drivers"
TEMPLATE = "lando_norris"


def archives(head):
    return [f"2025_asset_groups/{SCENES}/{kind}/{head}.erp" for kind in ("heads", "morph_poses")]


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


def asset_ops(heads):
    ops = []
    for head, files in heads.items():
        ops += [{"op": "file.put", "source": f"heads/{rel}", "target": rel} for rel in files]
        for kind in ("heads", "morph_poses"):
            ops.append({"op": "group.register", "group": f"asset_groups/{SCENES}/{kind}/{head}.assetgroup",
                        "erp": f"2025_asset_groups/{SCENES}/{kind}/{head}.erp",
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
    game = Path(args.game)
    if game.is_dir():
        stock = [rel for files in heads.values() for rel in files if (game / rel).exists()]
        if stock:
            sys.exit(f"these replace game files: {', '.join(stock)}")
    OPS.parent.mkdir(parents=True, exist_ok=True)
    OPS.write_text(json.dumps(asset_ops(heads), indent=1) + "\n")
    size = sum((OUT / rel).stat().st_size for files in heads.values() for rel in files)
    print(f"heads: {len(heads)} heads, {sum(map(len, heads.values()))} files, {size / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
