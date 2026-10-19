"""Resolve an .idf geom to drawable data: quad mesh, material colour and BC7 texture.

Chain (all resources live in the UI ERPs):
  geom.resourceid  <elem>?context=render     -> <...>&lod=0&mesh   (GfxMeshRes: vb/ib names + material slot)
                                                <...>&lod=0&vb=N    (GfxBufferRes: hdr frag0, data frag1)
  geom.materialid  <scene>.emb?context=default (GfxMatBindRes: slot -> .material)
  .material        GfxMaterialRes: shader, cbuffer params (me_colour, ...), texture .tif refs
  .tif             GfxSRVResource -> .tif.image (GfxSurfaceRes: hdr frag0, top mip frag1, BC7)
"""
import re
import struct
import threading

STRINGS = re.compile(rb'[\x20-\x7e]{2,}')
BC7_TYPES = {70}
FLOAT4 = 0x00040003


def _strings(data):
    return [m.group().decode() for m in STRINGS.finditer(data)]


def _cstr(data, off):
    end = data.index(b'\0', off)
    return data[off:end].decode('utf-8', 'replace')


def parse_matbind(data):
    n_slots, _n_mats, mat_base = struct.unpack_from('<III', data, 0)
    table = 12
    strings = table + 8 * n_slots
    out = {}
    for i in range(n_slots):
        name_off, mat_off = struct.unpack_from('<II', data, table + 8 * i)
        out[_cstr(data, strings + name_off)] = _cstr(data, table + mat_base + mat_off)
    return out


def parse_material(data):
    strs = _strings(data)
    shader = next((s for s in strs if s.endswith('.nefx2')), None)
    names = [s for s in strs if s.startswith('me_')]
    textures = []
    for s in strs:
        if s.startswith('eaid://') and ('.tif' in s or '.texarray' in s) and s not in textures:
            textures.append(s)
    params = {}
    try:
        str_start = data.find(b'eaid://shader_package')
        if str_start < 12:
            raise ValueError('no shader string')
        cb_size = struct.unpack_from('<I', data, str_start - 4)[0]
        cb_start = str_start - 8 - cb_size
        if cb_start < 0x24:
            raise ValueError('no cbuffer')
        cb = data[cb_start:str_start - 8]
        entries = []
        off = 0x24
        while off + 28 <= cb_start:
            f = struct.unpack_from('<7I', data, off)
            if f[0] != 1:
                break
            entries.append(f)
            off += 36
        cb_names = [n for n in names if not n.endswith('Tex')]
        for f, name in zip(entries, cb_names):
            size, at = f[3], f[6]
            if at + size > len(cb):
                continue
            if f[1] == FLOAT4 or size == 16:
                params[name] = [round(x, 5) for x in struct.unpack_from('<4f', cb, at)]
            elif size == 4:
                if f[1] == 3:
                    params[name] = round(struct.unpack_from('<f', cb, at)[0], 5)
                else:
                    params[name] = struct.unpack_from('<I', cb, at)[0]
    except (struct.error, ValueError):
        pass
    return {'shader': shader.rsplit('/', 1)[-1] if shader else None, 'params': params, 'textures': textures}


def parse_mesh(data):
    strs = _strings(data)
    vbs = sorted((s for s in strs if '&vb=' in s), key=lambda s: int(s.rsplit('=', 1)[1]))
    ib = next((s for s in strs if '&ib=' in s), None)
    slot = next((s for s in reversed(strs) if not s.startswith('eaid://')), None)
    return {'vbs': vbs, 'ib': ib, 'slot': slot}


def parse_buffer(hdr, data):
    _kind, stride, count, _flags = struct.unpack_from('<4I', hdr, 0)
    return stride, count, data


def parse_surface(hdr):
    f = struct.unpack_from('<8I', hdr, 0)
    return {'format': f[2], 'w': f[3], 'h': f[4], 'mips': f[6], 'array': max(f[7], 1)}


