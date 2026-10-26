"""Team car archives of older F1 games converted onto a stock F1 25 car slot, and a check of car archives against F1 25.

The conversion keys on each resource's version: cars of F1 2017, 2019, 2020 and F1 23 are converted and checked; F1 22 and F1 24
resources of the sampled versions take the same paths. Format notes: re/db/subsystems/legacy_cars.md.

  python3 tools/legacy_car.py convert <old car.erp> <F1 25 slot.erp> <out.erp> [--sidecars <old game folder>]...
  python3 tools/legacy_car.py check <car.erp>...
  python3 tools/legacy_car.py selftest

Needs the zstd command (erpw.py).
"""
import argparse
import hashlib
import os
import re
import struct
import sys
import tempfile
import zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import erpw
import gfx_material as gm

ZSTD, ZLIB, STORED = (0x10, 0x11), 1, 0x80
OLD_HEADER, HEADER, RECORD = 36, 76, 88
POSITION, NORMAL, TANGENT, TEXCOORD, BLEND = 1, 2, 3, 5, 7
ROLES = {
    6: [(POSITION, 0), (NORMAL, 0), (TANGENT, 0), (TEXCOORD, 0), (TEXCOORD, 1), (BLEND, 0)],
    5: [(POSITION, 0), (NORMAL, 0), (TANGENT, 0), (TEXCOORD, 0), (BLEND, 0)],
    4: [(POSITION, 0), (NORMAL, 0), (TANGENT, 0), (TEXCOORD, 0)],
    2: [(POSITION, 0), (BLEND, 0)],
}
DECL_FORMAT = {(12, 0x27): 0x2f, (8, 0x23): 0x23, (4, 0x1d): 0x1d, (4, 0xd): 0xd, (8, 0x21): 0x1f}
INTERLEAVED = [(0, 12, 0x27), (12, 8, 0x23), (20, 8, 0x23), (28, 8, 0x23), (36, 4, 0xd)]
GEOMETRY = {'GfxModelRes', 'ModelProxyRes', 'GfxMeshRes', 'GfxBufferRes', 'GfxMatBindRes'}
SCENE = {'World', 'WOInstances', 'EventGraph'}
SOURCE_DROP = SCENE | {'VTF', 'GfxSampleState'}
SHADER_FALLBACK = {
    'eaid://shader_package/tyre_sidewall/tyre_sidewall_no_logo.nefx2':
        'eaid://shader_package/tyre_sidewall/tyre_sidewall.nefx2',
}
VERSIONS = {'GfxBufferRes': 4, 'GfxMeshRes': 0x60000, 'GfxMaterialRes': 2, 'GfxSurfaceRes': 9}
KNOWN = {'GfxBufferRes': {3, 4}, 'GfxMeshRes': {0x10000, 0x60000}, 'GfxMaterialRes': {1, 2}, 'GfxSurfaceRes': {8, 9}}
CHECKED = {(1, 0x2f, 12, 0x27), (2, 0x23, 8, 0x23), (3, 0x23, 8, 0x23), (5, 0x23, 8, 0x23), (5, 0x1d, 4, 0x1d),
           (5, 0x1d, 8, 0x21), (7, 0xd, 4, 0xd), (7, 0xd, 4, 0x1d), (7, 0xd, 8, 0x21), (7, 0xd, 12, 0x27),
           (7, 0x1f, 4, 0x1d), (7, 0x1f, 8, 0x21), (7, 0x1f, 8, 0x23), (7, 0x1f, 12, 0x27)}


def kind(r):
    return r.type.rstrip(b'\0').decode()


def version(r):
    return struct.unpack_from('<I', r.unk)[0]


def set_version(r, v):
    r.unk = struct.pack('<I', v) + r.unk[4:]


def cstr(d, at):
    return d[at:d.index(b'\0', at)].decode()


