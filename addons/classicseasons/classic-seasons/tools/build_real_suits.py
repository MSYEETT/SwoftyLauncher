#!/usr/bin/env python3
"""Real suits from mods (data/real_suits.json), relabelled for Classic Seasons, into this addon's suits/ (not
committed), with suits/meta.json for gen.py and a contact sheet in suits/preview.png and art/suits/real.png.

Each entry names a mod archive that replaces an F1 25 suit (its base: a team suit <year>-<team> or a driver suit
<year>-<team>_<driver>, with an optional helmet archive). The build keeps the archive's resources whose name holds the
base label, renamed byte for byte to the entry's label (suit_sources.mod_label), so nothing replaces a stock file; the
shared body, glove and template resources stay in the stock base archive, which the asset group lists first. A
texture's `.mipmaps` sidecar:
- that the mod ships, that holds the levels the texture names and that differs from the stock file is copied to suits/sidecars/ under the relabelled path, which
  the texture then names;
- that the mod does not ship keeps its stock path when the texture's sidecar levels are the stock ones;
- otherwise goes, and the texture keeps its embedded levels (the F1 2016 pack's blanked logo atlases, whose sidecars
  the pack does not ship).
A helmet archive takes the suit archive's copy of every resource both hold (the 2018 Red Bull helmet archives keep the
stock suit textures). The build fails when a relabelled archive names a resource that neither it nor its stock base archive holds or names,
or when two built archives hold different resources of one name.

    python3 tools/build_real_suits.py --game <F1 25> --mods <folder holding the extracted mods>
"""

import argparse
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path[:0] = [str(HERE), str(ROOT.parents[2] / "tools")]

import bcn
import erpw
import suit_sources
from build_suits import contact_sheet, kit_textures, render, team_meshes
from carperf import kind

OUT = ROOT / "suits"
SIDECARS = OUT / "sidecars"
EAID = re.compile(rb"eaid://[\x21-\x7e]+")
TEAMS = "character_package/team_livery"
DRIVERS = "character_package/drivers/male"


def digest(e, r):
    return hashlib.md5(b"".join(e.frag_data(r, i) for i in range(len(r.frags)))).hexdigest()


def readable(path, levels):
    try:
        return all(len(bcn.read_level(path, lv)) == lv[4] for lv in levels)
    except Exception:
        return False


def find_mod_file(files, path):
    hits = files.get(path.rsplit("/", 1)[-1], [])
    exact = [p for p in hits if p.as_posix().endswith("/" + path)]
    return (exact or hits or [None])[0]


def refs(e):
    found = set()
    for r in e.res:
        for i in range(len(r.frags)):
            if kind(r) == "GfxSurfaceRes" and i == 2:
                continue
            found.update(m.group(0).decode() for m in EAID.finditer(e.frag_data(r, i)))
    return found