class AssetResolver:
    def __init__(self, catalog):
        self.catalog = catalog
        self.cache = {}
        self.lock = threading.Lock()

    def _candidates(self, name, prefer):
        out = self.catalog.locate(name, prefer)
        for sep in ('.idf/', '.idf?'):
            if sep in name:
                scene = name.split(sep, 1)[0] + '.idf'
                out += [r for r in self.catalog.erps_for(scene) if r not in out]
                break
        return out

    def _read(self, name, frag=0, prefer=None):
        for rel in self._candidates(name, prefer):
            try:
                return rel, self.catalog.read_fragment(rel, name, frag)
            except KeyError:
                continue
        raise KeyError(name)

    def _cached(self, key, fn):
        with self.lock:
            if key in self.cache:
                return self.cache[key]
        try:
            val = fn()
        except Exception as ex:
            val = {'error': f'{type(ex).__name__}: {ex}'}
        with self.lock:
            self.cache[key] = val
        return val

    def material(self, path, prefer=None):
        def load():
            rel, data = self._read(path, 0, prefer)
            m = parse_material(data)
            m['textures'] = [self.texture_image(t, rel) for t in m['textures']]
            m['textures'] = [t for t in m['textures'] if t]
            return m
        return self._cached(('mat', path), load)

    def texture_image(self, tif, prefer=None):
        if tif.endswith('.image'):
            return tif
        try:
            _rel, srv = self._read(tif, 0, prefer)
        except KeyError:
            return tif + '.image'
        return next((s for s in _strings(srv) if s.endswith('.image')), tif + '.image')

    def matbind(self, path, prefer=None):
        return self._cached(('emb', path), lambda: parse_matbind(self._read(path, 0, prefer)[1]))

    def resolve_slot(self, materialid, slot, prefer=None):
        binds = self.matbind(materialid, prefer) if materialid else {}
        mat = binds.get(slot) if isinstance(binds, dict) and 'error' not in binds else None
        if mat is None and isinstance(binds, dict) and len(binds) == 1 and 'error' not in binds:
            mat = next(iter(binds.values()))
        if mat and mat.endswith('uses_anim_mat_library.material'):
            alt = self.catalog.material_by_basename(slot + '.material', prefer)
            mat = alt or mat
        if mat is None and slot:
            mat = self.catalog.material_by_basename(slot + '.material', prefer)
        return mat

    def geom(self, resourceid, materialid, prefer=None):
        def load():
            mesh_name = resourceid + '&lod=0&mesh'
            rel, mesh_data = self._read(mesh_name, 0, prefer)
            mesh = parse_mesh(mesh_data)
            bufs = []
            for vb in mesh['vbs']:
                _r, hdr = self._read(vb, 0, rel)
                _r, data = self._read(vb, 1, rel)
                bufs.append(parse_buffer(hdr, data))
            if not bufs:
                raise ValueError('mesh has no vertex buffers')
            stride, count, data = bufs[0]
            if stride < 12:
                raise ValueError(f'unexpected position stride {stride}')
            verts = [list(struct.unpack_from('<3f', data, i * stride)) for i in range(count)]
            uv = None
            for st, cnt, d in bufs[1:]:
                if st == 8 and cnt == count:
                    uv = [list(struct.unpack_from('<2f', d, i * 8)) for i in range(cnt)]
                    break
            idx = list(range(count))
            if mesh['ib']:
                _r, ihdr = self._read(mesh['ib'], 0, rel)
                _r, idata = self._read(mesh['ib'], 1, rel)
                isz = struct.unpack_from('<I', ihdr, 4)[0]
                fmt = '<%dH' if isz == 2 else '<%dI'
                idx = list(struct.unpack_from(fmt % (len(idata) // isz), idata, 0))
            mat_path = self.resolve_slot(materialid, mesh['slot'], rel)
            mat = self.material(mat_path, rel) if mat_path else {'error': 'no material'}
            out = {'verts': [[round(v, 5) for v in p] for p in verts], 'uv': uv, 'idx': idx,
                   'slot': mesh['slot'], 'material': mat_path, 'erp': rel}
            if 'error' in mat:
                out['matError'] = mat['error']
            else:
                out['shader'] = mat['shader']
                out['params'] = mat['params']
                out['textures'] = mat['textures']
            return out
        return self._cached(('geom', resourceid, materialid), load)

    def surface(self, image, prefer=None):
        def load():
            rel, hdr = self._read(image, 0, prefer)
            info = parse_surface(hdr)
            info['erp'] = rel
            return info
        return self._cached(('surf', image), load)

    def surface_pixels(self, image, prefer=None):
        info = self.surface(image, prefer)
        if 'error' in info:
            raise KeyError(info['error'])
        _rel, data = self._read(image, 1, info.get('erp'))
        return info, data
