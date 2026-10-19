import struct, sys, collections
def parse(path):
    with open(path, 'rb') as f:
        return _parse(f)


def _parse(f):
    hdr = f.read(0x38)
    magic, ver = hdr[:4], struct.unpack_from('<I', hdr, 4)[0]
    info_off, info_size, data_off = struct.unpack_from('<QQQ', hdr, 0x10)
    nres, ntemp = struct.unpack_from('<II', hdr, 0x30)
    f.seek(0x38)
    out = []
    for _ in range(nres):
        esz = struct.unpack('<I', f.read(4))[0]
        nlen = struct.unpack('<H', f.read(2))[0]
        name = f.read(nlen).rstrip(b'\0').decode()
        rtype = f.read(16).rstrip(b'\0').decode()
        unk = struct.unpack('<I', f.read(4))[0]
        nfrag = f.read(1)[0]
        frags = []
        for _ in range(nfrag):
            fname = f.read(4).rstrip(b'\0').decode()
            off, size, flags = struct.unpack('<QQI', f.read(20))
            comp = f.read(1)[0]
            packed = struct.unpack('<Q', f.read(8))[0]
            frags.append((fname, off, size, flags, comp, packed))
        h = f.read(16)
        out.append((name, rtype, unk, frags, h.hex()))
    return dict(ver=ver, info_off=info_off, info_size=info_size, data_off=data_off, nres=nres, ntemp=ntemp), out
if __name__ == '__main__':
    meta, res = parse(sys.argv[1])
    print(meta)
    types = collections.Counter(r[1] for r in res)
    print(types.most_common())
    comps = collections.Counter(fr[4] for r in res for fr in r[3])
    print('compression', comps)
    if len(sys.argv) > 2:
        for r in res:
            if sys.argv[2] in r[0] or sys.argv[2] == '*':
                print(r[1], r[0], [(a, s, p, hex(fl), c) for a, o, s, fl, c, p in r[3]])
