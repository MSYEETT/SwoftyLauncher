"""GfxMaterialRes (.material) reader/writer for F1 25 and F1 23, and an F1 23 -> F1 25 converter.

Format notes: re/db/subsystems/gfx_material.md.

  python3 tools/gfx_material.py dump <file.erp> <resource-name-substring>
  python3 tools/gfx_material.py catalog <F1 25>/2025_asset_groups [-o tools/gfx_material_layouts.json]
  python3 tools/gfx_material.py convert <f1_23.erp> <out_dir>          (one F1 25 .material file per material)
  python3 tools/gfx_material.py convert-erp <f1_23.erp> <out.erp> [--drop-samplers]
"""
import json
import os
import struct
import sys
from collections import Counter
from dataclasses import dataclass, field

CONST, TEXTURE, SAMPLER = 1, 2, 3
HEAP = (6, 3)
LAYOUTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'gfx_material_layouts.json')


@dataclass
class Param:
    name: str
    kind: int
    value: object
    type: int = 0
    cls: int = 0
    cbuffer: int = 0
    offset: int = 0
    link: str = None

    @property
    def heap(self):
        return self.kind == CONST and (self.type, self.cls) == HEAP


@dataclass
class Material:
    game: int
    shader: str
    params: list
    cbuffers: list = field(default_factory=list)

    def get(self, name):
        return next((p for p in self.params if p.name == name), None)

    def constants(self):
        return [p for p in self.params if p.kind == CONST]

    def textures(self):
        return {p.name: p.value for p in self.params if p.kind == TEXTURE}

    def samplers(self):
        return {p.name: p.value for p in self.params if p.kind == SAMPLER}


@dataclass
class SamplerState:
    filter: int
    address_u: int
    address_v: int
    address_w: int
    lod_bias: float
    max_anisotropy: int
    unk6: int
    unk7: int


def sort_key(name):
    return name.lower().replace('_', '\x01')


def _cstr(b, off):
    end = b.index(b'\0', off)
    return b[off:end].decode()


class _Pool:
    def __init__(self, dedupe):
        self.buf = bytearray()
        self.seen = {}
        self.dedupe = dedupe

    def add(self, s):
        if self.dedupe and s in self.seen:
            return self.seen[s]
        off = len(self.buf)
        self.seen.setdefault(s, off)
        self.buf += s.encode() + b'\0'
        return off


def parse25(b, strict=True):
    if len(b) < 32:
        raise ValueError('too short')
    n, ncb, vsz, o_sh, o_nm, o_t1, o_t2, o_cb = struct.unpack_from('<8I', b, 0)
    base = 0x20 + 36 * n
    if base + o_cb > len(b) or not (vsz + 8 * ncb == o_sh <= o_nm <= o_t1 <= o_t2 <= o_cb):
        raise ValueError('bad header')
    recs = [struct.unpack_from('<9I', b, 0x20 + 36 * i) for i in range(n)]
    cbt = struct.unpack_from('<%dI' % (2 * ncb), b, base + vsz)
    cbuffers = [[_cstr(b, base + o_cb + cbt[2 * i]), cbt[2 * i + 1]] for i in range(ncb)]
    params = []
    for nm, kind, typ, cls, size, cb, off, at, link in recs:
        name = _cstr(b, base + o_nm + nm)
        if kind == CONST:
            if at + size > vsz:
                raise ValueError(f'{name}: value outside blob')
            p = Param(name, kind, b[base + at:base + at + size], typ, cls, cb, off)
            if (typ, cls) == HEAP:
                p.link = _cstr(b, base + o_t2 + link)
            elif link:
                raise ValueError(f'{name}: link on a non-heap constant')
        elif kind == TEXTURE:
            p = Param(name, kind, _cstr(b, base + o_t1 + at), typ, cls, cb, off)
            if size != len(p.value) + 1 or link:
                raise ValueError(f'{name}: bad texture record')
        else:
            raise ValueError(f'{name}: unknown kind {kind}')
        params.append(p)
    m = Material(25, _cstr(b, base + o_sh), params, cbuffers)
    if strict and write25(m) != bytes(b):
        raise ValueError('not canonical: re-encoding differs')
    return m


def write25(m):
    names, list1, list2, cbn = _Pool(True), _Pool(True), _Pool(True), _Pool(True)
    blob = bytearray()
    recs = []
    for p in m.params:
        nm = names.add(p.name)
        if p.kind == CONST:
            v = bytes(p.value)
            link = list2.add(p.link) if p.heap else 0
            recs.append((nm, CONST, p.type, p.cls, len(v), p.cbuffer, p.offset, len(blob), link))
            blob += v
        elif p.kind == TEXTURE:
            at = list1.add(p.value)
            recs.append((nm, TEXTURE, p.type, p.cls, len(p.value) + 1, p.cbuffer, p.offset, at, 0))
        else:
            raise ValueError(f'{p.name}: F1 25 materials have no kind {p.kind} records')
    cbt = b''.join(struct.pack('<II', cbn.add(name), size) for name, size in m.cbuffers)
    shader = m.shader.encode() + b'\0'
    o_sh = len(blob) + len(cbt)
    o_nm = o_sh + len(shader)
    o_t1 = o_nm + len(names.buf)
    o_t2 = o_t1 + len(list1.buf)
    o_cb = o_t2 + len(list2.buf)
    out = bytearray(struct.pack('<8I', len(recs), len(m.cbuffers), len(blob), o_sh, o_nm, o_t1, o_t2, o_cb))
    for r in recs:
        out += struct.pack('<9I', *r)
    out += blob + cbt + shader + names.buf + list1.buf + list2.buf + cbn.buf
    return bytes(out)


