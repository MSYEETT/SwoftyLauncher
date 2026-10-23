#!/usr/bin/env python3
"""Frame-rate pass over a renamed season car archive.

Some of fanfreluche's cars carry the porter artefact the F1 1990 cars had: every `shadow` submesh of the shadow LOD
is a copy of its part's LOD 0 (100,000+ triangles for the body), drawn into every shadow map. `optimise` rebuilds
each shadow caster above SHADOW_REBUILD triangles, and each caster above PART_REBUILD triangles that copies its
part's LOD 0, from that part's own lower visual LOD: the most detailed one at or under SHADOW_TARGET triangles and a
quarter of the old caster. A car whose LODs never get that small gets its lightest LOD reduced to the same budget by
vertex clustering (each vertex snaps to the first vertex of its grid cell; cells never merge different bones). The
new positions and bone indices go into the shadow vertex buffer, the new indices into the old index range; the freed
index range is cut and every later submesh start moves down with it. Shadow submeshes that share the vertex buffer
keep their triangles, re-indexed, and duplicated shadow records follow their original. `optimise` also caps every
non-livery texture at TEXTURE_CAP px by dropping embedded top mips (the SRV mip count follows); a texture whose
oversize levels stream from a `.mipmaps` sidecar is reported and left alone. `check` refuses a car whose shadow
caster stays above SHADOW_MAX triangles, and `verify` proves an optimised archive against its source. Everything
else in the archive stays byte-identical.

    python3 tools/carperf.py measure <erp>...
    python3 tools/carperf.py optimise <in.erp> <out.erp>
    python3 tools/carperf.py verify <before.erp> <after.erp>
"""

import array
import json
import math
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools"))

import erpw

MESH_HEADER, MESH_RECORD = 76, 88
SHADOW_REBUILD = 20000
SHADOW_TARGET = 20000
SHADOW_MAX = 40000
PART_REBUILD = 1000
LOD0_COPY = 0.9
TEXTURE_CAP = 1024
DECAL_CAP = 2048
SKIP_SUBMESHES = {"shadow", "rainbeads"}
POSITION, BLEND = 1, 7
MESH_NAME = re.compile(r"^(?P<base>.*)\?lod=(?P<lod>\d+)&(?:(?P<part>.+)&)?mesh$")
LIVERY = re.compile(r"_(paint|decal|decals)_[a-z]+\.tif\.image$")
TEAM_TEXTURE = re.compile(r"/teams/[^/]+/textures/")


def kind(r):
    return r.type.rstrip(b"\0").decode()


class Mesh:
    def __init__(self, e, r):
        self.r = r
        self.d = bytearray(e.frag_data(r, 0))
        self.dirty = False
        h = struct.unpack_from("<19i", self.d, 0)
        self.n, nv, ni = h[0], h[1], h[2]
        at = MESH_HEADER + h[5]
        self.refs = []
        for _ in range(nv + ni):
            end = self.d.index(b"\0", at)
            self.refs.append(self.d[at:end].decode())
            at = end + 1
        self.decl = [struct.unpack_from("<5I", self.d, MESH_HEADER + h[10] + 20 * i) for i in range(nv)]
        names = MESH_HEADER + h[7]
        self.names = []
        for i in range(self.n):
            at = names + self.field(i, 12)
            self.names.append(self.d[at:self.d.index(b"\0", at)].decode())
        m = MESH_NAME.match(r.name)
        self.base, self.lod, self.part = (m["base"], int(m["lod"]), m["part"] or "") if m else (r.name, -1, "")

    def field(self, i, k):
        return struct.unpack_from("<I", self.d, MESH_HEADER + i * MESH_RECORD + 4 * k)[0]

    def set_field(self, i, k, v):
        struct.pack_into("<I", self.d, MESH_HEADER + i * MESH_RECORD + 4 * k, v)
        self.dirty = True

    def sphere(self, i):
        return struct.unpack_from("<4f", self.d, MESH_HEADER + i * MESH_RECORD + 24)

    def set_sphere(self, i, s):
        struct.pack_into("<4f", self.d, MESH_HEADER + i * MESH_RECORD + 24, *s)
        self.dirty = True

    def streams(self, i):
        return [self.field(i, 14 + k) for k in range(self.field(i, 0))]

    def ib(self):
        return self.refs[-1] if self.refs else None

    def triangles(self, i):
        return self.field(i, 4) // 3

    def shadow(self):
        return sum(self.triangles(i) for i in range(self.n) if self.names[i] == "shadow")


