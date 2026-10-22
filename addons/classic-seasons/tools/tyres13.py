#!/usr/bin/env python3
"""Stock F1 25 tyre meshes reshaped onto the 13-inch bead for the seasons that ship their own common.erp.

fanfreluche's 2016-2021 common.erp replaces the vertex and index buffers and the meshes of the shared tyre models
(shared/tyres/idf/tyres.idf and tyres_wet.idf) with his own 13-inch geometry, about four times the stock vertex
count, but keeps the stock skinning (GfxMshSkin) and physics vertex lists (GfxMshVertex) that index the stock
vertices. `swap_tyres` puts the stock 2025 buffers and meshes back, renamed byte for byte to the season package, and
reshapes them the way the F1 1990 tyres_cm13 step does: the tread keeps its thickness and moves in to the period
outer radius, and the sidewall spans the 13-inch bead to the moved shoulder. The bead radius and bead width of each
axle come from fanfreluche's own tyre, pulled in to sit inside the smallest rim lip and outside the widest rim
flange of the season's cars when those are passed: the width scales by that bead width over the stock one. Normals
and tangents follow the same map. Materials, textures, skinning and every other
resource stay as they are.

    python3 tools/tyres13.py <in common.erp> <year> <out common.erp> [--game DIR] [--cars <car.erp>...]
"""

import argparse
import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import carperf
import erpw

DONOR = "2025_asset_groups/f1_2025_vehicle_package/teams/common.erp"
STOCK = b"eaid://f1_2025_vehicle_package/"
MODELS = ("shared/tyres/idf/tyres.idf?", "shared/tyres/idf/tyres_wet.idf?")
STOCK_WIDTH = {"front": 305, "rear": 405}
PERIOD = {2016: {"front": 245, "rear": 325, "diameter": 660}}
PERIOD.update({y: {"front": 305, "rear": 405, "diameter": 670} for y in range(2017, 2022)})
POSITION, NORMAL, TANGENT = 1, 2, 3
RIM_BAND = 0.006
RIM_OVERLAP = 0.0005


def tyre_resource(name):
    return any(m in name for m in MODELS)


def axle_of(m, bufs):
    out = {}
    for name in {m.names[i] for i in range(m.n)}:
        subs = [i for i in range(m.n) if m.names[i] == name and m.triangles(i)]
        widths = {i: max(abs(p[0]) for p in points(m, i, bufs)) for i in subs}
        lo, hi = min(widths.values()), max(widths.values())
        if hi - lo < 0.02:
            raise ValueError(f"{m.r.name}: {name} submeshes do not split into front and rear by width")
        out.update({i: "rear" if w > (lo + hi) / 2 else "front" for i, w in widths.items()})
    return out


def vertices(m, i, bufs):
    ib = bufs(m.ib()).indices()
    start, count, base = m.field(i, 3), m.field(i, 4), m.field(i, 5)
    return {v + base for v in ib[start:start + count]}


def stream(m, i, semantic):
    return carperf.stream_of(m, i, semantic)


def points(m, i, bufs):
    pos = bufs(stream(m, i, POSITION))
    return [struct.unpack_from("<3f", pos.data, 12 * v) for v in vertices(m, i, bufs)]


def donor_shape(donor, bufs):
    walls, treads = [], {"front": [], "rear": []}
    widths = {"front": [], "rear": []}
    for m in carperf.meshes(donor):
        if MODELS[0] not in m.r.name or m.lod != 0:
            continue
        axles = axle_of(m, bufs)
        for i, a in axles.items():
            pts = points(m, i, bufs)
            radii = [math.hypot(p[1], p[2]) for p in pts]
            if m.names[i] == "wall":
                walls.append((min(radii), max(radii)))
            else:
                treads[a].append(max(radii))
            widths[a].append(max(abs(p[0]) for p in pts))
    return {"bead": min(w[0] for w in walls), "shoulder": max(w[1] for w in walls),
            "outer": {a: max(t) for a, t in treads.items()}, "half_width": {a: max(w) for a, w in widths.items()}}


def radial(r, shape, axle, outer, new_bead):
    shift = outer - shape["outer"][axle]
    bead, shoulder = shape["bead"], shape["shoulder"]
    k = (shoulder + shift - new_bead) / (shoulder - bead)
    if r >= shoulder:
        return r + shift, 1.0
    if r >= bead:
        return new_bead + (r - bead) * k, k
    return r * new_bead / bead, new_bead / bead


