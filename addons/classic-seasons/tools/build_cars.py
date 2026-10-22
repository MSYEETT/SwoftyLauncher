#!/usr/bin/env python3
"""Build the season car packs from fanfreluche's "F1 <year> cars for F1 25" zips.

Each zip replaces the stock 2025 cars in place, so its archives name their resources
`eaid://f1_2025_vehicle_package/...`. The build renames them byte for byte to `eaid://cs_<year>_vehicle_package/...`
(the same length, so no payload offset moves). A zip without its own teams/common.erp keeps the cars' shared
references on the stock f1_2025 package, whose common.erp the season's asset groups then list. Bare `.mipmaps`
sidecar paths that the zip ships as loose files move to the season's folder with the files; every other sidecar path
stays on the stock files. Each car then gets the frame-rate pass (carperf: shadow casters rebuilt from a lower LOD,
non-livery textures and shipped sidecars capped at 1024 px), and 2016-2021 commons get the stock tyre reshaped to the
period size (tyres13). The build checks that every renamed reference resolves inside the season's archives, writes
them under cars/<year>/ and the file.put and group.register ops in assets/cars.json.
"""

import argparse
import glob
import json
import os
import re
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = ROOT.parent.parent
sys.path.insert(0, str(REPO / "tools"))

import erpw

sys.path.insert(0, str(ROOT / "tools"))
import carperf
import tyres13

SOURCE = "f1_2025_vehicle_package"
OLD = f"eaid://{SOURCE}/"
ANY_EAID = re.compile(rb"eaid://[\x21-\x7e]+")
SIDECAR = re.compile(rb"f1_20\d\d_vehicle_package/[\x21-\x7e]*?\.mipmaps")
STOCK_COMMON = f"2025_asset_groups/{SOURCE}/teams/common.erp"
FOLDERS = ("mercedes", "redbull", "ferrari", "williams", "force_india", "lotus", "toro_rosso", "sauber", "mclaren",
           "haas")


def package(year):
    return f"cs_{year}_vehicle_package"


def find_zip(downloads, year):
    short = str(year)[2:]
    pats = [f"F1 {year} [Cc]ars*F1 25*.zip", f"F1 {short} [Cc]ars*F1 25*.zip"]
    hits = sorted(p for pat in pats for p in glob.glob(os.path.join(downloads, pat)))
    if not hits:
        raise SystemExit(f"{year}: no zip in {downloads}")
    return hits[0]


def rewrite(e, pattern, repl):
    renamed = rewritten = 0
    for r in e.res:
        n = pattern.sub(repl, r.name.encode()).decode()
        if n != r.name:
            r.name = n
            renamed += 1
        for i in range(len(r.frags)):
            d = e.frag_data(r, i)
            nd = pattern.sub(repl, d)
            if nd != d:
                e.set_frag(r, i, nd)
                rewritten += 1
    return renamed, rewritten


def references(e):
    refs = set()
    for r in e.res:
        for i in range(len(r.frags)):
            for m in ANY_EAID.finditer(e.frag_data(r, i)):
                refs.add(m.group(0).split(b"?")[0].decode("latin-1"))
    return refs


def sidecars(e):
    found = set()
    for r in e.res:
        for i in range(len(r.frags)):
            found.update(m.group(0).decode() for m in SIDECAR.finditer(e.frag_data(r, i)))
    return found


