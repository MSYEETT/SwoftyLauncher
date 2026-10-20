"""Build the Tyres department frames from icons/tyres.svg and the stock texture arrays.

Every department frame is one slice of a BC7 texture array, and its name is the entry at
the same index in an `overrides.uidb` list. This script appends one slice to each R&D
array the Tyres surfaces read, in every UI archive that carries it, writes the three grown
fragments of each array into icons/frames/, and writes erp/rnd_tyres_icon.json: the
fragment replacements plus a patch-pkg that appends the matching `overrides.uidb` entries.

It compares the build with the shipped icons/frames/ and erp/rnd_tyres_icon.json and exits
1 on any difference. --write replaces the shipped files instead.
"""
import argparse
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

import bc7enc
import common
from pkg import Pkg

DATABASES = 'databases.erp'
OVERRIDES = 'eaid://ui_package/databases/overrides.uidb'
ARRAYS = 'eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.career.'
SVG = os.path.join(common.ADDON, 'icons', 'tyres.svg')
FRAMES = 'icons/frames'
OPS = 'erp/rnd_tyres_icon.json'

WHITE = (255, 255, 255)
TYRES_ORANGE = (255, 120, 0)

ATLASES = [
    ('rnd_dept_icons_small', 'tyres', 'ui_package/art/textures/career/rnd_tree/rnd_dept_icons_small/tyres.tif',
     'svg', WHITE),
    ('rnd_wheel_icon', 'tyres', 'ui_package/art/textures/career/rnd_tree/rnd_wheel_icon/tyres.tif', 'svg',
     TYRES_ORANGE),
    ('rnd_department_strips', 'rnd_tyres_strip',
     'ui_package/art/textures/career/laptop/rnd_colour_strip/rnd_tyres_strip.tif', 'solid', TYRES_ORANGE),
]
PREFIXES = ['', 'rnd_om_xm.']


def resvg():
    for d in os.environ.get('PATH', '').split(os.pathsep):
        p = os.path.join(d, 'resvg')
        if os.access(p, os.X_OK):
            return [p]
    return ['nix', 'run', 'nixpkgs#resvg', '--']


def png_rgba(data):
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    pos, idat, w = 8, b'', 0
    while pos < len(data):
        n, kind = struct.unpack_from('>I4s', data, pos)
        body = data[pos + 8:pos + 8 + n]
        if kind == b'IHDR':
            w, h, depth, ctype, _c, _f, inter = struct.unpack('>IIBBBBB', body)
            if (depth, ctype, inter) != (8, 6, 0):
                sys.exit(f'resvg wrote an unexpected PNG (depth {depth}, colour type {ctype}, interlace {inter})')
        elif kind == b'IDAT':
            idat += body
        pos += 12 + n
    raw, stride, out, prev = zlib.decompress(idat), w * 4, bytearray(), bytearray(w * 4)
    for y in range(h):
        f, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            b = prev[i]
            c = prev[i - 4] if i >= 4 else 0
            if f == 1:
                line[i] = (line[i] + a) & 255
            elif f == 2:
                line[i] = (line[i] + b) & 255
            elif f == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        out += line
        prev = line
    return w, h, bytes(out)


def rasterise(w, h, rgb):
    with tempfile.TemporaryDirectory() as d:
        dst = os.path.join(d, 'icon.png')
        subprocess.run(resvg() + ['-w', str(w), '-h', str(h), SVG, dst], check=True, capture_output=True)
        with open(dst, 'rb') as f:
            pw, ph, px = png_rgba(f.read())
    assert (pw, ph) == (w, h), (pw, ph, w, h)
    out = bytearray()
    for i in range(w * h):
        out += bytes(rgb) + px[4 * i + 3:4 * i + 4]
    return bytes(out)


def solid(w, h, rgb):
    return (bytes(rgb) + b'\xff') * (w * h)


def overrides_lists(db):
    root = Pkg(db.read(OVERRIDES)).root
    career = root['overrides']['Contents']['career']['Contents']
    lists = {}
    for prefix in PREFIXES:
        node = career['rnd_om_xm']['Contents'] if prefix else career
        for name, *_ in ATLASES:
            lists[prefix + name] = [e['Name'].value for e in node[name]['Contents'].items]
    return lists


