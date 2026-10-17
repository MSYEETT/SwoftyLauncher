import struct, sys, json, base64

ALIGN = {'stri': 4, 'bool': 4, 'ui08': 4, 'ui16': 4, 'ui32': 4, 'si32': 4, 'fp32': 4, 'rgba': 4,
         'ui64': 8, 'si64': 8, 'vec3': 16, 'vec4': 16, 'quat': 16, 'woid': 16, 'shnm': 16,
         'bbox': 32, 'mat4': 16}


def fnv1a32(b):
    h = 0x811C9DC5
    for c in b:
        h = ((h ^ c) * 0x01000193) & 0xFFFFFFFF
    return h


class Val:
    __slots__ = ('type', 'value', 'ref')

    def __init__(self, type, value, ref=None):
        self.type, self.value, self.ref = type, value, ref

    def __repr__(self):
        return f'Val({self.type},{self.value!r})'


class Obj:
    def __init__(self, items=None, version=1):
        self.items = items if items is not None else []
        self.version = version

    def keys(self):
        return [k.value for k, v in self.items]

    def get(self, key, default=None):
        for k, v in self.items:
            if k.value == key:
                return v
        return default

    def __getitem__(self, key):
        for k, v in self.items:
            if k.value == key:
                return v
        raise KeyError(key)

    def set(self, key, val):
        for i, (k, v) in enumerate(self.items):
            if k.value == key:
                self.items[i] = (k, val)
                return
        self.items.append((Val('stri', key), val))


class Arr:
    def __init__(self, items=None, tag='!ili'):
        self.items = items if items is not None else []
        self.tag = tag


class Iar:
    def __init__(self, raw):
        self.raw = raw


class Data:
    def __init__(self, type, bpd=0, raw=b''):
        self.type, self.bpd, self.raw = type, bpd, raw
        self.index = {}
        if type == 'stri':
            o = 0
            while o < len(raw):
                n = raw[o]
                s = raw[o + 1:o + 1 + n].decode('utf-8', 'replace')
                self.index.setdefault(s, o)
                o += n + 2
        else:
            for i in range(self.count()):
                self.index.setdefault(raw[i * bpd:(i + 1) * bpd], i)

    def count(self):
        return len(self.raw) // self.bpd if self.bpd else 0

    def get(self, idx):
        if self.type == 'stri':
            n = self.raw[idx]
            return self.raw[idx + 1:idx + 1 + n].decode('utf-8', 'replace')
        return decode_scalar(self.type, self.raw[idx * self.bpd:(idx + 1) * self.bpd])

    def intern(self, value):
        if self.type == 'stri':
            if value in self.index:
                return self.index[value]
            b = value.encode('utf-8')
            assert len(b) < 256
            o = len(self.raw)
            self.raw += bytes([len(b)]) + b + b'\0'
            self.index[value] = o
            return o
        b = encode_scalar(self.type, value, self.bpd)
        if b in self.index:
            return self.index[b]
        i = self.count()
        self.raw += b
        self.index[b] = i
        return i


def decode_scalar(t, b):
    if t == 'ui32': return struct.unpack('<I', b)[0]
    if t == 'si32': return struct.unpack('<i', b)[0]
    if t == 'ui64': return struct.unpack('<Q', b)[0]
    if t == 'si64': return struct.unpack('<q', b)[0]
    if t == 'ui16': return struct.unpack('<H', b)[0]
    if t == 'ui08': return b[0]
    if t == 'bool': return bool(b[0])
    if t == 'fp32': return round(struct.unpack('<f', b)[0], 6)
    if t == 'rgba': return '#' + b.hex()
    if t == 'shnm': return b.rstrip(b'\0').decode('utf-8', 'replace')
    if t in ('vec3', 'vec4', 'quat'): return [round(x, 6) for x in struct.unpack('<4f', b)]
    if t == 'mat4': return [round(x, 6) for x in struct.unpack('<16f', b)]
    if t == 'bbox': return [round(x, 6) for x in struct.unpack('<8f', b)]
    if t == 'woid': return b.hex()
    return b.hex()


