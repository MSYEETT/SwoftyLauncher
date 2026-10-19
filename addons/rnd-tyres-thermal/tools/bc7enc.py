"""Minimal BC7 encoder (mode 6 only) for UI masks whose colour is (near) constant.

Mode 6 is one subset, RGBA 7-bit endpoints plus one p-bit per endpoint and
4-bit indices. For each 4x4 block the endpoints are the min/max alpha texels,
which is exact for a single varying channel (white line art on alpha). The
output decodes with Swofty UI's bc7.js (bcdec port), which round_trip() checks
against the source texels.

encode_rgba() takes any RGBA image (the R&D card art). Fully transparent texels first
take the alpha-weighted mean colour of their block's visible texels, so a single-colour
frame (wireframe, mask, category icon) stays one colour per block and goes through the
exact alpha path above. Every other block, still mode 6, starts from the best of three
endpoint pairs (principal axis, bounding box, min/max alpha texel) and is refined by
least squares, with each texel's colour error weighted by its alpha. It needs numpy.
"""
from common import bc7_decode

W4 = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64]


def _interp(a, b, w):
    return ((64 - w) * a + w * b + 32) >> 6


def _endpoint(rgba):
    best = None
    for p in (0, 1):
        q = [min(127, max(0, (c - p + 1) >> 1)) for c in rgba]
        val = [(x << 1) | p for x in q]
        err = sum((v - c) ** 2 for v, c in zip(val, rgba))
        if best is None or err < best[0]:
            best = (err, q, p, val)
    return best[1], best[2], best[3]


def _block(px):
    lo = min(px, key=lambda t: t[3])
    hi = max(px, key=lambda t: t[3])
    e0, p0, v0 = _endpoint(list(lo))
    e1, p1, v1 = _endpoint(list(hi))
    pal = [[_interp(v0[c], v1[c], w) for c in range(4)] for w in W4]
    idx = []
    for t in px:
        idx.append(min(range(16), key=lambda i: sum((pal[i][c] - t[c]) ** 2 * (4 if c == 3 else 1) for c in range(4))))
    if idx[0] >= 8:
        e0, e1, p0, p1 = e1, e0, p1, p0
        idx = [15 - i for i in idx]
    bits = 1 << 6
    pos = 7

    def put(v, n):
        nonlocal bits, pos
        bits |= (v & ((1 << n) - 1)) << pos
        pos += n

    for c in range(4):
        put(e0[c], 7)
        put(e1[c], 7)
    put(p0, 1)
    put(p1, 1)
    put(idx[0], 3)
    for i in idx[1:]:
        put(i, 4)
    assert pos == 128
    return bits.to_bytes(16, 'little')


def encode(rgba, w, h):
    assert w % 4 == 0 and h % 4 == 0
    out = bytearray()
    for by in range(0, h, 4):
        for bx in range(0, w, 4):
            px = []
            for y in range(4):
                o = ((by + y) * w + bx) * 4
                for x in range(4):
                    px.append(tuple(rgba[o + 4 * x:o + 4 * x + 4]))
            out += _block(px)
    return bytes(out)


def round_trip(bc7, rgba, w, h):
    dec = bc7_decode(bc7, w, h)
    diff = [abs(a - b) for a, b in zip(dec, rgba)]
    return max(diff), sum(diff) / len(diff), dec