def data(r, i):
    fr = r.frags[i]
    if fr.comp & STORED:
        return fr.raw
    if fr.comp in ZSTD:
        return erpw.zstd_d(fr.raw)
    if fr.comp == ZLIB:
        return zlib.decompress(fr.raw)
    return fr.raw


def put(r, i, payload):
    fr = r.frags[i]
    fr.raw = erpw.zstd_c(payload)
    fr.comp = erpw.ZSTD
    fr.size = len(payload)
    fr.packed = len(fr.raw)


def copy_res(r, name):
    g = erpw.Res()
    g.name, g.type, g.unk, g.hash = name, r.type, r.unk, r.hash
    g.frags = []
    for fr in r.frags:
        c = erpw.Frag()
        for a in erpw.Frag.__slots__:
            setattr(c, a, getattr(fr, a))
        g.frags.append(c)
    return g


def lz4_block(src, off, out_size):
    out = bytearray()
    i, n = off, len(src)
    while len(out) < out_size and i < n:
        tok = src[i]
        i += 1
        lit = tok >> 4
        if lit == 15:
            while True:
                b = src[i]
                i += 1
                lit += b
                if b != 255:
                    break
        out += src[i:i + lit]
        i += lit
        if len(out) >= out_size or i >= n:
            break
        mo = src[i] | (src[i + 1] << 8)
        i += 2
        ml = tok & 15
        if ml == 15:
            while True:
                b = src[i]
                i += 1
                ml += b
                if b != 255:
                    break
        ml += 4
        start = len(out) - mo
        if mo >= ml:
            out += out[start:start + ml]
        else:
            for k in range(ml):
                out.append(out[start + k])
    return bytes(out[:out_size])


def _levels(desc, at, entry, lead):
    count = struct.unpack_from('<I', desc, at)[0]
    if count > 32 or at + 4 + count * entry > len(desc):
        return None
    levels = []
    for k in range(count):
        o = at + 4 + k * entry + lead
        levels.append((desc[o],) + struct.unpack_from('<QQQ', desc, o + 1))
    return levels


def sidecar_levels(desc):
    n = desc[0]
    path = desc[1:1 + n].decode().rstrip('\0')
    if len(desc) > 1 + n and desc[1 + n] == 0:
        levels = _levels(desc, 2 + n, 26, 1)
        if levels is not None and all(c in (0, 3) for c, *_ in levels):
            return path, 9, levels
    levels = _levels(desc, 1 + n, 25, 0)
    if levels is None:
        raise ValueError(f'{path}: sidecar descriptor of {len(desc)} bytes is neither layout')
    return path, 8, levels


class Sidecars:
    def __init__(self, roots):
        self.roots = roots
        self.by_leaf = None

    def find(self, path):
        for root in self.roots:
            f = os.path.join(root, path)
            if os.path.isfile(f):
                return f
        if self.by_leaf is None:
            self.by_leaf = {}
            for root in self.roots:
                for d, _, files in os.walk(root):
                    for x in files:
                        if x.endswith('.mipmaps'):
                            self.by_leaf.setdefault(x, []).append(os.path.join(d, x))
        hits = self.by_leaf.get(path.rsplit('/', 1)[-1], [])
        parent = path.rsplit('/', 3)[-3] if path.count('/') >= 3 else None
        hits = [h for h in hits if parent is None or f'/{parent}/' in h] or hits
        return hits[0] if len(hits) == 1 else None


