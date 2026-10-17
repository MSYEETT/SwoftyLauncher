import hashlib
import struct

ERPK = b'ERPK'
STORED = 0x91


def erp_index(path):
    with open(path, 'rb') as f:
        h = f.read(0x30)
        if h[:4] != ERPK:
            raise ValueError(f'{path}: not an ERP')
        ver = struct.unpack_from('<H', h, 4)[0]
        io, isz, do = struct.unpack_from('<QQQ', h, 0x10)
        f.seek(io)
        idx = f.read(isz)
    n = struct.unpack_from('<I', idx, 0)[0]
    p = 8
    out = []
    for _ in range(n):
        esz, nlen = struct.unpack_from('<IH', idx, p)
        q = p + 6
        uri = idx[q:q + nlen - 1].decode()
        q += nlen
        typ = idx[q:q + 16]
        q += 16
        arg, nf = struct.unpack_from('<IB', idx, q)
        q += 5
        frags = []
        for _ in range(nf):
            cc, off, size, flags, comp, packed = struct.unpack_from('<4sQQIBQ', idx, q)
            q += 0x21
            frags.append(dict(fourcc=cc, off=off, size=size, flags=flags, comp=comp, packed=packed))
        digest = idx[q:q + 16]
        q += 16
        assert q == p + 4 + esz
        out.append(dict(uri=uri, type=typ, arg=arg, frags=frags, digest=digest, data_off=do))
        p = q
    return ver, out


def erp_fragments(path, uri):
    import subprocess
    ver, res = erp_index(path)
    hit = [r for r in res if r['uri'] == uri]
    if len(hit) != 1:
        raise KeyError(f'{uri}: {len(hit)} hits in {path}')
    r = hit[0]
    out = []
    with open(path, 'rb') as f:
        for fr in r['frags']:
            f.seek(r['data_off'] + fr['off'])
            raw = f.read(fr['packed'])
            if fr['comp'] == 0x11:
                raw = subprocess.run(['zstd', '-dc'], input=raw, capture_output=True, check=True).stdout
            assert len(raw) == fr['size'], (uri, len(raw), fr['size'])
            out.append(raw)
    return r, out


def erp_write(path, resources):
    idx = bytearray()
    data = bytearray()
    nfrag = 0
    for r in resources:
        e = bytearray()
        name = r['uri'].encode() + b'\0'
        typ = r['type'] if isinstance(r['type'], bytes) else r['type'].encode()
        e += struct.pack('<H', len(name)) + name + typ.ljust(16, b'\0')[:16] + struct.pack('<IB', r.get('arg', 1), len(r['frags']))
        blob = b''
        for fr in r['frags']:
            payload = fr['data']
            cc = fr.get('fourcc', b'temp')
            e += cc + struct.pack('<QQIBQ', len(data), len(payload), fr.get('flags', 0x10), STORED, len(payload))
            data += payload
            blob += payload
            nfrag += 1
        e += hashlib.md5(blob).digest()
        idx += struct.pack('<I', len(e)) + e
    info = struct.pack('<II', len(resources), nfrag) + idx
    hdr = ERPK + struct.pack('<I', 3) + b'\0' * 8 + struct.pack('<QQQ', 0x30, len(info), 0x30 + len(info)) + b'\0' * 8
    with open(path, 'wb') as f:
        f.write(hdr + info + data)


def lng_hash(key):
    h = 5381
    for c in key.encode():
        c = c - 256 if c >= 128 else c
        h = (h * 33 + c) & 0xffffffff
    return h


def lngt_build(pairs, buckets=None):
    pairs = list(pairs)
    nb = buckets or max(1, len(pairs))
    order = sorted(range(len(pairs)), key=lambda i: lng_hash(pairs[i][0]) % nb)
    table = [[0, 0] for _ in range(nb)]
    sida, sidb, lngb = bytearray(), bytearray(), bytearray()
    for n, i in enumerate(order):
        b = lng_hash(pairs[i][0]) % nb
        if not table[b][1]:
            table[b][0] = n
        table[b][1] += 1
        sida += struct.pack('>II', len(sidb), len(lngb))
        sidb += pairs[i][0].encode() + b'\0'
        lngb += pairs[i][1].encode() + b'\0'
    body = b'HSHS' + struct.pack('>IIII', 12, nb, 5381, 33)
    body += b'HSHT' + struct.pack('>I', nb * 8) + b''.join(struct.pack('>II', s, c) for s, c in table)
    body += b'SIDA' + struct.pack('>II', len(order) * 8, len(order)) + bytes(sida)
    body += b'SIDB' + struct.pack('>I', len(sidb)) + bytes(sidb)
    body += b'LNGB' + struct.pack('>I', len(lngb)) + bytes(lngb)
    return b'LNGT' + struct.pack('>I', 8 + len(body)) + body