def _bleed(rgba):
    import numpy as np
    px = rgba.astype(np.int64)
    a = px[..., 3:4]
    tot = a.sum(1)
    mean = (px[..., :3] * a).sum(1)
    has = tot[:, 0] > 0
    mean[has] = (mean[has] + tot[has] // 2) // tot[has]
    fill = (a[..., 0] == 0) & has[:, None]
    px[..., :3] = np.where(fill[..., None], mean[:, None, :], px[..., :3])
    return px


def _blocks(rgba, w, h):
    import numpy as np
    img = np.frombuffer(rgba, dtype=np.uint8).reshape(h // 4, 4, w // 4, 4, 4)
    return img.transpose(0, 2, 1, 3, 4).reshape(-1, 16, 4)


def _quantise(ep):
    import numpy as np
    c = np.clip(np.rint(ep), 0, 255).astype(np.int64)
    best = None
    for p in (0, 1):
        q = np.clip((c - p + 1) >> 1, 0, 127)
        val = (q << 1) | p
        err = ((val - ep) ** 2).sum(1)
        if best is None:
            best = [q, np.full(len(c), p), val, err]
        else:
            take = err < best[3]
            best[0] = np.where(take[:, None], q, best[0])
            best[1] = np.where(take, p, best[1])
            best[2] = np.where(take[:, None], val, best[2])
            best[3] = np.where(take, err, best[3])
    return best[0], best[1], best[2]


def _assign(v0, v1, src, wt):
    import numpy as np
    w = np.array(W4, dtype=np.int64)[None, :, None]
    pal = ((64 - w) * v0[:, None, :] + w * v1[:, None, :] + 32) >> 6
    d = (pal[:, None, :, :] - src[:, :, None, :]).astype(np.float64)
    err = (d * d * wt[:, :, None, :]).sum(3)
    idx = err.argmin(2)
    return idx, np.take_along_axis(err, idx[..., None], 2)[..., 0].sum(1)


def _fit(src, wt, idx):
    import numpy as np
    f = np.array(W4, dtype=np.float64)[idx] / 64.0
    g = 1.0 - f
    a = (wt * (g * g)[..., None]).sum(1)
    b = (wt * (g * f)[..., None]).sum(1)
    c = (wt * (f * f)[..., None]).sum(1)
    x0 = (wt * g[..., None] * src).sum(1)
    x1 = (wt * f[..., None] * src).sum(1)
    det = a * c - b * b
    ok = np.abs(det) > 1e-9
    safe = np.where(ok, det, 1.0)
    flat = (x0 + x1) / np.maximum(a + 2 * b + c, 1e-9)
    e0 = np.where(ok, (c * x0 - b * x1) / safe, flat)
    e1 = np.where(ok, (a * x1 - b * x0) / safe, flat)
    return np.clip(e0, 0, 255), np.clip(e1, 0, 255)


def _candidate(px, src, wt, e0, e1, iters):
    import numpy as np
    q0, p0, v0 = _quantise(e0)
    q1, p1, v1 = _quantise(e1)
    idx, err = _assign(v0, v1, px, wt)
    for _ in range(iters):
        f0, f1 = _fit(src, wt, idx)
        r0, rp0, rv0 = _quantise(f0)
        r1, rp1, rv1 = _quantise(f1)
        ridx, rerr = _assign(rv0, rv1, px, wt)
        take = rerr < err
        q0 = np.where(take[:, None], r0, q0)
        q1 = np.where(take[:, None], r1, q1)
        p0 = np.where(take, rp0, p0)
        p1 = np.where(take, rp1, p1)
        v0 = np.where(take[:, None], rv0, v0)
        v1 = np.where(take[:, None], rv1, v1)
        idx = np.where(take[:, None], ridx, idx)
        err = np.where(take, rerr, err)
    return [q0, q1, p0, p1, idx, err]


def _colour_blocks(px):
    import numpy as np
    src = px.astype(np.float64)
    wt = np.empty_like(src)
    wt[..., :3] = src[..., 3:4] / 255.0
    wt[..., 3] = 4.0
    sw = np.sqrt(wt)
    mean = (src * wt).sum(1) / wt.sum(1)
    dev = (src - mean[:, None, :]) * sw
    cov = np.einsum('npi,npj->nij', dev, dev)
    axis = np.ones((len(src), 4)) / 2.0
    for _ in range(16):
        axis = np.einsum('nij,nj->ni', cov, axis)
        axis /= np.maximum(np.linalg.norm(axis, axis=1, keepdims=True), 1e-12)
    axis = axis / np.maximum(sw.mean(1), 1e-3)
    axis /= np.maximum(np.linalg.norm(axis, axis=1, keepdims=True), 1e-12)
    t = np.einsum('npc,nc->np', src - mean[:, None, :], axis)
    seeds = [(mean + t.min(1)[:, None] * axis, mean + t.max(1)[:, None] * axis),
             (src.min(1), src.max(1))]
    lo = np.take_along_axis(src, src[..., 3].argmin(1)[:, None, None].repeat(4, 2), 1)[:, 0]
    hi = np.take_along_axis(src, src[..., 3].argmax(1)[:, None, None].repeat(4, 2), 1)[:, 0]
    seeds.append((lo, hi))
    best = None
    for e0, e1 in seeds:
        cand = _candidate(px, src, wt, e0, e1, 4)
        if best is None:
            best = cand
            continue
        take = cand[5] < best[5]
        best = [np.where(take.reshape((-1,) + (1,) * (b.ndim - 1)), c, b) for c, b in zip(cand, best)]
    q0, q1, p0, p1, idx, _err = best
    out = []
    for n in range(len(px)):
        e0, e1, b0, b1, ix = q0[n].tolist(), q1[n].tolist(), int(p0[n]), int(p1[n]), idx[n].tolist()
        if ix[0] >= 8:
            e0, e1, b0, b1 = e1, e0, b1, b0
            ix = [15 - i for i in ix]
        bits, pos = 1 << 6, 7
        fields = []
        for c in range(4):
            fields += [(e0[c], 7), (e1[c], 7)]
        fields += [(b0, 1), (b1, 1), (ix[0], 3)] + [(i, 4) for i in ix[1:]]
        for v, k in fields:
            bits |= (v & ((1 << k) - 1)) << pos
            pos += k
        out.append(bits.to_bytes(16, 'little'))
    return out


def encode_rgba(rgba, w, h):
    assert w % 4 == 0 and h % 4 == 0
    px = _bleed(_blocks(rgba, w, h))
    flat = (px[:, :, :3] == px[:, :1, :3]).all((1, 2))
    blocks = [None] * len(px)
    cache = {}
    for n in flat.nonzero()[0].tolist():
        key = px[n].astype('uint8').tobytes()
        if key not in cache:
            cache[key] = _block([tuple(t) for t in px[n].tolist()])
        blocks[n] = cache[key]
    rest = (~flat).nonzero()[0]
    if len(rest):
        for n, b in zip(rest.tolist(), _colour_blocks(px[rest])):
            blocks[n] = b
    return b''.join(blocks)