class Buffer:
    def __init__(self, e, name):
        self.r = e.find(name)
        self.head = list(struct.unpack("<4I", e.frag_data(self.r, 0)))
        self.data = e.frag_data(self.r, 1)

    @property
    def stride(self):
        return self.head[1]

    def item(self, i):
        return self.data[i * self.stride:(i + 1) * self.stride]

    def indices(self):
        if self.stride not in (2, 4):
            raise ValueError(f"{self.r.name}: index stride {self.stride}")
        return array.array("H" if self.stride == 2 else "I", self.data)

    def store(self, e, data, count):
        self.head[2] = count
        e.set_frag(self.r, 0, struct.pack("<4I", *self.head))
        e.set_frag(self.r, 1, data)


def meshes(e):
    return [Mesh(e, r) for r in e.res if kind(r) == "GfxMeshRes" and r.name.endswith("&mesh")]


def shadow_triangles(e):
    return max((m.shadow() for m in meshes(e)), default=0)


def check(e, limit=SHADOW_MAX):
    worst = shadow_triangles(e)
    if worst > limit:
        raise ValueError(f"{e.path}: shadow caster of {worst} triangles (limit {limit})")
    return worst


def stream_of(m, i, semantic):
    hits = [k for k in m.streams(i) if m.decl[k][2] == semantic]
    return m.refs[hits[0]] if len(hits) == 1 else None


def lod_triangles(m, bufs):
    return sum(m.triangles(i) for i in usable(m, bufs)) if m else 0


def wants_rebuild(by_key, bufs, m):
    tris = m.shadow()
    if tris > SHADOW_REBUILD:
        return True
    lod0 = lod_triangles(by_key.get((m.base, m.part, 0)), bufs)
    return tris > PART_REBUILD and lod0 and tris >= LOD0_COPY * lod0