def build_season(year, src_zip, out_dir, game):
    pkg = package(year)
    new = f"eaid://{pkg}/"
    report = {"year": year, "zip": os.path.basename(src_zip), "cars": {}}
    with zipfile.ZipFile(src_zip) as z, tempfile.TemporaryDirectory(dir=out_dir.parent) as tmp:
        names = z.namelist()
        loose = {}
        for n in names:
            m = re.search(r"(f1_20\d\d_vehicle_package/.*\.mipmaps)$", n)
            if m and not n.endswith("/"):
                loose[m.group(1)] = n

        def member(rel):
            hits = [n for n in names if n.endswith("/2025_asset_groups/" + rel) or n == "2025_asset_groups/" + rel]
            if len(hits) > 1:
                raise SystemExit(f"{year}: {rel}: {len(hits)} matches in {src_zip}")
            return z.extract(hits[0], tmp) if hits else None

        own_common = member(f"{SOURCE}/teams/common.erp")
        prefix = OLD if own_common else OLD + "teams/"
        pattern = re.compile(re.escape(prefix.encode()) + rb"[^\x00]*")
        target = (new if own_common else new + "teams/").encode()
        repl = lambda m: target + m.group(0)[len(prefix):]

        archives = {f: f"{SOURCE}/teams/{f}/wep/{f}.erp" for f in FOLDERS}
        if own_common:
            archives = {"common": None, **archives}
        loaded = {}
        for key, rel in archives.items():
            path = own_common if key == "common" else member(rel)
            if not path:
                raise SystemExit(f"{year}: {rel} missing from {src_zip}")
            e = erpw.Erp(path)
            renamed, rewritten = rewrite(e, pattern, repl)
            loaded[key] = e
            report["cars"][key] = {"resources": len(e.res), "renamed": renamed, "rewritten": rewritten}

        shipped = set()
        for key, e in loaded.items():
            moved = {s for s in sidecars(e) if s in loose}
            if moved:
                swap = re.compile(b"|".join(re.escape(s.encode()) for s in sorted(moved, key=len, reverse=True)))
                rewrite(e, swap, lambda m: (pkg + m.group(0).decode()[m.group(0).index(b"/"):]).encode())
            shipped |= moved
            report["cars"][key]["sidecars_moved"] = len(moved)
        side = {pkg + s[s.index("/"):]: z.read(loose[s]) for s in shipped}
        trimmed, capped = carperf.cap_sidecars(loaded.values(), side)
        side.update(trimmed)
        report["sidecars"] = len(shipped)
        report["sidecars_capped"] = len(capped["capped"])
        for rel, data in sorted(side.items()):
            dest = out_dir / "sidecars" / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)

        known = set()
        for e in loaded.values():
            known |= {r.name.split("?")[0] for r in e.res}
        if not own_common:
            known |= {r.name.split("?")[0] for r in erpw.Erp(str(game / STOCK_COMMON)).res}
        for folder in FOLDERS:
            e = loaded[folder]
            world = f"{new}teams/{folder}/wep/{folder}.wep"
            if world not in known:
                raise SystemExit(f"{year} {folder}: no World {world}")
            refs = references(e)
            missing = sorted(r for r in refs - known if r.startswith((new, OLD)))
            foreign = sorted(r for r in refs if "_vehicle_package/" in r and not r.startswith((new, OLD))
                             and r not in known)
            missing_sidecars = sorted(s for s in sidecars(e) if s.startswith("f1_")
                                      and not (game / s).exists())
            report["cars"][folder].update(unresolved=missing[:20], unresolved_count=len(missing),
                                          foreign=foreign[:20], foreign_count=len(foreign),
                                          missing_sidecars=missing_sidecars[:20])

        if own_common and year in tyres13.PERIOD:
            report["tyres"] = {}
            tyres13.swap_tyres(loaded["common"], year, game, [loaded[f] for f in FOLDERS], report=report["tyres"])
        for key, e in loaded.items():
            perf = carperf.optimise(e)
            try:
                shadow = carperf.check(e)
            except ValueError as err:
                raise SystemExit(f"{year} {key}: {err}")
            report["cars"][key].update(shadow_before=perf["shadow_before"], shadow_triangles=shadow,
                                       shadows_rebuilt=len(perf["shadows"]), textures_capped=len(perf["textures"]),
                                       textures_left=len(perf["textures_left"]))

        for key, e in loaded.items():
            rel = "teams/common.erp" if key == "common" else f"teams/{key}/wep/{key}.erp"
            out = out_dir / rel
            out.parent.mkdir(parents=True, exist_ok=True)
            e.to_file(str(out))
            report["cars"][key]["bytes"] = out.stat().st_size
    (out_dir / "meta.json").write_text(json.dumps({"common": "own" if own_common else "stock",
                                                   "sidecars": sorted(str(p.relative_to(out_dir / "sidecars"))
                                                                      for p in (out_dir / "sidecars").rglob("*")
                                                                      if p.is_file())
                                                   if (out_dir / "sidecars").exists() else []}, indent=1) + "\n")
    return report


def meta(year):
    path = ROOT / "cars" / str(year) / "meta.json"
    return json.loads(path.read_text()) if path.exists() else None


def built(year):
    m = meta(year)
    if not m:
        return False
    teams = ROOT / "cars" / str(year) / "teams"
    return (m["common"] == "stock" or (teams / "common.erp").exists()) and all(
        (teams / f / "wep" / f"{f}.erp").exists() for f in FOLDERS)