def relabel(src, base_erp, old, new, mod_root, files, game, report):
    e = erpw.Erp(str(src))
    stock = erpw.Erp(str(base_erp))
    stock_by = {r.name: r for r in stock.res}
    ob, nb = old.encode(), new.encode()
    e.res = [r for r in e.res if ob in r.name.encode()]
    shipped = []
    for r in e.res:
        original = r.name
        r.name = r.name.replace(old, new)
        surf = kind(r) == "GfxSurfaceRes"
        for i in range(len(r.frags)):
            d = e.frag_data(r, i)
            if surf and i == 2:
                continue
            if ob in d:
                e.set_frag(r, i, d.replace(ob, nb))
        if not surf or len(r.frags) < 3:
            continue
        f2 = e.frag_data(r, 2)
        path, levels = bcn.sidecar(f2) if f2 and f2[0] else (None, [])
        if not levels:
            continue
        mod_file = find_mod_file(files, path)
        if mod_file and not readable(mod_file, levels):
            report.append(f"{src.name}: {mod_file.name} does not hold the levels its texture names")
            mod_file = None
        stock_file = Path(game) / path
        if mod_file and not (stock_file.exists() and stock_file.read_bytes() == mod_file.read_bytes()):
            moved = path.replace(old, new)
            if moved == path:
                raise SystemExit(f"{src}: {path}: no {old} to relabel")
            e.set_frag(r, 2, f2.replace(path.encode(), moved.encode()))
            target = SIDECARS / moved
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(mod_file, target)
            shipped.append(moved)
            continue
        base = stock_by.get(original)
        if stock_file.exists() and base is not None and len(base.frags) > 2 and stock.frag_data(base, 2) == f2:
            continue
        r.frags = r.frags[:2]
        e.set_frag(r, 0, e.frag_data(r, 0))
        report.append(f"{src.name}: {original.rsplit('/', 1)[-1]} keeps its embedded levels, no sidecar for {path}")
    left = [r.name for r in e.res for i in range(len(r.frags))
            if not (kind(r) == "GfxSurfaceRes" and i == 2) and ob in e.frag_data(r, i)]
    if left:
        raise SystemExit(f"{new}: {old} left in {left[:3]}")
    names = {r.name for r in e.res} | set(stock_by) | refs(stock)
    missing = set()
    for r in e.res:
        for i in range(len(r.frags)):
            if kind(r) == "GfxSurfaceRes" and i == 2:
                continue
            for m in EAID.finditer(e.frag_data(r, i)):
                ref = m.group(0).decode()
                if ref not in names and ref.split("?")[0] not in {n.split("?")[0] for n in names} \
                        and not ref.startswith("eaid://shader_package/"):
                    missing.add(ref)
    if missing:
        raise SystemExit(f"{new}: names {len(missing)} resource(s) neither archive holds: {sorted(missing)[:4]}")
    return e, shipped


def paths(entry):
    base, label = entry["base"], entry["label"]
    if "driver" not in entry:
        return {"target": f"2025_asset_groups/{TEAMS}/{label}/{label}.erp",
                "group": f"asset_groups/{TEAMS}/{label}/{label}.assetgroup",
                "base_erp": f"2025_asset_groups/{TEAMS}/{base}/{base}.erp",
                "template": f"asset_groups/{TEAMS}/{base}/{base}.assetgroup"}
    stem = Path(entry["erp"]).stem
    out = {"stem": stem}
    for key, name in (("", stem), ("helmet_", f"{stem}_helmet")):
        out[key + "target"] = f"2025_asset_groups/{DRIVERS}/{label}/{name}.erp"
        out[key + "group"] = f"asset_groups/{DRIVERS}/{label}/{name}.assetgroup"
        out[key + "base_erp"] = f"2025_asset_groups/{DRIVERS}/{base}/{name}.erp"
        out[key + "template"] = f"asset_groups/{DRIVERS}/{base}/{name}.assetgroup"
    return out


def personal_meshes(label, stem):
    return {"feet": f"eaid://{DRIVERS}/{label}/idf/{stem}.idf",
            "logos": f"eaid://{DRIVERS}/{label}/idf/{stem}_body_logos.idf",
            "glove_logos": f"eaid://{DRIVERS}/{label}/idf/{stem}_glove_logos.idf"}


def check_models(e, entry, p):
    label = entry["label"]
    if "driver" in entry:
        want = [f"{m}?model" for m in personal_meshes(label, p["stem"]).values()]
    else:
        want = [f"{m}?model" for m in team_meshes(label).values()]
    names = {r.name for r in e.res}
    missing = [w for w in want if w not in names]
    if missing:
        raise SystemExit(f"{label}: no {missing}")