def budget(m):
    return min(SHADOW_TARGET, max(m.triangles(i) for i in range(m.n) if m.names[i] == "shadow") // 4)


def source_lod(by_key, bufs, m):
    lowest = None
    for lod in range(1, m.lod):
        src = by_key.get((m.base, m.part, lod))
        tris = lod_triangles(src, bufs)
        if tris == 0:
            continue
        if tris <= budget(m):
            return src, "lod"
        if lowest is None or tris < lowest[1]:
            lowest = (src, tris)
    return (lowest[0], "cluster") if lowest else (None, None)


def cluster(verts, tris, limit):
    points = [struct.unpack_from("<3f", v) for v in verts]

    def run(cell):
        cells, rep = {}, []
        for i, (x, y, z) in enumerate(points):
            rep.append(cells.setdefault((math.floor(x / cell), math.floor(y / cell), math.floor(z / cell),
                                         verts[i][12:]), i))
        seen, out = set(), []
        for t in range(0, len(tris), 3):
            a, b, c = rep[tris[t]], rep[tris[t + 1]], rep[tris[t + 2]]
            if a == b or b == c or a == c:
                continue
            key = min((a, b, c), (b, c, a), (c, a, b))
            if key not in seen:
                seen.add(key)
                out.append(key)
        return out

    lo, hi = 0.001, 1.0
    best = run(hi)
    for _ in range(18):
        mid = math.sqrt(lo * hi)
        out = run(mid)
        if len(out) <= limit:
            best, hi = out, mid
        else:
            lo = mid
    index, new_verts, new_tris = {}, [], []
    for tri in best:
        for v in tri:
            if v not in index:
                index[v] = len(new_verts)
                new_verts.append(verts[v])
            new_tris.append(index[v])
    return new_verts, new_tris, hi


def usable(m, bufs):
    out = []
    for i in range(m.n):
        if m.names[i] in SKIP_SUBMESHES or m.triangles(i) == 0:
            continue
        pos, blend = stream_of(m, i, POSITION), stream_of(m, i, BLEND)
        if pos and blend and bufs(pos).stride == 12 and bufs(blend).stride == 8:
            out.append(i)
    return out


def gather(src, bufs, ib):
    verts, tris, index = [], [], {}
    for i in usable(src, bufs):
        pos, blend = bufs(stream_of(src, i, POSITION)), bufs(stream_of(src, i, BLEND))
        start, count, base = src.field(i, 3), src.field(i, 4), src.field(i, 5)
        local = []
        for v in ib[start:start + count]:
            key = pos.item(v + base) + blend.item(v + base)[:6] + b"\0\0"
            if key not in index:
                index[key] = len(verts)
                verts.append(key)
            local.append(index[key])
        for t in range(0, len(local) - 2, 3):
            a, b, c = local[t:t + 3]
            if a != b and b != c and a != c:
                tris += (a, b, c)
    return verts, tris


def fit_sphere(sphere, verts):
    cx, cy, cz, r = sphere
    worst = r
    for v in verts:
        x, y, z = struct.unpack_from("<3f", v, 0)
        worst = max(worst, math.sqrt((x - cx) ** 2 + (y - cy) ** 2 + (z - cz) ** 2))
    return sphere if worst <= r else (cx, cy, cz, worst * 1.0001)


def rebuild_shadows(e, report):
    ms = meshes(e)
    cache = {}

    def bufs(name):
        if name not in cache:
            cache[name] = Buffer(e, name)
        return cache[name]

    by_key = {(m.base, m.part, m.lod): m for m in ms}
    jobs = []
    for m in ms:
        if not m.shadow() or not wants_rebuild(by_key, bufs, m):
            continue
        src, mode = source_lod(by_key, bufs, m)
        casters = [i for i in range(m.n) if m.names[i] == "shadow"]
        alike = len({(m.field(i, 3), m.field(i, 4), m.field(i, 5), tuple(m.streams(i))) for i in casters}) == 1
        if not src or not alike:
            why = "no lower LOD" if not src else f"{len(casters)} different shadow submeshes"
            report["shadow_skipped"].append({"mesh": m.r.name, "triangles": m.shadow(), "why": why})
            continue
        verts, tris = gather(src, bufs, bufs(src.ib()).indices())
        how = {"source_lod": src.lod, "method": mode}
        if mode == "cluster":
            verts, tris, cell = cluster(verts, tris, budget(m))
            how["cell_m"] = round(cell, 4)
        if not tris or len(tris) // 3 >= m.triangles(casters[0]):
            report["shadow_skipped"].append({"mesh": m.r.name, "triangles": m.shadow(), "why": "no smaller caster"})
            continue
        jobs.append((m, casters[0], (verts, tris, how, casters[1:])))
    if not jobs:
        return
    groups = {}
    for m, i, geo in jobs:
        pos_name = m.refs[m.streams(i)[0]]
        groups.setdefault((pos_name, m.ib()), []).append((m, i, geo))
    for (pos_name, ib_name), group in groups.items():
        rebuild_group(e, ms, bufs, pos_name, ib_name, group, report)
    for m in ms:
        if m.dirty:
            e.set_frag(m.r, 0, bytes(m.d))


def rebuild_group(e, ms, bufs, pos_name, ib_name, group, report):
    users = [(m, i) for m in ms for i in range(m.n) if any(m.refs[k] == pos_name for k in m.streams(i))]
    layouts = {tuple(m.refs[k] for k in m.streams(i)) for m, i in users}
    if len(layouts) != 1 or any(m.ib() != ib_name for m, _ in users):
        raise ValueError(f"{pos_name}: shadow buffer shared by mixed layouts {sorted(layouts)}")
    pos_buf, blend_buf = (bufs(n) for n in layouts.pop())
    if pos_buf.stride != 12 or blend_buf.stride != 8 or pos_buf.head[2] != blend_buf.head[2]:
        raise ValueError(f"{pos_name}: unexpected shadow vertex layout")
    ibuf = bufs(ib_name)
    old = ibuf.indices()
    span = 1 << (8 * ibuf.stride)
    rebuilt = {(id(m), j) for m, i, geo in group for j in [i, *geo[3]]}
    kept = [(m, i) for m, i in users if (id(m), i) not in rebuilt]

    used = sorted({v + m.field(i, 5) for m, i in kept for v in old[m.field(i, 3):m.field(i, 3) + m.field(i, 4)]})
    remap = {v: n for n, v in enumerate(used)}
    if len(used) > span:
        raise ValueError(f"{pos_name}: {len(used)} kept shadow vertices do not fit the index size")
    pos_out = bytearray(b"".join(pos_buf.item(v) for v in used))
    blend_out = bytearray(b"".join(blend_buf.item(v) for v in used))
    count = len(used)

    new = array.array(old.typecode, old)
    for m, i in kept:
        start, n, base = m.field(i, 3), m.field(i, 4), m.field(i, 5)
        new[start:start + n] = array.array(old.typecode, (remap[v + base] for v in old[start:start + n]))
        m.set_field(i, 5, 0)

    everyone = [(m, i) for m in ms if m.ib() == ib_name for i in range(m.n)]
    cuts = []
    for m, i, (verts, tris, how, dups) in group:
        if len(verts) > span:
            raise ValueError(f"{m.r.name}: {len(verts)} shadow vertices do not fit the index size")
        before = m.triangles(i)
        start, n = m.field(i, 3), m.field(i, 4)
        alone = all(o is m and j in (i, *dups) or o.field(j, 3) >= start + n or o.field(j, 3) + o.field(j, 4) <= start
                    for o, j in everyone)
        if alone and len(tris) <= n:
            new[start:start + len(tris)] = array.array(old.typecode, tris)
            cuts.append((start + len(tris), start + n))
        else:
            if alone:
                cuts.append((start, start + n))
            m.set_field(i, 3, len(new))
            new.extend(tris)
        m.set_field(i, 4, len(tris))
        m.set_field(i, 5, count)
        m.set_sphere(i, fit_sphere(m.sphere(i), verts))
        for j in dups:
            for k in (3, 4, 5):
                m.set_field(j, k, m.field(i, k))
            m.set_sphere(j, m.sphere(i))
        pos_out += b"".join(v[:12] for v in verts)
        blend_out += b"".join(v[12:] for v in verts)
        count += len(verts)
        report["shadows"].append({"mesh": m.r.name, "before": before, "after": len(tris) // 3,
                                  "vertices": len(verts), "duplicates": len(dups), **how})

    cuts = [c for c in sorted(cuts) if c[1] > c[0]]
    if cuts:
        kept_ranges, at = [], 0
        for a, b in cuts:
            kept_ranges.append(new[at:a])
            at = b
        kept_ranges.append(new[at:])
        final = array.array(old.typecode)
        for part in kept_ranges:
            final.extend(part)

        def shift(s):
            return s - sum(min(b, max(a, s)) - a for a, b in cuts)

        for m, i in everyone:
            s = m.field(i, 3)
            if any(a < s < b for a, b in cuts):
                raise ValueError(f"{m.r.name}: submesh {m.names[i]} starts inside a freed index range")
            ns = shift(s)
            if ns != s:
                m.set_field(i, 3, ns)
        new = final
    for m, i in everyone:
        if m.field(i, 3) + m.field(i, 4) > len(new):
            raise ValueError(f"{m.r.name}: submesh {m.names[i]} runs past the index buffer")
    ibuf.store(e, new.tobytes(), len(new))
    pos_buf.store(e, bytes(pos_out), count)
    blend_buf.store(e, bytes(blend_out), count)


def surface(e, r):
    h = struct.unpack("<10I", e.frag_data(r, 0))
    side_n, side_path = 0, None
    if len(r.frags) > 2:
        f2 = e.frag_data(r, 2)
        side_path = f2[1:1 + f2[0]].rstrip(b"\0").decode()
        side_n = struct.unpack_from("<I", f2, 1 + f2[0] + 1)[0]
    return {"fmt": h[2], "w": h[3], "h": h[4], "levels": h[6], "array": h[7], "side": side_n,
            "sidecar": side_path, "full_w": h[3] << side_n, "full_h": h[4] << side_n, "bytes": r.frags[1].size}


def role(name):
    base = name.rsplit("/", 1)[-1]
    livery = LIVERY.search(name)
    if TEAM_TEXTURE.search(name) and livery and "steering" not in base:
        return "livery" if livery.group(1) == "paint" else "decal"
    for key in ("steering", "wheel", "detail", "damage", "driver", "helmet", "visor", "suit", "glove"):
        if key in base:
            return key
    return "other"


def level_sizes(w, h, levels, block):
    return [max(1, (max(1, w >> k) + 3) // 4) * max(1, (max(1, h >> k) + 3) // 4) * block for k in range(levels)]


def block_size(s):
    for block in (8, 16):
        if sum(level_sizes(s["w"], s["h"], s["levels"], block)) * s["array"] == s["bytes"]:
            return block
    return None


def srv_for(e, name):
    hits = [r for r in e.res if kind(r) == "GfxSRVResource" and r.name == name[:-len(".image")]]
    return hits[0] if len(hits) == 1 else None


def cap_for(name, cap):
    r = role(name)
    return None if r == "livery" else max(cap, DECAL_CAP) if r == "decal" else cap


def cap_textures(e, report, cap=TEXTURE_CAP):
    for r in e.res:
        if kind(r) != "GfxSurfaceRes":
            continue
        s = surface(e, r)
        limit = cap_for(r.name, cap)
        if limit is None or max(s["full_w"], s["full_h"]) <= limit:
            continue
        if s["side"]:
            report["textures_left"].append({"texture": r.name, "size": [s["full_w"], s["full_h"]],
                                            "why": f"{s['side']} top level(s) in {s['sidecar']}"})
            continue
        block = block_size(s)
        srv = srv_for(e, r.name)
        if not block or s["array"] != 1 or not srv:
            report["textures_left"].append({"texture": r.name, "size": [s["w"], s["h"]],
                                            "why": "unknown layout" if not block or s["array"] != 1 else "no SRV"})
            continue
        drop = 0
        while max(s["w"] >> drop, s["h"] >> drop) > limit and drop < s["levels"] - 1:
            drop += 1
        if not drop:
            report["textures_left"].append({"texture": r.name, "size": [s["w"], s["h"]], "why": "one mip level"})
            continue
        sizes = level_sizes(s["w"], s["h"], s["levels"], block)
        head = bytearray(e.frag_data(r, 0))
        struct.pack_into("<2I", head, 12, max(1, s["w"] >> drop), max(1, s["h"] >> drop))
        struct.pack_into("<I", head, 24, s["levels"] - drop)
        e.set_frag(r, 0, bytes(head))
        e.set_frag(r, 1, e.frag_data(r, 1)[sum(sizes[:drop]):])
        view = bytearray(e.frag_data(srv, 0))
        if struct.unpack_from("<I", view, 12)[0] != s["levels"]:
            raise ValueError(f"{srv.name}: SRV mip count disagrees with the surface")
        struct.pack_into("<I", view, 12, s["levels"] - drop)
        e.set_frag(srv, 0, bytes(view))
        report["textures"].append({"texture": r.name, "role": role(r.name), "before": [s["w"], s["h"]],
                                   "after": [max(1, s["w"] >> drop), max(1, s["h"] >> drop)],
                                   "bytes_saved": sum(sizes[:drop])})


def sidecar_levels(f2):
    at = 1 + f2[0] + 1
    count = struct.unpack_from("<I", f2, at)[0]
    return at, [struct.unpack_from("<BBQQQ", f2, at + 4 + 26 * i) for i in range(count)]


def cap_sidecars(archives, files, cap=TEXTURE_CAP):
    refs = {}
    for e in archives:
        for r in e.res:
            limit = cap_for(r.name, cap)
            if kind(r) != "GfxSurfaceRes" or len(r.frags) < 3 or limit is None:
                continue
            s = surface(e, r)
            if s["sidecar"] in files and max(s["full_w"], s["full_h"]) > limit:
                refs.setdefault(s["sidecar"], []).append((e, r, s, limit))
    out, report = {}, {"capped": [], "left": []}
    for path, users in sorted(refs.items()):
        data = files[path]
        f2s = {e.frag_data(r, 2) for e, r, _, _ in users}
        dims = {(s["w"], s["h"], s["side"]) for _, _, s, _ in users}
        limit = min(lim for *_, lim in users)
        at, levels = sidecar_levels(next(iter(f2s)))
        w, h, side = next(iter(dims))
        drop = 0
        while drop < side and max(w << (side - drop), h << (side - drop)) > limit:
            drop += 1
        whole = levels and levels[0][2] == 0 and sum(lv[3] for lv in levels) == len(data) and all(
            levels[i + 1][2] == levels[i][2] + levels[i][3] for i in range(len(levels) - 1))
        if len(f2s) != 1 or len(dims) != 1 or not whole or drop >= side:
            report["left"].append({"sidecar": path, "size": [w << side, h << side],
                                   "why": "references disagree" if len(f2s) != 1 or len(dims) != 1 else
                                   "unexpected sidecar layout" if not whole else "embedded levels above the cap"})
            continue
        f2 = bytearray(next(iter(f2s)))
        base = levels[drop][2]
        kept = [(a, b, off - base, packed, raw) for a, b, off, packed, raw in levels[drop:]]
        body = b"".join(struct.pack("<BBQQQ", *lv) for lv in kept)
        f2 = f2[:at] + struct.pack("<I", len(kept)) + body + f2[at + 4 + 26 * len(levels):]
        for e, r, _, _ in users:
            e.set_frag(r, 2, bytes(f2))
        out[path] = data[base:]
        report["capped"].append({"sidecar": path, "before": [w << side, h << side],
                                 "after": [w << (side - drop), h << (side - drop)], "bytes_saved": base,
                                 "textures": len(users)})
    return out, report


def optimise(e, cap=TEXTURE_CAP):
    report = {"shadow_before": shadow_triangles(e), "shadows": [], "shadow_skipped": [], "textures": [],
              "textures_left": []}
    rebuild_shadows(e, report)
    cap_textures(e, report, cap)
    report["shadow_after"] = shadow_triangles(e)
    return report


def measure(e):
    ms = meshes(e)
    lod0 = [sum(m.triangles(i) for i in range(m.n)) for m in ms if m.lod == 0]
    main0 = [sum(m.triangles(i) for i in range(m.n)) for m in ms if m.lod == 0 and not m.part]
    roles = {}
    for r in e.res:
        if kind(r) != "GfxSurfaceRes":
            continue
        s = surface(e, r)
        k = role(r.name)
        cur = roles.setdefault(k, {"max": 0, "count": 0, "bytes": 0, "over_cap": 0})
        cur["max"] = max(cur["max"], s["full_w"], s["full_h"])
        cur["count"] += 1
        cur["bytes"] += s["bytes"]
        cur["over_cap"] += max(s["full_w"], s["full_h"]) > TEXTURE_CAP
    return {"shadow": max((m.shadow() for m in ms), default=0), "shadow_total": sum(m.shadow() for m in ms),
            "lod0_main": max(main0, default=0), "lod0_largest": max(lod0, default=0), "textures": roles}


def verify(before, after):
    if [(r.name, r.type) for r in before.res] != [(r.name, r.type) for r in after.res]:
        raise ValueError("resource list changed")
    changed = {}
    for a, b in zip(before.res, after.res):
        if len(a.frags) != len(b.frags):
            raise ValueError(f"{a.name}: fragment count changed")
        if any(before.frag_data(a, i) != after.frag_data(b, i) for i in range(len(a.frags))):
            changed[a.name] = kind(a)
    bad = sorted(n for n, k in changed.items() if k not in ("GfxMeshRes", "GfxBufferRes", "GfxSurfaceRes",
                                                             "GfxSRVResource"))
    if bad:
        raise ValueError(f"unexpected changes: {bad[:5]}")
    old_ms, new_ms = meshes(before), meshes(after)
    old_bufs, new_bufs = {}, {}

    def ob(n):
        return old_bufs.setdefault(n, Buffer(before, n))

    def nb(n):
        return new_bufs.setdefault(n, Buffer(after, n))

    ibs = {}
    rebuilt, same = [], 0
    for om, nm in zip(old_ms, new_ms):
        if om.refs != nm.refs or om.names != nm.names or om.n != nm.n:
            raise ValueError(f"{om.r.name}: mesh layout changed")
        for i in range(om.n):
            for k in range(22):
                if k not in (3, 4, 5, 6, 7, 8, 9) and om.field(i, k) != nm.field(i, k):
                    raise ValueError(f"{om.r.name}: submesh {om.names[i]} field {k} changed")
        if bytes(om.d[MESH_HEADER + om.n * MESH_RECORD:]) != bytes(nm.d[MESH_HEADER + nm.n * MESH_RECORD:]) or \
                om.d[:MESH_HEADER] != nm.d[:MESH_HEADER]:
            raise ValueError(f"{om.r.name}: mesh tables changed")
        for i in range(om.n):
            if om.field(i, 4) != nm.field(i, 4):
                if om.names[i] != "shadow":
                    raise ValueError(f"{om.r.name}: {om.names[i]} index count changed")
                rebuilt.append((om, nm, i))
                continue
            oi = ibs.setdefault(("o", om.ib()), ob(om.ib()).indices())
            ni = ibs.setdefault(("n", nm.ib()), nb(nm.ib()).indices())
            os_, ns, n = om.field(i, 3), nm.field(i, 3), om.field(i, 4)
            streams = [om.refs[k] for k in om.streams(i)]
            if all(n_ not in changed for n_ in streams) and om.field(i, 5) == nm.field(i, 5):
                if oi[os_:os_ + n] != ni[ns:ns + n]:
                    raise ValueError(f"{om.r.name}: {om.names[i]} indices differ")
            else:
                ob_, nb_ = om.field(i, 5), nm.field(i, 5)
                for s in streams:
                    a, b = ob(s), nb(s)
                    if any(a.item(x + ob_) != b.item(y + nb_) for x, y in zip(oi[os_:os_ + n], ni[ns:ns + n])):
                        raise ValueError(f"{om.r.name}: {om.names[i]} vertices differ in {s}")
            same += 1
    by_key = {(m.base, m.part, m.lod): m for m in new_ms}
    shadows = []
    for om, nm, i in rebuilt:
        tris = nm.triangles(i)
        if tris > SHADOW_TARGET and tris >= om.triangles(i):
            raise ValueError(f"{nm.r.name}: shadow of {tris} triangles")
        pos = nb(nm.refs[nm.streams(i)[0]])
        ni = ibs.setdefault(("n", nm.ib()), nb(nm.ib()).indices())
        start, n, base = nm.field(i, 3), nm.field(i, 4), nm.field(i, 5)
        verts = {pos.item(v + base) for v in ni[start:start + n]}
        lods = [m for (b_, p, l), m in by_key.items() if b_ == nm.base and p == nm.part and l < nm.lod]
        known = set()
        for m in lods:
            for j in usable(m, nb):
                pb = nb(stream_of(m, j, POSITION))
                s, c, bv = m.field(j, 3), m.field(j, 4), m.field(j, 5)
                idx = ibs.setdefault(("n", m.ib()), nb(m.ib()).indices())
                known.update(pb.item(v + bv) for v in idx[s:s + c])
        if not verts <= known:
            raise ValueError(f"{nm.r.name}: {len(verts - known)} shadow vertices not on the car's own LODs")
        cx, cy, cz, r = nm.sphere(i)
        for v in verts:
            x, y, z = struct.unpack("<3f", v)
            if math.sqrt((x - cx) ** 2 + (y - cy) ** 2 + (z - cz) ** 2) > r * 1.001:
                raise ValueError(f"{nm.r.name}: shadow vertex outside the bounding sphere")
        shadows.append({"mesh": nm.r.name, "before": om.triangles(i), "after": tris, "vertices": len(verts)})
    textures = []
    for a, b in zip(before.res, after.res):
        if kind(a) != "GfxSurfaceRes" or a.name not in changed:
            continue
        sa, sb = surface(before, a), surface(after, b)
        block = block_size(sa)
        drop = sa["levels"] - sb["levels"]
        if not block or sb["w"] != max(1, sa["w"] >> drop) or sb["h"] != max(1, sa["h"] >> drop):
            raise ValueError(f"{a.name}: inconsistent capped header")
        if after.frag_data(b, 1) != before.frag_data(a, 1)[sum(level_sizes(sa["w"], sa["h"], sa["levels"], block)[:drop]):]:
            raise ValueError(f"{a.name}: capped data is not the original lower mips")
        if block_size(sb) != block:
            raise ValueError(f"{a.name}: capped size disagrees with its header")
        if max(sb["full_w"], sb["full_h"]) > TEXTURE_CAP or role(a.name) == "livery":
            raise ValueError(f"{a.name}: capped wrongly")
        srv = srv_for(after, b.name)
        if struct.unpack_from("<I", after.frag_data(srv, 0), 12)[0] != sb["levels"]:
            raise ValueError(f"{srv.name}: SRV mip count {sb['levels']} not written")
        textures.append({"texture": a.name, "before": [sa["w"], sa["h"]], "after": [sb["w"], sb["h"]]})
    return {"changed": len(changed), "submeshes_identical": same, "shadows": shadows, "textures": textures}


def main(argv):
    if argv[0] == "measure":
        for p in argv[1:]:
            print(json.dumps({"erp": p, **measure(erpw.Erp(p))}))
    elif argv[0] == "optimise":
        e = erpw.Erp(argv[1])
        rep = optimise(e)
        check(e)
        e.to_file(argv[2])
        print(json.dumps(rep, indent=1))
    elif argv[0] == "verify":
        print(json.dumps(verify(erpw.Erp(argv[1]), erpw.Erp(argv[2])), indent=1))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