def lngt_parse(d):
    assert d[:4] == b'LNGT'
    p = 8
    nb = None
    hsht = sida = sidb = lngb = None
    while p < len(d):
        tag = d[p:p + 4]
        if tag == b'HSHS':
            _, nb, seed, mult = struct.unpack_from('>IIII', d, p + 4)
            p += 20
        elif tag == b'HSHT':
            size = struct.unpack_from('>I', d, p + 4)[0]
            hsht = [struct.unpack_from('>II', d, p + 8 + 8 * i) for i in range(nb)]
            p += 8 + size
        elif tag == b'SIDA':
            size, n = struct.unpack_from('>II', d, p + 4)
            sida = [struct.unpack_from('>II', d, p + 12 + 8 * i) for i in range(n)]
            p += 12 + size
        elif tag == b'SIDB':
            size = struct.unpack_from('>I', d, p + 4)[0]
            sidb = d[p + 8:p + 8 + size]
            p += 8 + size
        elif tag == b'LNGB':
            size = struct.unpack_from('>I', d, p + 4)[0]
            lngb = d[p + 8:p + 8 + size]
            p += 8 + size
        else:
            raise ValueError(f'unknown chunk {tag!r} at {p:#x}')
    cut = lambda b, o: b[o:b.index(b'\0', o)].decode('utf-8', 'replace')
    return [(cut(sidb, k), cut(lngb, v)) for k, v in sida], nb, hsht


def lngt_lookup(d, key):
    pairs, nb, hsht = lngt_parse(d)
    start, count = hsht[lng_hash(key) % nb]
    for i in range(start, start + count):
        if pairs[i][0] == key:
            return pairs[i][1]
    return None


def bc7_mode6_block(alpha16, rgb=(255, 0, 255)):
    w = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64]
    lo, hi = min(alpha16), max(alpha16)
    a0 = lo & ~1
    a1 = hi | 1
    c0 = tuple(c & ~1 for c in rgb)
    c1 = tuple(c | 1 for c in rgb)
    ends = [(c0, a0, 0), (c1, a1, 1)]
    interp = [((64 - x) * a0 + x * a1 + 32) >> 6 for x in w]
    idx = [min(range(16), key=lambda i: abs(interp[i] - a)) for a in alpha16]
    if idx[0] >= 8:
        ends.reverse()
        idx = [15 - i for i in idx]
    bits = 0
    pos = 0

    def put(v, n):
        nonlocal bits, pos
        bits |= (v & ((1 << n) - 1)) << pos
        pos += n

    put(1 << 6, 7)
    for ch in range(3):
        for e in ends:
            put(e[0][ch] >> 1, 7)
    for e in ends:
        put(e[1] >> 1, 7)
    for e in ends:
        put(e[2], 1)
    for i, v in enumerate(idx):
        put(v, 3 if i == 0 else 4)
    assert pos == 128
    return bits.to_bytes(16, 'little')


def dds_bc7(width, height, data):
    hdr = struct.pack('<4sIIIIIII44sII4sIIIIIIIIII', b'DDS ', 124, 0x1 | 0x2 | 0x4 | 0x1000 | 0x80000, height, width,
                      len(data), 0, 1, b'\0' * 44, 32, 0x4, b'DX10', 0, 0, 0, 0, 0, 0x1000, 0, 0, 0, 0)
    return hdr + struct.pack('<IIIII', 98, 3, 0, 1, 0) + data


def bc7_recolour(width, height, data, rgb=(255, 0, 255)):
    import io
    from PIL import Image
    im = Image.open(io.BytesIO(dds_bc7(width, height, data)))
    im.load()
    a = im.getchannel('A').load()
    bw, bh = (width + 3) // 4, (height + 3) // 4
    out = bytearray()
    for by in range(bh):
        for bx in range(bw):
            block = [a[min(bx * 4 + x, width - 1), min(by * 4 + y, height - 1)] for y in range(4) for x in range(4)]
            out += bc7_mode6_block(block, rgb)
    assert len(out) == len(data), (len(out), len(data))
    check = Image.open(io.BytesIO(dds_bc7(width, height, bytes(out))))
    check.load()
    ca, cr = check.getchannel('A').load(), check.getchannel('R').load()
    err = max(abs(ca[x, y] - a[x, y]) for y in range(height) for x in range(width))
    red = min(cr[x, y] for y in range(height) for x in range(width))
    return bytes(out), err, red, im, check