def build(game, out):
    lists = overrides_lists(common.stock(game, DATABASES))
    ops, edits, report = [], [], []
    for name, frame, tif, kind, rgb in ATLASES:
        pixels = None
        for prefix in PREFIXES:
            lst = prefix + name
            view_name = f'{ARRAYS}{lst}.texarray'
            image_name = view_name + '.image'
            frags = None
            for erp, arc in common.carriers(game, image_name):
                got = (arc.read(image_name, 0), arc.read(image_name, 1), arc.read(view_name, 0))
                if frags is None:
                    frags = got
                elif got != frags:
                    sys.exit(f'{lst}: {erp} carries a different stock array')
                for i, path in enumerate(['image.f0', 'image.f1', 'texarray.f0']):
                    res = image_name if i < 2 else view_name
                    ops.append({'op': 'replace-resource',
                                'erp': f'2025_asset_groups/ui_package/{erp}', 'resource': res,
                                'fragment': i % 2 if i < 2 else 0, 'source': f'{FRAMES}/{lst}.{path}'})
            if frags is None:
                sys.exit(f'{lst}: no stock archive carries {image_name}')
            head, data, view = frags
            fmt, w, h = struct.unpack_from('<III', head, 8)
            slices = struct.unpack_from('<I', head, 28)[0]
            if fmt != common.BC7_FORMAT or len(data) != slices * w * h or struct.unpack_from('<I', view, 0x14)[0] != slices:
                sys.exit(f'{lst}: unexpected stock array (format {fmt}, {w}x{h}, {slices} slices, {len(data)} bytes)')
            names = lists[lst]
            if len(names) != slices or frame in names:
                sys.exit(f'{lst}: overrides.uidb lists {names} for {slices} slices')
            if pixels is None:
                rgba = rasterise(w, h, rgb) if kind == 'svg' else solid(w, h, rgb)
                pixels = bc7enc.encode(rgba, w, h)
                worst, mean, _ = bc7enc.round_trip(pixels, rgba, w, h)
                report.append(f'{name}.{frame}: {w}x{h} BC7, round trip max {worst}/255 mean {mean:.2f}')
            head2 = bytearray(head)
            struct.pack_into('<I', head2, 28, slices + 1)
            view2 = bytearray(view)
            struct.pack_into('<I', view2, 0x14, slices + 1)
            for path, blob in (('image.f0', bytes(head2)), ('image.f1', data + pixels), ('texarray.f0', bytes(view2))):
                dst = os.path.join(out, FRAMES, f'{lst}.{path}')
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                with open(dst, 'wb') as f:
                    f.write(blob)
            at = 'overrides.Contents.career.Contents.' + ('rnd_om_xm.Contents.' if prefix else '') + f'{name}.Contents'
            edits.append({'op': 'clone-entry', 'at': at, 'src': f'[Name={names[-1]}]', 'as': frame,
                          'set': {'Value': tif}})
            report.append(f'{lst}: slice {slices} = {frame}')
    ops.append({'op': 'patch-pkg', 'erp': f'2025_asset_groups/ui_package/{DATABASES}',
                'resource': OVERRIDES, 'fragment': 0, 'edits': edits})
    with open(os.path.join(out, OPS), 'w') as f:
        json.dump({'schema_version': 1, 'ops': ops}, f, indent=2)
        f.write('\n')
    return report


def files(root):
    found = {OPS} if os.path.isfile(os.path.join(root, OPS)) else set()
    for d, _dirs, names in os.walk(os.path.join(root, FRAMES)):
        found |= {os.path.relpath(os.path.join(d, n), root) for n in names}
    return found


def main():
    ap = argparse.ArgumentParser(description='Build the Tyres department frames and erp/rnd_tyres_icon.json.')
    common.add_game_arg(ap)
    ap.add_argument('--work', default=os.path.join(common.BUILD, 'icons'), help='work folder (default: build/icons)')
    ap.add_argument('--write', action='store_true', help='replace the shipped icons/frames/ and erp/rnd_tyres_icon.json')
    a = ap.parse_args()
    shutil.rmtree(a.work, ignore_errors=True)
    os.makedirs(os.path.join(a.work, 'erp'))
    for line in build(a.game, a.work):
        print(line)
    if a.write:
        shutil.rmtree(os.path.join(common.ADDON, FRAMES), ignore_errors=True)
        shutil.copytree(os.path.join(a.work, FRAMES), os.path.join(common.ADDON, FRAMES))
        shutil.copyfile(os.path.join(a.work, OPS), os.path.join(common.ADDON, OPS))
        print(f'wrote {FRAMES}/ and {OPS}')
        return 0
    built, shipped = files(a.work), files(common.ADDON)
    differ = [r for r in sorted(built & shipped)
              if open(os.path.join(a.work, r), 'rb').read() != open(os.path.join(common.ADDON, r), 'rb').read()]
    missing, extra = sorted(built - shipped), sorted(shipped - built)
    print(f'{len(built & shipped) - len(differ)} same, {len(differ)} differ, {len(missing)} not shipped, '
          f'{len(extra)} only shipped')
    for r in differ + missing + extra:
        print('  ', r)
    return 1 if differ or missing or extra else 0


if __name__ == '__main__':
    sys.exit(main())
