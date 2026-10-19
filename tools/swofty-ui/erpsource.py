import os
import subprocess
import sys
import threading
from collections import OrderedDict

TOOLS = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if TOOLS not in sys.path:
    sys.path.insert(0, TOOLS)

from erp import parse as erp_parse

try:
    from compression import zstd as _zstd
except ImportError:
    _zstd = None

try:
    import zstandard as _zstandard
except ImportError:
    _zstandard = None

EAID_PREFIX = 'eaid://ui_package/'
ZSTD = 0x11
STORED = 0x91
ZSTD_MAGIC = b'\x28\xb5\x2f\xfd'
ASSET_KINDS = {'GfxMaterialRes', 'GfxMatBindRes', 'GfxSRVResource', 'GfxSurfaceRes'}


class NoDecoder(Exception):
    pass


def _zstd_cli():
    try:
        return subprocess.run(['zstd', '-V'], capture_output=True, check=False).returncode == 0
    except OSError:
        return False


def decoder():
    if _zstd is not None:
        return 'compression.zstd'
    if _zstandard is not None:
        return 'zstandard'
    if _zstd_cli():
        return 'zstd command'
    return None


def decompress(raw):
    if _zstd is not None:
        return _zstd.decompress(raw)
    if _zstandard is not None:
        return _zstandard.ZstdDecompressor().decompressobj().decompress(raw)
    try:
        r = subprocess.run(['zstd', '-dc'], input=raw, capture_output=True, check=False)
    except OSError:
        r = None
    if r is None or r.returncode != 0:
        raise NoDecoder('ERP fragments are zstd-compressed: run Python 3.14 or newer, '
                        'or install the zstandard module or the zstd command')
    return r.stdout


class ErpCatalog:
    def __init__(self, ui_dir):
        self.ui_dir = ui_dir
        self.erps = []
        self._erp_set = set()
        self.scenes = {}
        self.assets = {}
        self.material_names = {}
        self._toc = OrderedDict()
        self._lock = threading.Lock()

    @staticmethod
    def _erps_in(folder):
        return [os.path.join(folder, n) for n in sorted(os.listdir(folder))
                if n.lower().endswith('.erp') and os.path.isfile(os.path.join(folder, n))]

    def build(self):
        files = self._erps_in(self.ui_dir)
        for sub in sorted(os.listdir(self.ui_dir)):
            full = os.path.join(self.ui_dir, sub)
            if os.path.isdir(full) and not sub.startswith('texture'):
                files += self._erps_in(full)
        for path in files:
            try:
                _meta, res = erp_parse(path)
            except Exception:
                continue
            idx = len(self.erps)
            self.erps.append(os.path.relpath(path, self.ui_dir).replace(os.sep, '/'))
            self._erp_set.add(self.erps[-1])
            for name, kind, _unk, frags, _h in res:
                if kind == 'UILayout':
                    self.scenes.setdefault(name, []).append(idx)
                elif kind in ASSET_KINDS:
                    self.assets.setdefault(name, []).append(idx)
                    if kind == 'GfxMaterialRes':
                        self.material_names.setdefault(name.rsplit('/', 1)[-1], []).append(name)
        return self

    def locate(self, name, prefer=None):
        out = [prefer] if prefer else []
        for i in self.assets.get(name, []) + self.scenes.get(name, []):
            rel = self.erps[i]
            if rel not in out:
                out.append(rel)
        return out

    def material_by_basename(self, base, prefer=None):
        names = self.material_names.get(base) or []
        if prefer:
            for n in names:
                if prefer in self.locate(n)[1:]:
                    return n
        return names[0] if names else None

    def erp_path(self, rel):
        if rel not in self._erp_set:
            raise KeyError(f'{rel} is not a UI archive')
        return os.path.join(self.ui_dir, *rel.split('/'))

    def toc(self, rel):
        with self._lock:
            if rel in self._toc:
                self._toc.move_to_end(rel)
                return self._toc[rel]
        meta, res = erp_parse(self.erp_path(rel))
        by_name = {r[0]: r for r in res}
        with self._lock:
            self._toc[rel] = (meta, res, by_name)
            while len(self._toc) > 6:
                self._toc.popitem(last=False)
        return self._toc[rel]

    def read_fragment(self, rel, name, frag=0):
        meta, _res, by_name = self.toc(rel)
        r = by_name.get(name)
        if r is None:
            raise KeyError(f'{name} not in {rel}')
        frags = r[3]
        if frag >= len(frags):
            raise KeyError(f'{name} has {len(frags)} fragment(s), asked for {frag}')
        _fn, off, _size, _flags, comp, packed = frags[frag]
        with open(self.erp_path(rel), 'rb') as f:
            f.seek(meta['data_off'] + off)
            raw = f.read(packed)
        if comp == ZSTD or (comp != STORED and raw[:4] == ZSTD_MAGIC):
            return decompress(raw)
        return raw

    def erps_for(self, eaid):
        return [self.erps[i] for i in self.scenes.get(eaid, [])]

    def read_scene(self, eaid, prefer=None):
        erps = self.erps_for(eaid)
        if not erps:
            raise KeyError(f'no UILayout named {eaid}')
        rel = prefer if prefer in erps else erps[0]
        return rel, self.read_fragment(rel, eaid, 0)

    def resources_with_prefix(self, rel, prefix, limit=200):
        _meta, res, _by = self.toc(rel)
        out = []
        for name, kind, _unk, frags, _h in res:
            if name.startswith(prefix):
                out.append({'name': name, 'kind': kind,
                            'fragments': [{'name': f[0], 'size': f[2], 'packed': f[5]} for f in frags]})
                if len(out) >= limit:
                    break
        return out

    def catalog_json(self):
        rows = []
        for name in sorted(self.scenes):
            short = name.removeprefix(EAID_PREFIX)
            rows.append([short, self.scenes[name]])
        return {'uiDir': self.ui_dir, 'prefix': EAID_PREFIX, 'erps': self.erps, 'scenes': rows}