def encode_scalar(t, v, bpd):
    if t == 'ui32': return struct.pack('<I', v)
    if t == 'si32': return struct.pack('<i', v)
    if t == 'ui64': return struct.pack('<Q', v)
    if t == 'si64': return struct.pack('<q', v)
    if t == 'ui16': return struct.pack('<H', v)
    if t == 'ui08': return bytes([v])
    if t == 'bool': return bytes([1 if v else 0])
    if t == 'fp32': return struct.pack('<f', v)
    if t == 'rgba': return bytes.fromhex(v.lstrip('#'))
    if t == 'shnm': return v.encode().ljust(16, b'\0')[:16]
    if t in ('vec3', 'vec4', 'quat'): return struct.pack('<4f', *v)
    if t == 'mat4': return struct.pack('<16f', *v)
    if t == 'bbox': return struct.pack('<8f', *v)
    return bytes.fromhex(v).ljust(bpd, b'\0')


class Pkg:
    def __init__(self, data=None):
        self.name = b'\0\0\0\0'
        self.root = Obj()
        self.vca = []
        if data is not None:
            self._read(data)

    def tindex(self, t):
        for i, d in enumerate(self.vca):
            if d.type == t:
                return i
        self.vca.append(Data(t, 0 if t == 'stri' else 16 if ALIGN.get(t, 4) >= 16 and t != 'mat4' and t != 'bbox' else {'mat4': 64, 'bbox': 32, 'ui64': 8, 'si64': 8, 'ui16': 2, 'ui08': 1, 'bool': 1}.get(t, 4)))
        return len(self.vca) - 1

    def _read(self, d):
        self.d = d
        assert d[:4] == b'!pkg'
        self.name = d[4:8]
        a, b = struct.unpack_from('<II', d, 8)
        vo = b & 0xFFFFFF
        assert d[vo:vo + 4] == b'!vca'
        n = struct.unpack_from('<I', d, vo + 4)[0]
        for i in range(n):
            o = struct.unpack_from('<I', d, vo + 8 + 4 * i)[0] & 0xFFFFFF
            tag = d[o:o + 4]
            if tag == b'!sbi':
                ln = struct.unpack_from('<I', d, o + 4)[0]
                self.vca.append(Data('stri', 0, d[o + 8:o + 8 + ln]))
            else:
                assert tag == b'!vbi', tag
                t = d[o + 4:o + 8].decode()
                cnt, bpd = struct.unpack_from('<II', d, o + 8)
                self.vca.append(Data(t, bpd, d[o + 16:o + 16 + cnt * bpd]))
        self.root = self._chunk(a & 0xFFFFFF)
        del self.d

    def _val(self, ot):
        t, off = ot >> 24, ot & 0xFFFFFF
        if t == 0x80:
            return self._chunk(off)
        dt = self.vca[t]
        return Val(dt.type, dt.get(off), (t, off))

    def _chunk(self, o):
        d = self.d
        tag = d[o:o + 4]
        if tag == b'!idi':
            h = struct.unpack_from('<I', d, o + 4)[0]
            cnt, ver = h & 0xFFFFFF, h >> 24
            es = 12 if ver == 1 else 8
            items = []
            for i in range(cnt):
                p = o + 8 + es * i
                k, v = struct.unpack_from('<II', d, p)
                items.append((self._val(k), self._val(v)))
            return Obj(items, ver)
        if tag == b'!ili' or tag == b'!vca':
            cnt = struct.unpack_from('<I', d, o + 4)[0]
            return Arr([self._val(struct.unpack_from('<I', d, o + 8 + 4 * i)[0]) for i in range(cnt)], tag.decode())
        if tag == b'!iar':
            return Iar(d[o + 4:o + 12])
        raise ValueError(f'chunk {tag} at {o:#x}')

    def _ref(self, v):
        if v.ref is not None:
            t, off = v.ref
            if t < len(self.vca) and self.vca[t].type == v.type:
                try:
                    if self.vca[t].get(off) == v.value:
                        return (t << 24) | off
                except Exception:
                    pass
        t = self.tindex(v.type)
        off = self.vca[t].intern(v.value)
        v.ref = (t, off)
        return (t << 24) | off

    def to_bytes(self):
        out = bytearray()
        out += b'!pkg' + self.name + b'\0' * 8
        refs = {}

        from collections import deque
        q = deque()

        def emit(node):
            start = len(out)
            if isinstance(node, Obj):
                es = 12 if node.version == 1 else 8
                out.extend(b'!idi' + struct.pack('<I', len(node.items) | (node.version << 24)))
                table = len(out)
                out.extend(b'\0' * (es * len(node.items)))
                items = sorted(node.items, key=lambda kv: fnv1a32(kv[0].value.encode())) if node.version == 1 else node.items
                for i, (k, v) in enumerate(items):
                    struct.pack_into('<I', out, table + es * i, self._ref(k))
                    if node.version == 1:
                        struct.pack_into('<I', out, table + es * i + 8, fnv1a32(k.value.encode()))
                    if isinstance(v, Val):
                        struct.pack_into('<I', out, table + es * i + 4, self._ref(v))
                    else:
                        q.append((table + es * i + 4, v))
            elif isinstance(node, Arr):
                out.extend(node.tag.encode() + struct.pack('<I', len(node.items)))
                table = len(out)
                out.extend(b'\0' * (4 * len(node.items)))
                for i, v in enumerate(node.items):
                    if isinstance(v, Val):
                        struct.pack_into('<I', out, table + 4 * i, self._ref(v))
                    else:
                        q.append((table + 4 * i, v))
            elif isinstance(node, Iar):
                out.extend(b'!iar' + node.raw)
            return start

        root_off = emit(self.root)
        while q:
            slot, node = q.popleft()
            struct.pack_into('<I', out, slot, 0x80000000 | emit(node))
        while len(out) % 4:
            out.append(0)
        vca_off = len(out)
        out.extend(b'!vca' + struct.pack('<I', len(self.vca)))
        table = len(out)
        out.extend(b'\0' * (4 * len(self.vca)))
        for i, dt in enumerate(self.vca):
            al = ALIGN.get(dt.type, 4)
            while len(out) % al:
                out.append(0)
            struct.pack_into('<I', out, table + 4 * i, 0x80000000 | len(out))
            if dt.type == 'stri':
                out.extend(b'!sbi' + struct.pack('<I', len(dt.raw)) + dt.raw)
            else:
                out.extend(b'!vbi' + dt.type.encode() + struct.pack('<II', dt.count(), dt.bpd) + dt.raw)
        while len(out) % 4:
            out.append(0)
        struct.pack_into('<II', out, 8, 0x80000000 | root_off, 0x80000000 | vca_off)
        return bytes(out)