def embed_sidecar(head, tail, desc, sidecars):
    path, layout, levels = sidecar_levels(desc)
    if not levels:
        return None, f'{path}: descriptor v{layout} lists no sidecar levels'
    f = sidecars.find(path) if sidecars else None
    if not f:
        return None, f'{path}: no sidecar, archive levels only'
    with open(f, 'rb') as fh:
        buf = fh.read()
    raw, skipped = [], []
    for comp, off, packed, size in sorted(levels, key=lambda m: m[3]):
        if comp == 0:
            level = buf[off:off + size]
        elif comp == 3:
            level = lz4_block(buf, off, size)
        else:
            skipped.append(comp)
            break
        if len(level) != size:
            skipped.append('short')
            break
        raw.insert(0, level)
    if not raw:
        return None, f'{path}: sidecar levels not decodable {skipped}, archive levels only'
    h = list(struct.unpack('<10I', head))
    k = len(raw)
    h[3], h[4], h[6] = h[3] << k, h[4] << k, h[6] + k
    note = f' ({len(levels) - k} top level(s) skipped: compression {skipped[0]})' if skipped else ''
    return (struct.pack('<10I', *h), b''.join(raw) + tail, k), f'{path}: {k} sidecar level(s) embedded, {h[3]}x{h[4]}{note}'


def split_interleaved(src, heads, report):
    split, made, pending = {}, [], {}
    names = [r.name for r in src.res if kind(r) == 'GfxBufferRes']
    nxt = 1 + max(int(n.rsplit('vb=', 1)[1]) for n in names if 'vb=' in n)
    for r in list(src.res):
        if kind(r) != 'GfxBufferRes' or heads[r.name][1] != 40:
            continue
        raw = data(r, 1)
        count = heads[r.name][2]
        prefix = r.name.rsplit('vb=', 1)[0] + 'vb='
        split[r.name] = []
        for off, size, fmt in INTERLEAVED:
            g = copy_res(r, f'{prefix}{nxt}')
            head = (heads[r.name][0], size, count, fmt)
            heads[g.name] = head
            pending[g.name] = (head, b''.join(raw[i * 40 + off:i * 40 + off + size] for i in range(count)))
            made.append(g)
            split[r.name].append(g.name)
            nxt += 1
        report.append(f'{r.name}: interleaved 40-byte stream split into {split[r.name][0]}..{split[r.name][-1]}')
    return split, made, pending


def mesh(name, d, heads, report, split):
    h = struct.unpack_from('<9I', d, 0)
    n, nvb, nib = h[0], h[1], h[2]
    if n == 0:
        return bytes(HEADER)
    pool = OLD_HEADER + h[5]
    refs, at = [], pool
    for _ in range(nvb + nib):
        s = cstr(d, at)
        refs.append((at - pool, s))
        at += len(s) + 1
    by_off = dict(refs)
    names = d[OLD_HEADER + h[7]:OLD_HEADER + h[8]]
    trail = d[OLD_HEADER + h[8]:]
    kept, roles = [], {}
    for i in range(n):
        f = list(struct.unpack_from('<22I', d, OLD_HEADER + i * RECORD))
        streams = [by_off[f[14 + k]] for k in range(f[0])]
        if f[0] == 1 and streams[0] in split:
            streams = split[streams[0]]
            f[0] = len(streams)
        if f[0] not in ROLES:
            report.append(f'dropped submesh {cstr(names, f[12])!r}: {f[0]} stream(s) of strides {[heads[s][1] for s in streams]}')
            continue
        for k, s in enumerate(streams):
            role = (k,) + ROLES[f[0]][k]
            if roles.setdefault(s, role) != role:
                raise ValueError(f'{s} used as {roles[s]} and {role}')
        kept.append((f, streams, trail[f[10]:f[10] + 8]))
    if not kept:
        return bytes(HEADER)
    vbs = [s for _, s in refs[:nvb] if s in roles]
    vbs += [s for parts in split.values() for s in parts if s in roles and s not in vbs]
    ibs = [s for _, s in refs[nvb:]]
    index = {s: k for k, s in enumerate(vbs)}
    pool25, offs = bytearray(), {}
    for s in vbs + ibs:
        offs[s] = len(pool25)
        pool25 += s.encode() + b'\0'
    n25 = len(kept)
    recs, trail25 = bytearray(), bytearray()
    for i, (f, streams, tr) in enumerate(kept):
        g = f[:14] + [0] * 8
        for k, s in enumerate(streams):
            g[14 + k] = index[s]
        g[10] = 8 * i
        recs += struct.pack('<22I', *g)
        trail25 += tr
    decl = bytearray()
    for s in vbs:
        slot, sem, idx = roles[s]
        decl += struct.pack('<5I', slot, 0, sem, idx, DECL_FORMAT[(heads[s][1], heads[s][3])])
    tail = b''.join(struct.pack('<II', 1, k) for k in range(len(vbs)))
    o_pool = n25 * RECORD
    o_ib = o_pool + offs[ibs[0]]
    o_names = o_pool + len(pool25)
    o_trail = o_names + len(names)
    o_decl = o_trail + len(trail25)
    o_tail = o_decl + len(decl)
    root = re.search(r'\?lod=(\d+)&mesh$', name)
    head = struct.pack('<19I', n25, len(vbs), len(ibs), n25, n25, o_pool, o_ib, o_names, o_trail, len(vbs),
                       o_decl, o_tail, 0, o_pool, 0, o_pool, 0, 0, int(root.group(1)) if root else 0)
    return head + recs + pool25 + names + trail25 + decl + tail


