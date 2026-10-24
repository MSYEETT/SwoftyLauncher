"""Mesh streams and a small triangle rasteriser for build_suits.py: UV coverage of a car body, the decal slots of a
suit's logo mesh, and front and back renders of a suit for the previews.

A GfxMeshRes submesh lists its vertex streams (carperf.Mesh.decl: stream, semantic, semantic index, format) and an
index range in the mesh's index buffer. Semantic 1 is the position, 5 a texture coordinate. Buffer formats: 39 three
floats, 29 two halves, 35 four halves.
"""

import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import carperf

POSITION, TEXCOORD = 1, 5
LAYOUTS = {39: ("<f4", 3), 29: ("<f2", 2), 35: ("<f2", 4)}


def mesh(e, name):
    hits = [m for m in carperf.meshes(e) if m.r.name == name]
    if len(hits) != 1:
        raise KeyError(f"{name}: {len(hits)} meshes")
    return hits[0]


def lod0(e, base):
    return mesh(e, f"{base}?lod=0&mesh")


def stream(e, m, i, semantic, index=0):
    for k in m.streams(i):
        if m.decl[k][2] == semantic and m.decl[k][3] == index:
            b = carperf.Buffer(e, m.refs[k])
            fmt = b.head[3]
            if fmt not in LAYOUTS:
                raise ValueError(f"{m.refs[k]}: vertex format {fmt}")
            dtype, n = LAYOUTS[fmt]
            return np.frombuffer(b.data, dtype).reshape(-1, n).astype(np.float64)
    return None


def triangles(e, m, i):
    ib = carperf.Buffer(e, m.ib())
    idx = np.frombuffer(ib.data, "<u2" if ib.stride == 2 else "<u4").astype(np.int64)
    start, count = m.field(i, 3), m.field(i, 4)
    return idx[start:start + count].reshape(-1, 3)


def submesh(e, m, name, nth=0):
    hits = [i for i in range(m.n) if m.names[i] == name]
    return hits[nth] if len(hits) > nth else None


def raster(pts, attrs, height, width, depth=None, zbuf=None, out=None, cover=None):
    """Fill triangles pts (n, 3, 2) in pixel coordinates with attrs (n, 3, k) interpolated. With depth (n, 3) the
    nearer (larger) value wins against zbuf."""
    k = attrs.shape[2]
    out = np.zeros((height, width, k), np.float32) if out is None else out
    cover = np.zeros((height, width), bool) if cover is None else cover
    if depth is not None and zbuf is None:
        zbuf = np.full((height, width), -np.inf, np.float32)
    lo = np.floor(pts.min(1) - 0.5).astype(np.int64)
    hi = np.ceil(pts.max(1) - 0.5).astype(np.int64)
    for t in range(len(pts)):
        x0, y0 = max(lo[t, 0], 0), max(lo[t, 1], 0)
        x1, y1 = min(hi[t, 0], width - 1), min(hi[t, 1], height - 1)
        if x1 < x0 or y1 < y0:
            continue
        (ax, ay), (bx, by), (cx, cy) = pts[t]
        area = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay)
        if abs(area) < 1e-12:
            continue
        xs = np.arange(x0, x1 + 1) + 0.5
        ys = (np.arange(y0, y1 + 1) + 0.5)[:, None]
        w0 = ((bx - xs) * (cy - ys) - (cx - xs) * (by - ys)) / area
        w1 = ((cx - xs) * (ay - ys) - (ax - xs) * (cy - ys)) / area
        w2 = 1 - w0 - w1
        inside = (w0 >= -1e-6) & (w1 >= -1e-6) & (w2 >= -1e-6)
        if not inside.any():
            continue
        region = (slice(y0, y1 + 1), slice(x0, x1 + 1))
        if depth is not None:
            z = w0 * depth[t, 0] + w1 * depth[t, 1] + w2 * depth[t, 2]
            inside &= z > zbuf[region]
            zbuf[region] = np.where(inside, z, zbuf[region])
        a = attrs[t]
        val = w0[..., None] * a[0] + w1[..., None] * a[1] + w2[..., None] * a[2]
        out[region] = np.where(inside[..., None], val, out[region])
        cover[region] |= inside
    return out, cover, zbuf


def components(tris):
    parent = np.arange(tris.max() + 1)

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for a, b, c in tris:
        ra, rb, rc = find(a), find(b), find(c)
        parent[rb] = ra
        parent[find(rc)] = ra
    roots = np.array([find(t[0]) for t in tris])
    return [np.nonzero(roots == r)[0] for r in np.unique(roots)]