def bead_fit(e, donor, donor_bufs):
    cache = {}

    def bufs(name):
        if name not in cache:
            cache[name] = carperf.Buffer(e, name)
        return cache[name]

    stock = [m for m in carperf.meshes(donor) if MODELS[0] in m.r.name and m.lod == 0][0]
    axles = axle_of(stock, donor_bufs)
    own = [m for m in carperf.meshes(e) if MODELS[0] in m.r.name and m.lod == 0][0]
    if own.names != stock.names:
        raise ValueError(f"{own.r.name}: submeshes {own.names} differ from the stock tyre")
    fit = {}
    for i, a in axles.items():
        if stock.names[i] != "wall":
            continue
        for tag, m, b in (("stock", stock, donor_bufs), ("own", own, bufs)):
            pts = points(m, i, b)
            radii = [math.hypot(p[1], p[2]) for p in pts]
            r = min(radii)
            x = max(abs(p[0]) for p, rr in zip(pts, radii) if rr < r + 0.005)
            cur = fit.setdefault(a, {}).setdefault(tag, [r, x])
            cur[0], cur[1] = min(cur[0], r), max(cur[1], x)
    return {a: {"bead": v["own"][0], "width_scale": v["own"][1] / v["stock"][1], "bead_half_width": v["own"][1],
                "stock_bead_half_width": v["stock"][1]} for a, v in fit.items()}


def rim_fit(cars):
    rims = {}
    for e in cars:
        cache = {}

        def bufs(name):
            if name not in cache:
                cache[name] = carperf.Buffer(e, name)
            return cache[name]

        lips = {}
        for m in carperf.meshes(e):
            if m.lod != 0 or not m.part.startswith("wheel_"):
                continue
            axle = "front" if m.part[-2] == "f" else "rear"
            for i in range(m.n):
                if m.triangles(i) < 3:
                    continue
                pts = points(m, i, bufs)
                radii = [math.hypot(p[1], p[2]) for p in pts]
                lip = max(radii)
                flange = max(abs(p[0]) for p, r in zip(pts, radii) if r > lip - RIM_BAND)
                cur = lips.setdefault(axle, [0, 0])
                cur[0], cur[1] = max(cur[0], lip), max(cur[1], flange)
        for axle, (lip, flange) in lips.items():
            cur = rims.setdefault(axle, {"lip": 9.0, "flange": 0.0})
            cur["lip"], cur["flange"] = min(cur["lip"], lip), max(cur["flange"], flange)
    return rims


def reshape_vertex(p, n, t, shape, axle, sx, outer, new_bead):
    rho = math.hypot(p[1], p[2])
    if rho < 1e-9:
        return (p[0] * sx, p[1], p[2]), n, t
    er = (0.0, p[1] / rho, p[2] / rho)
    et = (0.0, -p[2] / rho, p[1] / rho)
    f, d = radial(rho, shape, axle, outer, new_bead)
    s = f / rho
    q = (p[0] * sx, p[1] * s, p[2] * s)

    def remap(v, ax, ar, at):
        x, r, tt = v[0], v[1] * er[1] + v[2] * er[2], v[1] * et[1] + v[2] * et[2]
        m = (x * ax, er[1] * r * ar + et[1] * tt * at, er[2] * r * ar + et[2] * tt * at)
        ln = math.sqrt(sum(c * c for c in m)) or 1.0
        return tuple(c / ln for c in m)

    return q, remap(n, 1 / sx, 1 / d, 1 / s) + (n[3],), remap(t, sx, d, s) + (t[3],)