def base(e):
    stem = os.path.basename(e.path)[:-len('.erp')]
    hits = [r.name for r in e.res if kind(r) == 'ModelProxyRes' and r.name.endswith(f'/{stem}.idf?render')]
    if len(hits) != 1:
        raise ValueError(f'{e.path}: {len(hits)} model proxies named {stem}.idf?render')
    return hits[0][:-len('.idf?render')]


def refs_of(r):
    d = b''.join(data(r, j) for j in range(len(r.frags)))
    return {m.decode() for m in re.findall(rb'eaid://[\x21-\x7e]+', d)}


def convert(src_path, donor_path, out_path, sidecars=None):
    src, don = erpw.Erp(src_path), erpw.Erp(donor_path)
    unknown = sorted({(kind(r), version(r)) for r in src.res if version(r) not in KNOWN.get(kind(r), [version(r)])})
    if unknown:
        raise ValueError(f'{src_path}: no conversion for ' + ', '.join(f'{k} version {v:#x}' for k, v in unknown))
    sb, db = base(src), base(don)
    report = [f'source {src_path}', f'donor {donor_path}', f'base {sb} -> {db}']
    heads = {r.name: struct.unpack('<4I', data(r, 0)) for r in src.res if kind(r) == 'GfxBufferRes'}
    split, made, pending = split_interleaved(src, heads, report)
    grown, out = {}, []
    for r in src.res + made:
        k = kind(r)
        if k in SOURCE_DROP or k == 'GfxBufferRes' and r.name in split:
            continue
        v = version(r)
        if k == 'GfxBufferRes' and r.name in pending:
            h = list(pending[r.name][0])
            h[0] |= 0x1000
            put(r, 0, struct.pack('<4I', *h))
            put(r, 1, pending[r.name][1])
        elif k == 'GfxBufferRes' and v == 3:
            h = list(heads[r.name])
            h[0] |= 0x1000
            put(r, 0, struct.pack('<4I', *h))
            put(r, 1, data(r, 1))
        elif k == 'GfxMeshRes' and v == 0x10000:
            put(r, 0, mesh(r.name, data(r, 0), heads, report, split))
        elif k == 'GfxMaterialRes' and v == 1:
            m23 = gm.parse23(data(r, 0))
            if m23.shader not in gm.catalog() and m23.shader in SHADER_FALLBACK:
                report.append(f'{r.name}: shader {m23.shader} -> {SHADER_FALLBACK[m23.shader]}')
                m23.shader = SHADER_FALLBACK[m23.shader]
            m25, notes = gm.plan23to25(m23)
            put(r, 0, gm.write25(m25))
            if notes['defaulted'] or notes['dropped']:
                report.append(f"{r.name}: defaulted {notes['defaulted']} dropped {notes['dropped']}")
        elif k == 'GfxSurfaceRes':
            head, tail = data(r, 0), data(r, 1)
            if len(r.frags) == 3:
                full, note = embed_sidecar(head, tail, data(r, 2), sidecars)
                r.frags = r.frags[:2]
                if full:
                    head, tail, extra = full
                    grown[r.name[:-len('.image')]] = extra
                report.append(note)
            put(r, 0, head)
            put(r, 1, tail)
        else:
            for i in range(len(r.frags)):
                put(r, i, data(r, i))
        if k in VERSIONS:
            set_version(r, VERSIONS[k])
        if r.name.startswith((sb + '.idf&', sb + '.idf?render', sb + '.emb')):
            r.name = db + r.name[len(sb):]
        out.append(r)
    for r in out:
        if kind(r) == 'GfxSRVResource' and r.name in grown:
            v = bytearray(data(r, 0))
            struct.pack_into('<I', v, 12, struct.unpack_from('<I', v, 12)[0] + grown[r.name])
            put(r, 0, bytes(v))
    for r in out:
        r.hash = hashlib.md5(b''.join(data(r, j) for j in range(len(r.frags)))).digest()

    have = {r.name for r in out}
    keep = [r for r in don.res if kind(r) in SCENE]
    named = {r.name: r for r in don.res}
    todo = list(keep)
    while todo:
        r = todo.pop()
        for n in refs_of(r):
            if n in named and n not in have and named[n] not in keep and kind(named[n]) not in GEOMETRY:
                keep.append(named[n])
                todo.append(named[n])
    contexts = sorted({n for r in keep for n in refs_of(r) if n.startswith(db + '.emb?context=')}
                      | {r.name for r in don.res if r.name.startswith(db + '.emb?context=')})
    default = next(r for r in out if r.name == db + '.emb?context=default')
    for c in contexts:
        if c not in have:
            out.append(copy_res(default, c))
            report.append(f'{c}: copy of the source default context')
    kept = {r.name for r in keep}
    missing = sorted({n for r in keep for n in refs_of(r) if n in named and n not in have and n not in kept})
    report += [f'donor kept: {r.name}' for r in keep] + [f'donor reference not in the output: {n}' for n in missing]
    don.res = out + keep
    don.to_file(out_path)
    report.append(f'wrote {out_path}: {len(out)} source + {len(keep)} donor resources')
    return report


