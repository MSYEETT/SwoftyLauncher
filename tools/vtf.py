import struct

class Node:
    __slots__ = ('kind', 'pad', 'name', 'attrs', 'children', 'text', 'end')
    def __init__(self, kind, pad=0, name=b'', attrs=None, children=None, text=b'', end=None):
        self.kind = kind; self.pad = pad; self.name = name
        self.attrs = attrs if attrs is not None else []
        self.children = children if children is not None else []
        self.text = text; self.end = end
    def attr(self, k):
        for a, v in self.attrs:
            if a == k: return v
    def set_attr(self, k, v):
        for i, (a, _) in enumerate(self.attrs):
            if a == k: self.attrs[i] = (a, v); return
        raise KeyError(k)
    def find(self, name):
        return [c for c in self.children if c.kind == 0 and c.name == name]
    def clone(self):
        return Node(self.kind, self.pad, self.name, list(self.attrs), [c.clone() for c in self.children], self.text, self.end)

def parse(data):
    if data[:5] != b'\x01BXML':
        raise ValueError('not BXML')
    pos = 5
    def cstr():
        nonlocal pos
        e = data.index(b'\0', pos); s = data[pos:e]; pos = e + 1; return s
    def node():
        nonlocal pos
        start = pos
        ln, typ, pad, nattr = struct.unpack_from('<HBBH', data, pos)
        pos += 6
        if typ == 0:
            n = Node(0, pad, cstr())
            n.attrs = [(cstr(), cstr()) for _ in range(nattr)]
            assert pos - start - 2 == ln, (start, ln, pos)
            while True:
                c = node()
                if c.kind == 5:
                    n.end = c; break
                n.children.append(c)
            return n
        if typ == 1:
            n = Node(1, pad, text=cstr())
            assert nattr == 0 and pos - start - 2 == ln
            return n
        if typ == 5:
            assert ln == 4 and nattr == 0, (start, ln, nattr)
            return Node(5, pad)
        raise ValueError(f'node type {typ} at {start:#x}')
    root = node()
    tail = []
    while pos < len(data):
        ln, typ, pad, nattr = struct.unpack_from('<HBBH', data, pos)
        if typ != 6 or ln != 4 or nattr != 0:
            raise ValueError(f'unexpected trailing record type {typ} at {pos:#x}')
        tail.append(pad); pos += 6
    root.text = bytes(tail)
    return root

def write(root):
    out = bytearray(b'\x01BXML')
    def rec(typ, pad, body, nattr=0):
        out.extend(struct.pack('<HBBH', len(body) + 4, typ, pad, nattr)); out.extend(body)
    def node(n):
        if n.kind == 0:
            body = n.name + b'\0' + b''.join(k + b'\0' + v + b'\0' for k, v in n.attrs)
            rec(0, n.pad, body, len(n.attrs))
            for c in n.children: node(c)
            rec(5, n.end.pad if n.end else 0, b'')
        elif n.kind == 1:
            rec(1, n.pad, n.text + b'\0')
    node(root)
    for pad in root.text:
        rec(6, pad, b'')
    return bytes(out)

def walk(n, depth=0):
    yield n, depth
    for c in n.children:
        yield from walk(c, depth + 1)

def tsmd_stats(root):
    els = [n for n, _ in walk(root) if n.kind == 0]
    texts = [n.text for n, _ in walk(root) if n.kind == 1]
    keys = [k for n in els for k, _ in n.attrs]
    vals = [v for n in els for _, v in n.attrs]
    names = [n.name for n in els]
    cats = [keys, vals, names, texts, names + keys, vals + texts, names + keys + vals + texts]
    out = []
    for c in cats:
        out += [len(c), sum(len(s) for s in c)]
    for c in cats:
        u = set(c)
        out += [len(u), sum(len(s) for s in u)]
    return out

def tsmd(root, version=1):
    return b'TSMD' + struct.pack('<29I', version, *tsmd_stats(root))

ERS_FIELDS = [b'Track', b'MediumDeployFactor', b'MediumDerateFactor', b'HotLapDeployFactor', b'HotLapDerateFactor',
              b'HarvestFactorMGUH', b'OverrideDetectionLapProgress', b'OverrideActivationLapProgress',
              b'HarvestLimitMGUK', b'HarvestLimitQualifyingMGUK', b'HarvestTorqueFactor']

def fmt_value(v):
    if isinstance(v, bool):
        raise ValueError('bool ers value')
    if isinstance(v, int):
        return str(v).encode()
    if isinstance(v, float):
        if v.is_integer() and abs(v) >= 1e6:
            return str(int(v)).encode()
        return repr(v).encode()
    return str(v).encode()

def ers_node(root):
    for c in root.children:
        if c.kind == 0 and c.name == b'EnergyRecoverySystem' and c.find(b'TrackSettings'):
            return c

def track_settings(root):
    ers = ers_node(root)
    out = {}
    for ts in ers.find(b'TrackSettings') if ers else []:
        t = ts.find(b'Track')
        if t:
            out[t[0].attr(b'value').decode()] = ts
    return out

def add_track_settings(root, code, base, values):
    ers = ers_node(root)
    if ers is None:
        return False
    entries = ers.find(b'TrackSettings')
    by = track_settings(root)
    if code in by:
        new = by[code]
    else:
        if base not in by:
            raise KeyError(f'no TrackSettings for {base}')
        new = by[base].clone()
        new.set_attr(b'name', str(len(entries)).encode())
        last = max(i for i, c in enumerate(ers.children) if c.kind == 0 and c.name == b'TrackSettings')
        ers.children.insert(last + 1, new)
        new.find(b'Track')[0].set_attr(b'value', code.encode())
        ers.set_attr(b'numTrackSettingsTypes', str(len(entries) + 1).encode())
    template = None
    for e in entries:
        if e.children:
            template = e.children[1]
            break
    for k, v in values.items():
        kb = k.encode()
        if kb not in ERS_FIELDS[1:]:
            raise KeyError(f'unknown ers key {k}')
        f = new.find(kb)
        if f:
            f[0].set_attr(b'value', fmt_value(v))
            continue
        el = template.clone()
        el.name = kb
        el.attrs = [(b'value', fmt_value(v))]
        el.children = []
        order = ERS_FIELDS.index(kb)
        pos = len(new.children)
        for i, c in enumerate(new.children):
            if c.kind == 0 and c.name in ERS_FIELDS and ERS_FIELDS.index(c.name) > order:
                pos = i
                break
        new.children.insert(pos, el)
    return True
