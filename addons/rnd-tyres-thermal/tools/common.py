"""Paths and stock-archive access shared by the rnd-tyres-thermal asset tools.

The tools live in addons/rnd-tyres-thermal/tools/ and use the repo's tools/ (idf.py,
pkg.py, erp.py, erpw.py, uitree.py, uicheck.py) and tools/swofty-ui/ (gamedir.py,
erpsource.py, assets.py, static/bc7.js). Stock resources are read straight from the
game's UI archives, which must be the stock ones (run `swofty restore` first).
"""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ADDON = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(ADDON))
BUILD = os.path.join(ADDON, 'build')

for _p in (os.path.join(REPO, 'tools', 'swofty-ui'), os.path.join(REPO, 'tools')):
    if _p not in sys.path:
        sys.path.insert(0, _p)

import gamedir
from assets import parse_surface
from erp import parse as erp_parse
from erpsource import STORED, ZSTD, ZSTD_MAGIC, decompress

PKG = 'eaid://ui_package/'
CAREER = 'flow_career_hub.erp'
TEAM_HQ = 'flow_team_hq.erp'
PERSISTENT = 'persistent.erp'
ARCHIVES = [CAREER, TEAM_HQ]
BC7_FORMAT = 70
BC7_DEC = os.path.join(HERE, 'bc7dec.mjs')

UI_SCREEN = PKG + 'screens/laptop_r_and_d.ui'
OVERVIEW = PKG + 'art/screens/career/rnd_overview.idf'
GRAPH_BG = PKG + 'art/textures/team_hq/rnd/dev_potential/graph_bg.tif'
MASKS = {
    'comp_potential': PKG + 'art/textures/career/rnd_tree/dev_potential_chart/rnd_dev_comp_potential.tif',
    'current_nub': PKG + 'art/textures/career/rnd_tree/dev_potential_chart/rnd_dev_current.tif',
    'potential_nub': PKG + 'art/textures/career/rnd_tree/dev_potential_chart/rnd_dev_potential.tif',
}


def add_game_arg(ap):
    ap.add_argument('--game', help='the F1 25 folder, or a folder holding the stock UI archives '
                    '(default: F1_25_DIR, then the launcher setting, then Steam)')


def ui_package(game=None):
    try:
        return gamedir.resolve(game)[0]
    except gamedir.NotFound as ex:
        sys.exit(f'{ex}: pass --game')


def bc7_decode(data, w, h):
    with tempfile.TemporaryDirectory() as d:
        src, dst = os.path.join(d, 'in.bc7'), os.path.join(d, 'out.rgba')
        with open(src, 'wb') as f:
            f.write(data)
        subprocess.run(['node', BC7_DEC, src, str(w), str(h), dst], check=True)
        with open(dst, 'rb') as f:
            return f.read()


class Archive:
    def __init__(self, path):
        self.path = path
        self.meta, res = erp_parse(path)
        self.res = {r[0]: r for r in res}

    def names(self):
        return set(self.res)

    def read(self, name, frag=0):
        r = self.res.get(name)
        if r is None:
            raise KeyError(f'{name} is not in {self.path}')
        _fn, off, _size, _flags, comp, packed = r[3][frag]
        with open(self.path, 'rb') as f:
            f.seek(self.meta['data_off'] + off)
            raw = f.read(packed)
        if comp == ZSTD or (comp != STORED and raw[:4] == ZSTD_MAGIC):
            return decompress(raw)
        return raw

    def texture(self, tif):
        info = parse_surface(self.read(tif + '.image', 0))
        if info['format'] != BC7_FORMAT:
            raise ValueError(f'{tif}: format {info["format"]}, not BC7')
        w, h = info['w'], info['h']
        return w, h, bc7_decode(self.read(tif + '.image', 1)[:w * h], w, h)


def stock(game=None, erp=CAREER):
    return Archive(os.path.join(ui_package(game), erp))


def carriers(game, resource):
    import glob
    ui = ui_package(game)
    out = []
    for path in sorted(glob.glob(os.path.join(ui, '*.erp'))):
        arc = Archive(path)
        if resource in arc.names():
            out.append((os.path.basename(path), arc))
    return out