def check(path):
    e = erpw.Erp(path)
    errs = {}

    def err(what):
        errs[what] = errs.get(what, 0) + 1

    bufs = {r.name: r for r in e.res if kind(r) == 'GfxBufferRes'}
    heads = {n: struct.unpack('<4I', data(r, 0)) for n, r in bufs.items()}
    srvs = {r.name: r for r in e.res if kind(r) == 'GfxSRVResource'}
    shaders = gm.catalog()
    indices, submeshes = {}, 0
    for n, r in bufs.items():
        if version(r) != 4:
            err('buffer version')
        if not heads[n][0] & 0x1000:
            err('buffer flag')
    for r in e.res:
        k = kind(r)
        if any(fr.comp not in (erpw.ZSTD, erpw.STORED) for fr in r.frags):
            err('fragment compression')
        if k == 'GfxMaterialRes':
            if version(r) != 2:
                err('material version')
            elif gm.parse25(data(r, 0)).shader not in shaders:
                err('shader not in F1 25')
        elif k == 'GfxSurfaceRes':
            if version(r) != 9:
                err('surface version')
            if len(r.frags) not in (2, 3):
                err('surface fragments')
            elif len(r.frags) == 3 and sidecar_levels(data(r, 2))[1] != 9:
                err('sidecar descriptor layout')
            srv = srvs.get(r.name[:-len('.image')])
            mips = struct.unpack_from('<I', data(r, 0), 24)[0]
            if srv and struct.unpack_from('<I', data(srv, 0), 12)[0] != mips:
                err('SRV level count')
        elif k == 'GfxSampleState':
            err('sampler state')
        if k != 'GfxMeshRes':
            continue
        if version(r) != 0x60000:
            err('mesh version')
            continue
        d = data(r, 0)
        h = struct.unpack_from('<19I', d, 0)
        if h[0] == 0:
            continue
        refs, at = [], HEADER + h[5]
        for _ in range(h[1] + h[2]):
            s = cstr(d, at)
            refs.append(s)
            at += len(s) + 1
        decl = [struct.unpack_from('<5I', d, HEADER + h[10] + 20 * i) for i in range(h[9])]
        ib = refs[h[1]]
        if ib not in bufs:
            err('missing index buffer')
            continue
        if ib not in indices:
            indices[ib] = memoryview(data(bufs[ib], 1)).cast('H' if heads[ib][1] == 2 else 'I')
        for i in range(h[0]):
            f = struct.unpack_from('<22I', d, HEADER + i * RECORD)
            submeshes += 1
            if f[3] + f[4] > heads[ib][2]:
                err('index range')
            idx = indices[ib][f[3]:f[3] + f[4]]
            top = max(idx) + f[5] if len(idx) else -1
            for k in range(f[0]):
                slot, _, sem, _, fmt = decl[f[14 + k]]
                vb = refs[f[14 + k]]
                if slot != k:
                    err('declaration slot')
                if vb not in heads:
                    err('missing vertex buffer')
                    continue
                if (sem, fmt, heads[vb][1], heads[vb][3]) not in CHECKED:
                    err(f'semantic {sem} format {fmt:#x} on stride {heads[vb][1]} format {heads[vb][3]:#x}')
                if top >= heads[vb][2]:
                    err('vertex range')
    return submeshes, errs