def asset_ops(years):
    ops = []
    for year in years:
        pkg = package(year)
        m = meta(year)
        if m["common"] == "own":
            common = f"2025_asset_groups/{pkg}/teams/common.erp"
            ops.append({"op": "file.put", "source": f"cars/{year}/teams/common.erp", "target": common})
        else:
            common = STOCK_COMMON
        for rel in m["sidecars"]:
            ops.append({"op": "file.put", "source": f"cars/{year}/sidecars/{rel}", "target": rel})
        if (ROOT / "cars" / str(year) / "tyrecompounds" / "tyrecompounds.erp").exists():
            compounds = f"{pkg}/tyrecompounds/tyrecompounds"
            ops.append({"op": "file.put", "source": f"cars/{year}/tyrecompounds/tyrecompounds.erp",
                        "target": f"2025_asset_groups/{compounds}.erp"})
            ops.append({"op": "group.register", "group": f"asset_groups/{compounds}.assetgroup",
                        "erps": [f"2025_asset_groups/{compounds}.erp"],
                        "template": f"asset_groups/{SOURCE}/tyrecompounds/tyrecompounds.assetgroup"})
        if (ROOT / "cars" / str(year) / "incardriver" / "incardriver_f1.erp").exists():
            scenes = f"animation_package/scenes/incardriver/cs{year % 100:02d}/incardriver_f1"
            ops.append({"op": "file.put", "source": f"cars/{year}/incardriver/incardriver_f1.erp",
                        "target": f"2025_asset_groups/{scenes}.erp"})
            ops.append({"op": "group.register", "group": f"asset_groups/{scenes}.assetgroup",
                        "erps": [f"2025_asset_groups/{scenes}.erp"],
                        "template": "asset_groups/animation_package/scenes/incardriver/2025/incardriver_f1.assetgroup"})
        for folder in FOLDERS:
            target = f"2025_asset_groups/{pkg}/teams/{folder}/wep/{folder}.erp"
            ops.append({"op": "file.put", "source": f"cars/{year}/teams/{folder}/wep/{folder}.erp", "target": target})
            ops.append({"op": "group.register", "group": f"asset_groups/{pkg}/teams/{folder}/wep/{folder}.assetgroup",
                        "erps": [common, target],
                        "template": f"asset_groups/{SOURCE}/teams/{folder}/wep/{folder}.assetgroup"})
    return {"schema_version": 1, "ops": ops}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--downloads", default=os.path.expanduser("~/Downloads"))
    ap.add_argument("--game", default="/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
    ap.add_argument("--years", default="2016-2024")
    args = ap.parse_args()
    lo, _, hi = args.years.partition("-")
    years = list(range(int(lo), int(hi or lo) + 1))
    reports_path = ROOT / "cars" / "report.json"
    reports = {r["year"]: r for r in json.loads(reports_path.read_text())} if reports_path.exists() else {}
    for year in years:
        src = find_zip(args.downloads, year)
        out = ROOT / "cars" / str(year)
        if out.exists():
            for p in sorted(out.rglob("*"), reverse=True):
                p.unlink() if p.is_file() else p.rmdir()
        out.mkdir(parents=True, exist_ok=True)
        r = build_season(year, src, out, Path(args.game))
        bad = {k: v["unresolved_count"] for k, v in r["cars"].items() if v.get("unresolved_count")}
        foreign = {k: v["foreign_count"] for k, v in r["cars"].items() if v.get("foreign_count")}
        miss = {k: len(v["missing_sidecars"]) for k, v in r["cars"].items() if v.get("missing_sidecars")}
        print(f"{year}: {len(r['cars'])} archives from {r['zip']}, {r['sidecars']} sidecar(s) moved, "
              f"unresolved {bad or 'none'}, foreign {foreign or 'none'}, missing sidecars {miss or 'none'}", flush=True)
        reports[year] = r
    reports_path.write_text(json.dumps([reports[y] for y in sorted(reports)], indent=1) + "\n")
    all_years = sorted(int(p.name) for p in (ROOT / "cars").iterdir() if p.is_dir() and p.name.isdigit()
                       and built(int(p.name)))
    (ROOT / "assets").mkdir(exist_ok=True)
    (ROOT / "assets" / "cars.json").write_text(json.dumps(asset_ops(all_years), indent=1) + "\n")


if __name__ == "__main__":
    main()