def parse23(b, strict=True):
    if len(b) < 16:
        raise ValueError('too short')
    n, vsz, o_nm, o_str = struct.unpack_from('<4I', b, 0)
    base = 0x10 + 16 * n
    if not (vsz < o_nm <= o_str) or base + o_str > len(b):
        raise ValueError('bad header')
    params = []
    for i in range(n):
        nm, kind, size, at = struct.unpack_from('<4I', b, 0x10 + 16 * i)
        name = _cstr(b, base + o_nm + nm)
        if kind == CONST:
            if at + size > vsz:
                raise ValueError(f'{name}: value outside blob')
            params.append(Param(name, kind, b[base + at:base + at + size]))
        elif kind in (TEXTURE, SAMPLER):
            s = _cstr(b, base + o_str + at)
            if size != len(s) + 1:
                raise ValueError(f'{name}: bad string record')
            params.append(Param(name, kind, s))
        else:
            raise ValueError(f'{name}: unknown kind {kind}')
    m = Material(23, _cstr(b, base + vsz), params)
    if strict and write23(m) != bytes(b):
        raise ValueError('not canonical: re-encoding differs')
    return m


def write23(m):
    names, strs = _Pool(True), _Pool(True)
    blob = bytearray()
    recs = []
    for p in m.params:
        nm = names.add(p.name)
        if p.kind == CONST:
            recs.append((nm, CONST, len(p.value), len(blob)))
            blob += bytes(p.value)
        elif p.kind in (TEXTURE, SAMPLER):
            recs.append((nm, p.kind, len(p.value) + 1, strs.add(p.value)))
        else:
            raise ValueError(f'{p.name}: unknown kind {p.kind}')
    shader = m.shader.encode() + b'\0'
    o_nm = len(blob) + len(shader)
    out = bytearray(struct.pack('<4I', len(recs), len(blob), o_nm, o_nm + len(names.buf)))
    for r in recs:
        out += struct.pack('<4I', *r)
    out += blob + shader + names.buf + strs.buf
    return bytes(out)


def parse_sampler(b):
    if len(b) != 32:
        raise ValueError('GfxSampleState is 32 bytes')
    f = struct.unpack('<4IfIII', b)
    return SamplerState(*f)


def write_sampler(s):
    return struct.pack('<4IfIII', s.filter, s.address_u, s.address_v, s.address_w, s.lod_bias,
                       s.max_anisotropy, s.unk6, s.unk7)


def layout_of(m):
    return {
        'cbuffers': [list(c) for c in m.cbuffers],
        'params': [[p.name, p.kind, p.type, p.cls, len(p.value) if p.kind == CONST else 0, p.cbuffer, p.offset]
                   for p in m.params],
    }


def build_catalog(materials):
    shapes, values = {}, {}
    for m in materials:
        lay = layout_of(m)
        key = json.dumps(lay)
        shapes.setdefault(m.shader, Counter())[key] += 1
        vals = values.setdefault((m.shader, key), {})
        for p in m.params:
            v = p.value.hex() if p.kind == CONST else p.value
            vals.setdefault(p.name, Counter())[v] += 1
    out, conflicts = {}, {}
    for shader, c in sorted(shapes.items()):
        key, count = c.most_common(1)[0]
        if len(c) > 1:
            conflicts[shader] = len(c)
        lay = json.loads(key)
        vals = values[(shader, key)]
        lay['defaults'] = {name: vals[name].most_common(1)[0][0] for name, *_ in lay['params']}
        lay['materials'] = count
        out[shader] = lay
    return out, conflicts


def save_catalog(cat, path):
    with open(path, 'w') as f:
        f.write('{"source": "F1 25 GfxMaterialRes: most common layout per shader", "shaders": {\n')
        rows = [json.dumps(k) + ': ' + json.dumps(v, sort_keys=True, separators=(',', ':')) for k, v in sorted(cat.items())]
        f.write(',\n'.join(rows))
        f.write('\n}}\n')


_catalog = None


def catalog():
    global _catalog
    if _catalog is None:
        with open(LAYOUTS) as f:
            _catalog = json.load(f)['shaders']
    return _catalog


def _as_sampler(v):
    if isinstance(v, SamplerState):
        return v
    return parse_sampler(v)