def _res(name, rtype, v, payloads, comp=erpw.ZSTD):
    r = erpw.Res()
    r.name, r.type, r.unk, r.hash, r.frags = name, rtype.encode().ljust(16, b'\0'), struct.pack('<I', v), bytes(16), []
    for payload in payloads:
        fr = erpw.Frag()
        fr.name, fr.flags, fr.off, fr.raw = b'temp', 16, 0, payload
        fr.comp, fr.size, fr.packed = 0, len(payload), len(payload)
        r.frags.append(fr)
        if comp in ZSTD:
            put(r, len(r.frags) - 1, payload)
            fr.comp = comp
        elif comp == ZLIB:
            fr.raw, fr.comp, fr.packed = zlib.compress(payload), ZLIB, len(zlib.compress(payload))
        else:
            fr.comp = comp
    return r


def _old_mesh(base_vertex):
    vbs = [f'eaid://t/vb={i}' for i in range(5)]
    pool = b''.join(s.encode() + b'\0' for s in vbs + ['eaid://t/ib=0'])
    offs = [pool.index(s.encode()) for s in vbs]
    rec = [5, 4, 2, 0, 3, base_vertex, 0, 0, 0, 1, 0, 1, 0, 0] + offs + [0] * 3
    body = struct.pack('<22I', *rec) + pool + b'part\0' + bytes(8)
    names = RECORD + len(pool)
    return struct.pack('<9I', 1, 5, 1, 1, 1, RECORD, RECORD + offs[-1] + len(vbs[-1]) + 1, names, names + 5) + body