def swap_tyres(e, year, game, cars=(), report=None):
    if year not in PERIOD:
        raise ValueError(f"{year}: no period tyre size")
    donor = erpw.Erp(str(Path(game) / DONOR))
    cache = {}

    def bufs(name):
        if name not in cache:
            cache[name] = carperf.Buffer(donor, name)
        return cache[name]

    shape = donor_shape(donor, bufs)
    size = PERIOD[year]
    outer = size["diameter"] / 2000
    fit = bead_fit(e, donor, bufs)
    rims = rim_fit(cars)
    for a, rim in rims.items():
        fit[a]["bead"] = min(fit[a]["bead"], rim["lip"] - RIM_OVERLAP)
        half = max(fit[a]["bead_half_width"], rim["flange"] + RIM_OVERLAP)
        fit[a].update(bead_half_width=half, width_scale=half / fit[a]["stock_bead_half_width"], rim=rim)
    sx = {a: fit[a]["width_scale"] for a in fit}
    owner = {}
    for m in carperf.meshes(donor):
        if not tyre_resource(m.r.name):
            continue
        for i, a in axle_of(m, bufs).items():
            key = (stream(m, i, POSITION), stream(m, i, NORMAL), stream(m, i, TANGENT))
            for v in vertices(m, i, bufs):
                if owner.setdefault((key, v), a) != a:
                    raise ValueError(f"{key[0]} vertex {v} shared by front and rear tyres")
    streams = {}
    for (key, v), a in owner.items():
        streams.setdefault(key, {})[v] = a
    data = {}
    for (pos, nrm, tan), axles in streams.items():
        p, n, t = bytearray(bufs(pos).data), bytearray(bufs(nrm).data), bytearray(bufs(tan).data)
        if bufs(pos).stride != 12 or bufs(nrm).stride != 8 or bufs(tan).stride != 8:
            raise ValueError(f"{pos}: unexpected tyre vertex layout")
        for v, a in axles.items():
            q, nn, tt = reshape_vertex(struct.unpack_from("<3f", p, 12 * v), struct.unpack_from("<4e", n, 8 * v),
                                       struct.unpack_from("<4e", t, 8 * v), shape, a, sx[a], outer,
                                       fit[a]["bead"])
            struct.pack_into("<3f", p, 12 * v, *q)
            struct.pack_into("<4e", n, 8 * v, *nn)
            struct.pack_into("<4e", t, 8 * v, *tt)
        data[pos], data[nrm], data[tan] = bytes(p), bytes(n), bytes(t)
    target = f"eaid://cs_{year}_vehicle_package/".encode()
    if len(target) != len(STOCK):
        raise ValueError("package rename changes length")
    swapped = 0
    for r in donor.res:
        if not tyre_resource(r.name) or carperf.kind(r) not in ("GfxBufferRes", "GfxMeshRes"):
            continue
        dst = e.find(r.name.replace(STOCK.decode(), target.decode()))
        if carperf.kind(dst) != carperf.kind(r) or len(dst.frags) != len(r.frags):
            raise ValueError(f"{dst.name}: layout differs from the stock tyre resource")
        for i in range(len(r.frags)):
            blob = data[r.name] if i == 1 and r.name in data else donor.frag_data(r, i)
            e.set_frag(dst, i, blob.replace(STOCK, target))
        swapped += 1
    if report is not None:
        report.update(donor=shape, fit=fit, outer=outer, swapped=swapped,
                      period_width_scale={a: size[a] / STOCK_WIDTH[a] for a in fit})
    return swapped


def measure(e):
    cache = {}

    def bufs(name):
        if name not in cache:
            cache[name] = carperf.Buffer(e, name)
        return cache[name]

    out = {}
    for m in carperf.meshes(e):
        if MODELS[0] not in m.r.name or m.lod != 0:
            continue
        for i, a in axle_of(m, bufs).items():
            pts = points(m, i, bufs)
            radii = [math.hypot(p[1], p[2]) for p in pts]
            cur = out.setdefault(f"{a} {m.names[i]}", [9, 0, 0])
            cur[0], cur[1] = round(min(cur[0], min(radii)), 4), round(max(cur[1], max(radii)), 4)
            cur[2] = round(max(cur[2], max(abs(p[0]) for p in pts)), 4)
    return out


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("year", type=int)
    ap.add_argument("out")
    ap.add_argument("--game", default="/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
    ap.add_argument("--cars", nargs="*", default=[])
    args = ap.parse_args(argv)
    e = erpw.Erp(args.src)
    before = measure(e)
    rep = {}
    swap_tyres(e, args.year, args.game, [erpw.Erp(c) for c in args.cars], rep)
    e.to_file(args.out)
    print({"before": before, "after": measure(erpw.Erp(args.out)), **rep})


if __name__ == "__main__":
    main(sys.argv[1:])
