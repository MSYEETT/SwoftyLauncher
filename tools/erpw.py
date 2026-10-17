import struct, sys, os, hashlib, subprocess

ZSTD = 0x11
STORED = 0x91


def zstd_d(b):
    r = subprocess.run(['zstd', '-dc'], input=b, capture_output=True)
    if r.returncode:
        raise RuntimeError(r.stderr.decode())
    return r.stdout


def zstd_c(b, level=19):
    r = subprocess.run(['zstd', '-c', f'-{level}', '-q'], input=b, capture_output=True)
    if r.returncode:
        raise RuntimeError(r.stderr.decode())
    return r.stdout


class Frag:
    __slots__ = ('name', 'off', 'size', 'flags', 'comp', 'packed', 'raw')


class Res:
    __slots__ = ('name', 'type', 'unk', 'frags', 'hash')


class Erp:
    def __init__(self, path):
        self.path = path
        with open(path, 'rb') as f:
            self.hdr = f.read(0x30)
            assert self.hdr[:4] == b'ERPK'
            self.ver = struct.unpack_from('<I', self.hdr, 4)[0]
            info_off, info_size, data_off = struct.unpack_from('<QQQ', self.hdr, 0x10)
            nres, ntemp = struct.unpack('<II', f.read(8))
            self.res = []
            for _ in range(nres):
                r = Res()
                struct.unpack('<I', f.read(4))
                nlen = struct.unpack('<H', f.read(2))[0]
                r.name = f.read(nlen).rstrip(b'\0').decode()
                r.type = f.read(16)
                r.unk = f.read(4)
                if self.ver >= 4:
                    r.unk += f.read(2)
                r.frags = []
                for _ in range(f.read(1)[0]):
                    fr = Frag()
                    fr.name = f.read(4)
                    fr.off, fr.size, fr.flags = struct.unpack('<QQI', f.read(20))
                    fr.comp = f.read(1)[0]
                    fr.packed = struct.unpack('<Q', f.read(8))[0]
                    fr.raw = None
                    r.frags.append(fr)
                r.hash = f.read(16)
                self.res.append(r)
            assert f.tell() == data_off, (f.tell(), data_off)
            self.data_off = data_off
            for r in self.res:
                for fr in r.frags:
                    f.seek(data_off + fr.off)
                    fr.raw = f.read(fr.packed)

    def find(self, name):
        hits = [r for r in self.res if r.name == name]
        if len(hits) != 1:
            raise KeyError(f'{name}: {len(hits)} hits')
        return hits[0]

    def frag_data(self, r, i):
        fr = r.frags[i]
        return zstd_d(fr.raw) if fr.comp == ZSTD else fr.raw

    def set_frag(self, r, i, data):
        fr = r.frags[i]
        if fr.comp == ZSTD:
            fr.raw = zstd_c(data)
        else:
            fr.raw = data
        fr.size = len(data)
        fr.packed = len(fr.raw)
        r.hash = hashlib.md5(b''.join(self.frag_data(r, j) for j in range(len(r.frags)))).digest()

    def to_file(self, out):
        idx = bytearray()
        nfr = 0
        off = 0
        for r in self.res:
            e = bytearray()
            nm = r.name.encode() + b'\0'
            e += struct.pack('<H', len(nm)) + nm + r.type + r.unk + bytes([len(r.frags)])
            for fr in r.frags:
                fr.off = off
                off += fr.packed
                e += fr.name + struct.pack('<QQI', fr.off, fr.size, fr.flags) + bytes([fr.comp]) + struct.pack('<Q', fr.packed)
                nfr += 1
            e += r.hash
            idx += struct.pack('<I', len(e)) + e
        info = struct.pack('<II', len(self.res), nfr) + idx
        hdr = bytearray(self.hdr)
        struct.pack_into('<QQQ', hdr, 0x10, 0x30, len(info), 0x30 + len(info))
        with open(out, 'wb') as f:
            f.write(hdr)
            f.write(info)
            for r in self.res:
                for fr in r.frags:
                    f.write(fr.raw)


if __name__ == '__main__':
    if sys.argv[1] == 'rebuild':
        e = Erp(sys.argv[2])
        e.to_file(sys.argv[3])
        a, b = open(sys.argv[2], 'rb').read(), open(sys.argv[3], 'rb').read()
        print('identical' if a == b else f'DIFF {len(a)} {len(b)}')
    elif sys.argv[1] == 'replace':
        src, out = sys.argv[2], sys.argv[3]
        e = Erp(src)
        for spec in sys.argv[4:]:
            name, idx, path = spec.rsplit(':', 2)
            r = e.find(name)
            e.set_frag(r, int(idx), open(path, 'rb').read())
            print('replaced', name, idx, len(open(path, 'rb').read()))
        e.to_file(out)