def selftest():
    assert data(_res('z', 'GfxSRVResource', 2, [b'abc'], 0x10), 0) == b'abc'
    assert data(_res('s', 'GfxSRVResource', 2, [b'abc'], 0x90), 0) == b'abc'
    assert data(_res('l', 'GfxSRVResource', 2, [b'abc'], ZLIB), 0) == b'abc'
    assert lz4_block(bytes([0x44]) + b'abcd' + bytes([4, 0]), 0, 12) == b'abcdabcdabcd'
    entry = struct.pack('<QQQ', 16, 12, 64)
    path = b'f1_2019_vehicle_package/teams/t/textures/t_d.tif.mipmaps'
    v8 = bytes([len(path)]) + path + struct.pack('<I', 1) + b'\3' + entry + struct.pack('<2f', 40, 1)
    v9 = bytes([len(path)]) + path + b'\0' + struct.pack('<I', 1) + b'\0\3' + entry + struct.pack('<2f', 40, 1)
    empty = bytes([len(path)]) + path + b'\0' + struct.pack('<I', 0) + struct.pack('<2f', 40, 1)
    assert sidecar_levels(v8) == (path.decode(), 8, [(3, 16, 12, 64)])
    assert sidecar_levels(v9) == (path.decode(), 9, [(3, 16, 12, 64)])
    assert sidecar_levels(empty) == (path.decode(), 9, [])
    streams = [(12, 0x27, b'\0' * 36), (8, 0x23, b'\0' * 24), (8, 0x23, b'\0' * 24), (8, 0x23, b'\0' * 24), (4, 0xd, b'\0' * 12)]
    results = []
    for base_vertex in (0, 1):
        heads, res = {}, []
        for i, (stride, fmt, payload) in enumerate(streams):
            heads[f'eaid://t/vb={i}'] = (0x109, stride, 3, fmt)
            res.append(_res(f'eaid://t/vb={i}', 'GfxBufferRes', 4, [struct.pack('<4I', 0x1109, stride, 3, fmt), payload]))
        heads['eaid://t/ib=0'] = (0x10a, 2, 3, 0x13)
        res.append(_res('eaid://t/ib=0', 'GfxBufferRes', 4, [struct.pack('<4I', 0x110a, 2, 3, 0x13), struct.pack('<3H', 0, 1, 2)]))
        report = []
        res.append(_res('eaid://t/idf/t.idf&lod=0&mesh', 'GfxMeshRes', 0x60000, [mesh('eaid://t/idf/t.idf&lod=0&mesh', _old_mesh(base_vertex), heads, report, {})]))
        e = erpw.Erp.__new__(erpw.Erp)
        e.hdr, e.ver, e.res = b'ERPK' + struct.pack('<I', 3) + bytes(0x28), 3, res
        with tempfile.TemporaryDirectory() as d:
            out = os.path.join(d, 't.erp')
            e.to_file(out)
            results.append(check(out))
    assert results[0] == (1, {}), results[0]
    assert results[1] == (1, {'vertex range': 5}), results[1]
    return 'selftest: ok'


def _main(argv):
    ap = argparse.ArgumentParser(prog='legacy_car.py', description='Converts a team car archive of an older F1 game onto a stock F1 25 car slot, and checks car archives against F1 25.')
    sub = ap.add_subparsers(dest='cmd', required=True)
    c = sub.add_parser('convert', help='convert an old car archive onto an F1 25 slot')
    c.add_argument('src', help="the old game's team archive (<team>/wep/<team>.erp)")
    c.add_argument('donor', help='a stock F1 25 car archive of the slot, from the baseline')
    c.add_argument('out', help='the converted archive to write')
    c.add_argument('--sidecars', action='append', default=[], metavar='FOLDER',
                   help="the old game's folder, searched for the .mipmaps the archive names (repeatable)")
    k = sub.add_parser('check', help="check car archives against F1 25's versions, streams and ranges")
    k.add_argument('erp', nargs='+')
    sub.add_parser('selftest', help='run the built-in tests on synthetic resources')
    a = ap.parse_args(argv)
    if a.cmd == 'selftest':
        print(selftest())
        return 0
    if a.cmd == 'convert':
        print('\n'.join(convert(a.src, a.donor, a.out, Sidecars(a.sidecars) if a.sidecars else None)))
        return 0
    bad = 0
    for path in a.erp:
        n, errs = check(path)
        print(f'{path}: {n} submeshes, {"no errors" if not errs else errs}')
        bad += bool(errs)
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(_main(sys.argv[1:]))
