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

F1 23's own 2022 and 2023 kit (entries with "mod": "f1_23") builds from --f1-23 on F1 25 bases (f1_23_suits.py); the
build makes every real suit it has a source for and drops the rest from suits/.

    python3 tools/build_real_suits.py --game <F1 25> --mods <folder holding the extracted mods> --f1-23 <F1 23> \
        [--logos f1-23|base]
"""

import argparse
import hashlib
import json
import re
import shutil
import struct
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path[:0] = [str(HERE), str(ROOT.parents[2] / "tools")]

import bcn
import erpw
import f1_23_suits
import suit_sources
from build_suits import contact_sheet, kit_textures, render, surface, team_meshes
from carperf import kind

OUT = ROOT / "suits"
SIDECARS = OUT / "sidecars"
EAID = re.compile(rb"eaid://[\x21-\x7e]+")
MATERIAL_TEXTURE = 2
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


def bad_texture_links(e):
    out = []
    for r in e.res:
        if kind(r) != "GfxMaterialRes":
            continue
        d = e.frag_data(r, 0)
        recs = [struct.unpack_from("<9I", d, 0x20 + 36 * i) for i in range(struct.unpack_from("<I", d, 0)[0])]
        starts = {x[7] for x in recs if x[1] == MATERIAL_TEXTURE}
        if any(x[1] != MATERIAL_TEXTURE and x[8] and x[8] not in starts for x in recs):
            out.append(r.name)
    return out


def paths(entry):
    base, label = entry["base"], entry["label"]
    if "driver" not in entry:
        return {"target": f"2025_asset_groups/{TEAMS}/{label}/{label}.erp",
                "group": f"asset_groups/{TEAMS}/{label}/{label}.assetgroup",
                "base_erp": f"2025_asset_groups/{TEAMS}/{base}/{base}.erp",
                "template": f"asset_groups/{TEAMS}/{base}/{base}.assetgroup"}
    stem = Path(entry["erp"]).stem if "erp" in entry else entry["base"].split("_", 1)[1]
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


def tokens(entry, label):
    base = entry["base"]
    old = base if "driver" not in entry else base[:len(base) - len(base.split("_", 1)[1]) - 1]
    return old, label[:len(old)]


def build_f1_23(game, f1_23, entry, label, logos, db, team, report, textures=True):
    p = paths({**entry, "label": label})
    old, new = tokens(entry, label)
    stem = p.get("stem", label)
    sources = f1_23_suits.f1_23_archives(f1_23, entry)
    meta = {k: entry[k] for k in ("season", "team", "driver", "mod", "base") if k in entry}
    meta.update(p, label=label, f1_23_source=entry["source"], sidecars=[], helmet=bool(entry.get("helmet")),
                textures="f1-23" if textures else "base", logos=logos if textures else "base")
    archives, suit = {}, None
    for key in ("", "helmet_") if entry.get("helmet") else ("",):
        base_erp = Path(game) / p[key + "base_erp"]
        e, _ = relabel(base_erp, base_erp, old, new, None, {}, game, report)
        if textures:
            n = f1_23_suits.swap_textures(e, entry, label, stem, sources["helmet" if key else "suit"], f1_23, report)
            report.append(f"{label}{' helmet' if key else ''}: {n} textures from F1 23 {entry['source']}")
        if not key and textures and logos == "f1-23":
            lm, n = f1_23_suits.take_logos(e, sources["suit"][0], entry, label, stem)
            meta.update(lm)
            report.append(f"{label}: {n} logo mesh records from F1 23")
        for shared in (team or {}, suit or {}):
            e.res = [shared.get(r.name, r) for r in e.res]
        missing = f1_23_suits.unresolved(e, erpw.Erp(str(base_erp)))
        if missing:
            raise SystemExit(f"{label}: names {len(missing)} resource(s) neither archive holds: {missing[:4]}")
        if not key:
            suit = {r.name: r for r in e.res}
        archives[key] = e
    if entry.get("helmet"):
        meta["helmet_colours"] = db.helmet_colours(entry["base"])
    return archives, meta


def build(game, mods, f1_23=None, logos="f1-23"):
    if OUT.exists():
        shutil.rmtree(OUT)
    SIDECARS.mkdir(parents=True)
    report, built, owners, teams = [], [], {}, {}
    db = f1_23_suits.Db(game) if f1_23 else None
    for entry in [e for e in suit_sources.real() if e["kind"] == "mod"]:
        if entry["mod"] == suit_sources.F1_23:
            if not f1_23:
                continue
            archives, meta = build_f1_23(game, f1_23, entry, entry["label"], logos, db,
                                         teams.get((entry["season"], entry["team"])), report)
            if "driver" not in entry:
                teams[(entry["season"], entry["team"])] = {r.name: r for r in archives[""].res}
            for key, e in archives.items():
                for r in e.res:
                    h = digest(e, r)
                    if owners.setdefault(r.name, (h, entry["label"]))[0] != h:
                        raise SystemExit(f"{r.name}: {entry['label']} and {owners[r.name][1]} differ")
                source = f"suits/{entry['label']}/{Path(meta[key + 'target']).name}"
                (ROOT / source).parent.mkdir(parents=True, exist_ok=True)
                e.to_file(str(ROOT / source))
                meta[key + "source"] = source
            built.append(meta)
            print(f"{entry['label']}: {entry['season']} {entry['team']}{' ' + entry['driver'] if 'driver' in entry else ''} "
                  f"from F1 23 {entry['source']} ({entry['base']}, logos {meta['logos']})", flush=True)
            continue
        if not mods:
            continue
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
            broken = bad_texture_links(e)
            if broken:
                raise SystemExit(f"{entry['label']}: a parameter of {broken[0]} names no texture")
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


def f1_23_preview(game, f1_23, built):
    for season in sorted({m["season"] for m in built}):
        rows = []
        for m in [m for m in built if m["season"] == season]:
            entry = suit_sources.by_label(m["label"])
            sources = f1_23_suits.f1_23_archives(f1_23, entry)["helmet" if m["helmet"] else "suit"]
            e = erpw.Erp(str(ROOT / m["source"]))
            team_base = m["base"].split("_", 1)[0]
            e_base = erpw.Erp(str(Path(game) / f"2025_asset_groups/{TEAMS}/{team_base}/{team_base}.erp"))
            if "driver" in m:
                pairs = f1_23_suits.prefixes(entry, m["label"], m["stem"])
                meshes = personal_meshes(m["label"], m["stem"])
                for key, name in (("logos", "logos_model"), ("glove_logos", "glove_logos_model")):
                    if name in m:
                        meshes[key] = f"eaid://character_package/{m[name]}.idf"
            else:
                pairs = f1_23_suits.prefixes(entry, m["label"], m["label"])
                meshes = team_meshes(m["label"])
            prefixes = [ours for ours, _ in pairs]
            tex = kit_textures(e, prefixes, [game])
            views = render(e_base, e, meshes, tex)
            shown = {}
            for part in ("livery_v2_d", "logo_01_d", "glove_d", "helmet_d"):
                for ours, theirs in pairs:
                    name = f"{ours}{part}.tif.image"
                    if any(r.name == name for r in e.res):
                        shown[part] = (f1_23_suits.f1_23_texture(sources, theirs + f"{part}.tif.image", f1_23),
                                       bcn.top_level(surface(e, name), [game]))
                        break
            caption = (f"{m['season']} {m['team']}" + (f" · {m['driver']}" if "driver" in m else "") +
                       f"  ·  {m['label']}  ·  base {m['base']}  ·  F1 23 {m['f1_23_source']}"
                       f"  ·  logos {m['logos']}" + ("" if m["helmet"] or "driver" not in m else "  ·  stock helmet"))
            rows.append((caption, views, shown))
            print(f"preview {m['label']}", flush=True)
        f1_23_suits.sheet(f"Classic Seasons {season}: F1 23 suits", rows, OUT / f"f1-23-{season}.png",
                          ROOT / "art" / "suits" / f"{season}.png")


def preview(game, built):
    roots = [SIDECARS, game]
    rows = []
    for m in [m for m in built if m["mod"] != suit_sources.F1_23]:
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
    ap.add_argument("--mods", type=Path)
    ap.add_argument("--f1-23", type=Path)
    ap.add_argument("--logos", choices=("f1-23", "base"), default="f1-23")
    a = ap.parse_args(argv[1:])
    if not (a.mods or a.f1_23):
        ap.error("give --mods, --f1-23 or both")
    built = build(a.game, a.mods, a.f1_23, a.logos)
    if a.mods:
        preview(a.game, built)
    if a.f1_23:
        f1_23_preview(a.game, a.f1_23, [m for m in built if m["mod"] == suit_sources.F1_23])


if __name__ == "__main__":
    main(sys.argv)