def to_py(node, typed=False):
    if isinstance(node, Val):
        return f'{node.type} {node.value}' if typed else node.value
    if isinstance(node, Obj):
        o = {}
        for k, v in node.items:
            key = k.value
            n = 2
            while key in o:
                key = f'{k.value}#{n}'
                n += 1
            o[key] = to_py(v, typed)
        return o
    if isinstance(node, Arr):
        return [to_py(v, typed) for v in node.items]
    if isinstance(node, Iar):
        return '!iar ' + node.raw.hex()
    return repr(node)


def fourcc(v):
    b = struct.pack('<I', v)
    return b.decode() if all(65 <= c <= 90 or 97 <= c <= 122 or 48 <= c <= 57 or c == 95 for c in b) else None


def fmt(k, v):
    if v.type == 'stri':
        return f'"{v.value}"'
    if v.type == 'ui32' and k in ('type',) and fourcc(v.value):
        return f"'{fourcc(v.value)}'"
    return f'{v.value!r} <{v.type}>'


def text(node, ind=0, out=None):
    out = [] if out is None else out
    p = '  ' * ind
    if isinstance(node, Obj):
        for k, v in node.items:
            if isinstance(v, Val):
                out.append(f'{p}{k.value}: {fmt(k.value, v)}')
            else:
                out.append(f'{p}{k.value}:' + (' []' if isinstance(v, Arr) and not v.items else ''))
                text(v, ind + 1, out)
    elif isinstance(node, Arr):
        for i, v in enumerate(node.items):
            if isinstance(v, Val):
                out.append(f'{p}- {fmt(None, v)}')
            else:
                out.append(f'{p}- [{i}]')
                text(v, ind + 1, out)
    elif isinstance(node, Iar):
        out.append(f'{p}!iar {node.raw.hex()}')
    return out


if __name__ == '__main__':
    cmd = sys.argv[1]
    if cmd == 'text':
        p = Pkg(open(sys.argv[2], 'rb').read())
        print('\n'.join(text(p.root)))
    elif cmd == 'json':
        p = Pkg(open(sys.argv[2], 'rb').read())
        print(json.dumps(to_py(p.root, typed='-t' in sys.argv), indent=1, ensure_ascii=False))
    elif cmd == 'roundtrip':
        bad = 0
        for f in sys.argv[2:]:
            d = open(f, 'rb').read()
            if d[:4] != b'!pkg':
                continue
            try:
                r = Pkg(d).to_bytes()
            except Exception as e:
                print('ERR', f, e); bad += 1; continue
            if r != d:
                i = next((i for i in range(min(len(r), len(d))) if r[i] != d[i]), min(len(r), len(d)))
                print('DIFF', f, len(d), len(r), hex(i)); bad += 1
        print('bad', bad)