def overlap(a, b):
    w = min(a[2], b[2]) - max(a[0], b[0])
    h = min(a[3], b[3]) - max(a[1], b[1])
    if w <= 0 or h <= 0:
        return 0.0
    return w * h / min((a[2] - a[0]) * (a[3] - a[1]), (b[2] - b[0]) * (b[3] - b[1]))


def decal_slots(e, base, name, size):
    """Atlas rectangles (x0, y0, x1, y1 in pixels of a size x size atlas) of a decal mesh, merged where they overlap,
    each with the positions it lands on and the body UV under it. A decal vertex holds the body UV in its first
    texture coordinate pair and the atlas UV in the second. Decals whose rectangles overlap by more than half of the
    smaller one share a rectangle (one logo shown in several places)."""
    m = lod0(e, base)
    found = []
    for i in range(m.n):
        if m.names[i] != name:
            continue
        pos, uv = stream(e, m, i, POSITION), stream(e, m, i, TEXCOORD)
        tris = triangles(e, m, i)
        for part in components(tris):
            v = np.unique(tris[part])
            u = uv[v, 2:4] * size
            found.append([*u.min(0), *u.max(0), pos[v].mean(0), uv[v, :2].mean(0)])
    slots = []
    for x0, y0, x1, y1, centre, body in sorted(found, key=lambda s: -(s[2] - s[0]) * (s[3] - s[1])):
        box = [x0, y0, x1, y1]
        for s in slots:
            if overlap(s["box"], box) > 0.5:
                b = s["box"]
                s["box"] = [min(b[0], x0), min(b[1], y0), max(b[2], x1), max(b[3], y1)]
                s["at"].append(centre.tolist())
                s["uv"].append(body.tolist())
                break
        else:
            slots.append({"box": box, "at": [centre.tolist()], "uv": [body.tolist()]})
    for s in slots:
        s["box"] = [int(math.floor(s["box"][0])), int(math.floor(s["box"][1])), int(math.ceil(s["box"][2])),
                    int(math.ceil(s["box"][3]))]
    return sorted(slots, key=lambda s: (s["box"][1], s["box"][0]))


def sample(tex, uv):
    h, w = tex.shape[:2]
    x = np.clip((uv[..., 0] % 1.0) * w, 0, w - 1).astype(np.int64)
    y = np.clip((uv[..., 1] % 1.0) * h, 0, h - 1).astype(np.int64)
    return tex[y, x]


class View:
    """Orthographic front (back=False) or back view of a standing driver, y up."""

    def __init__(self, height, width, back, top=1.72, bottom=-0.04):
        self.h, self.w, self.back = height, width, back
        self.scale = height / (top - bottom)
        self.top = top
        self.rgb = np.zeros((height, width, 3), np.float32)
        self.alpha = np.zeros((height, width), np.float32)
        self.zbuf = np.full((height, width), -np.inf, np.float32)

    def project(self, pos):
        x = -pos[..., 0] if self.back else pos[..., 0]
        z = -pos[..., 2] if self.back else pos[..., 2]
        return np.stack([self.w / 2 + x * self.scale, (self.top - pos[..., 1]) * self.scale], -1), z

    def draw(self, pos, uv, tris, tex, bias=0.0, blend=False):
        uv = uv[:, 2:4] if blend else uv[:, :2]
        p = pos[tris]
        xy, z = self.project(p)
        n = np.cross(p[:, 1] - p[:, 0], p[:, 2] - p[:, 0])
        n /= np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)
        shade = 0.55 + 0.45 * np.abs(n[:, 2])
        attrs = np.concatenate([uv[tris], np.repeat(shade[:, None, None], 3, 1)], 2)
        out, cover, _ = raster(xy, attrs, self.h, self.w, z + bias, self.zbuf.copy() if blend else self.zbuf)
        col = sample(tex, out[..., :2]).astype(np.float32)
        rgb = col[..., :3] * out[..., 2:3]
        a = (col[..., 3] / 255.0 if tex.shape[2] == 4 and blend else np.ones(cover.shape)) * cover
        self.rgb = self.rgb * (1 - a[..., None]) + rgb * a[..., None]
        self.alpha = np.maximum(self.alpha, cover.astype(np.float32))

    def image(self, background):
        bg = np.array(background, np.float32)
        out = bg * (1 - self.alpha[..., None]) + self.rgb * self.alpha[..., None]
        return np.clip(out, 0, 255).astype(np.uint8)