def build(game, mods):
    if OUT.exists():
        shutil.rmtree(OUT)
    SIDECARS.mkdir(parents=True)
    report, built, owners = [], [], {}
    for entry in [e for e in suit_sources.real() if e["kind"] == "mod"]:
        mod = suit_sources.REAL["mods"][entry["mod"]]
        root = Path(mods) / mod["folder"]
        files = {}
        for f in root.rglob("*"):
            if f.is_file():
                files.setdefault(f.name, []).append(f)
        p = paths(entry)
        archives = [("", entry["erp"])] + ([("helmet_", entry["helmet"])] if entry.get("helmet") else [])
        meta = {k: entry[k] for k in ("season", "team", "driver", "mod", "base", "label") if k in entry}
        meta.update(p, sidecars=[], helmet=bool(entry.get("helmet")))
        if entry.get("helmet"):
            meta["helmet_colours"] = entry["helmet_colours"]
        old = entry["base"] if "driver" not in entry else entry["base"][:len(entry["base"]) - len(entry["base"].split("_", 1)[1]) - 1]
        new = entry["label"][:len(old)]
        suit = None
        for key, rel in archives:
            e, shipped = relabel(root / rel, Path(game) / p[key + "base_erp"], old, new, root, files, game, report)
            if not key:
                check_models(e, entry, p)
                suit = {r.name: r for r in e.res}
            else:
                swapped = 0
                for n, r in enumerate(e.res):
                    if r.name in suit and digest(e, r) != digest(e_suit, suit[r.name]):
                        e.res[n] = suit[r.name]
                        swapped += 1
                if swapped:
                    report.append(f"{entry['label']}: the helmet archive takes {swapped} resource(s) from the suit archive")
            for r in e.res:
                h = digest(e, r)
                if owners.setdefault(r.name, (h, entry["label"]))[0] != h:
                    raise SystemExit(f"{r.name}: {entry['label']} and {owners[r.name][1]} differ")
            source = f"suits/{entry['label']}/{Path(p[key + 'target']).name}"
            (ROOT / source).parent.mkdir(parents=True, exist_ok=True)
            e.to_file(str(ROOT / source))
            if not key:
                e_suit = e
            meta[key + "source"] = source
            meta["sidecars"] += shipped
        built.append(meta)
        print(f"{entry['label']}: {entry['season']} {entry['team']}{' ' + entry['driver'] if 'driver' in entry else ''} "
              f"from {mod['name']} ({entry['base']}), {len(meta['sidecars'])} sidecars")
    (OUT / "meta.json").write_text(json.dumps({"suits": built}, indent=1) + "\n")
    for line in report:
        print(line)
    return built


def preview(game, built):
    roots = [SIDECARS, game]
    rows = []
    for m in built:
        e = erpw.Erp(str(ROOT / m["source"]))
        e_base = erpw.Erp(str(Path(game) / m["base_erp"]))
        if "driver" in m:
            team_base = m["base"].split("_", 1)[0]
            e_base = erpw.Erp(str(Path(game) / f"2025_asset_groups/{TEAMS}/{team_base}/{team_base}.erp"))
            team_token = m["label"][:len(m["label"]) - len(m["stem"]) - 1]
            prefixes = [f"eaid://{DRIVERS}/{m['label']}/textures/{m['stem']}_",
                        f"eaid://{TEAMS}/{team_token}/textures/{team_token}_"]
            meshes = personal_meshes(m["label"], m["stem"])
        else:
            prefixes = [f"eaid://{TEAMS}/{m['label']}/textures/{m['label']}_"]
            meshes = team_meshes(m["label"])
        tex = kit_textures(e, prefixes, roots)
        views = render(e_base, e, meshes, tex)
        logo = tex.get("logo_01_d")
        if logo is None:
            logo = np.zeros((1024, 1024, 4), np.uint8)
        title = {"name": f"{m['season']} {m['team']}" + (f" · {m['driver']}" if "driver" in m else "")}
        rows.append((title, {"label": m["label"], "base": m["base"],
                             "design": suit_sources.REAL["mods"][m["mod"]]["name"], "colours": [],
                             "logo_slots": []}, views, tex["livery_v2_d"], logo))
    contact_sheet("Classic Seasons: real suits from mods", rows, OUT / "preview.png", ROOT / "art" / "suits" / "real.png")


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", required=True, type=Path)
    ap.add_argument("--mods", required=True, type=Path)
    a = ap.parse_args(argv[1:])
    built = build(a.game, a.mods)
    preview(a.game, built)


if __name__ == "__main__":
    main(sys.argv)