def plan23to25(m23, samplers=None, layout=None):
    if isinstance(m23, (bytes, bytearray)):
        m23 = parse23(m23)
    if m23.game != 23:
        raise ValueError('expected an F1 23 material')
    if layout is None:
        layout = catalog().get(m23.shader)
        if layout is None:
            raise KeyError(f'no F1 25 layout for shader {m23.shader}')
    consts = {p.name: p.value for p in m23.params if p.kind == CONST}
    texs = m23.textures()
    notes = {'defaulted': [], 'dropped': [], 'samplers': {}}
    params = []
    defaults = layout['defaults']
    for name, kind, typ, cls, size, cb, off in layout['params']:
        if kind == CONST and (typ, cls) == HEAP:
            slot = name[:-len('HeapIndex')]
            link = texs.get(slot)
            if link is None:
                link = defaults[slot]
                notes['defaulted'].append(slot)
            params.append(Param(name, kind, bytes(size), typ, cls, cb, off, link))
        elif kind == CONST:
            v = consts.get(name)
            if v is None or len(v) != size:
                v = bytes.fromhex(defaults[name])
                notes['defaulted'].append(name)
            params.append(Param(name, kind, v, typ, cls, cb, off))
        else:
            v = texs.get(name)
            if v is None:
                v = defaults[name]
                notes['defaulted'].append(name)
            params.append(Param(name, kind, v, typ, cls, cb, off))
    used = {p.name for p in params}
    notes['dropped'] = [p.name for p in m23.params if p.kind != SAMPLER and p.name not in used]
    samplers = samplers or {}
    for p in m23.params:
        if p.kind == SAMPLER:
            s = samplers.get(p.value)
            notes['samplers'][p.name] = _as_sampler(s) if s is not None else None
    m25 = Material(25, m23.shader, params, [list(c) for c in layout['cbuffers']])
    return m25, notes


def convert23to25(m23, samplers=None, layout=None):
    return write25(plan23to25(m23, samplers, layout)[0])


def _erp():
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import erpw
    try:
        from compression import zstd
        erpw.zstd_d = zstd.decompress
    except ImportError:
        pass
    return erpw


def _type(r):
    return r.type.rstrip(b'\0').decode()


def _main(argv):
    if not argv:
        print(__doc__)
        return 2
    cmd = argv[0]
    if cmd == 'dump':
        erpw = _erp()
        e = erpw.Erp(argv[1])
        for r in e.res:
            if _type(r) == 'GfxMaterialRes' and argv[2] in r.name:
                b = e.frag_data(r, 0)
                try:
                    m = parse25(b)
                except ValueError:
                    m = parse23(b)
                print(r.name, f'F1 {m.game}', m.shader, m.cbuffers)
                for p in m.params:
                    v = p.value.hex() if p.kind == CONST else p.value
                    extra = f' -> {p.link}' if p.link else ''
                    head = f'{p.type:#x}/{p.cls} off={p.offset:<4} ' if m.game == 25 else ''
                    print(f'  {p.kind} {head}{p.name} = {v}{extra}')
        return 0
    if cmd == 'catalog':
        erpw = _erp()
        root = argv[1]
        out = argv[argv.index('-o') + 1] if '-o' in argv else LAYOUTS
        mats, seen = [], set()
        for dp, _dn, fn in os.walk(root):
            for x in sorted(fn):
                if not x.endswith('.erp'):
                    continue
                e = erpw.Erp(os.path.join(dp, x))
                for r in e.res:
                    if _type(r) == 'GfxMaterialRes':
                        b = e.frag_data(r, 0)
                        if b not in seen:
                            seen.add(b)
                            mats.append(parse25(b))
        cat, conflicts = build_catalog(mats)
        save_catalog(cat, out)
        print(len(mats), 'unique materials,', len(cat), 'shaders,', len(conflicts), 'with more than one layout')
        return 0
    if cmd in ('convert', 'convert-erp'):
        erpw = _erp()
        e = erpw.Erp(argv[1])
        samplers = {r.name: e.frag_data(r, 0) for r in e.res if _type(r) == 'GfxSampleState'}
        if cmd == 'convert':
            os.makedirs(argv[2], exist_ok=True)
        for r in e.res:
            if _type(r) != 'GfxMaterialRes':
                continue
            m25, notes = plan23to25(parse23(e.frag_data(r, 0)), samplers)
            if cmd == 'convert':
                with open(os.path.join(argv[2], r.name.rsplit('/', 1)[1]), 'wb') as f:
                    f.write(write25(m25))
            else:
                e.set_frag(r, 0, write25(m25))
            print(r.name, 'defaulted', notes['defaulted'], 'dropped', notes['dropped'])
        if cmd == 'convert-erp':
            if '--drop-samplers' in argv:
                e.res = [r for r in e.res if _type(r) != 'GfxSampleState']
            e.to_file(argv[2])
        return 0
    print(__doc__)
    return 2


if __name__ == '__main__':
    sys.exit(_main(sys.argv[1:]))
