"""Build the Tyres upgrade card art and category icons into the stock R&D texture arrays.

art/manifest.json lists four frames per upgrade (available wireframe, in-progress
wireframe, complete render, mask) and one category icon per Tyres upgrade type. Every
frame is one BC7 slice of a texture array, named by the entry at the same index in an
`overrides.uidb` list, as in build_icons.py. This script appends the slices to the 2026
(`rnd_om_xm.`) and the pre-2026 lists, in every UI archive that carries each array,
writes the grown fragments into art/arrays/, and writes erp/rnd_tyres_card_art.json: the
fragment replacements plus one patch-pkg that appends the `overrides.uidb` entries.

Frame names follow each list's stock convention:
  rnd_om_xm.rnd_part_image      om_xm_<name>, om_xm_<name>_progress, om_xm_<name>_complete
  rnd_om_xm.rnd_part_mask       om_xm_<name>_m
  rnd_part_image                <name>, <name>_in_progress, <name>_complete
  rnd_part_mask                 <name>
  [rnd_om_xm.]rnd_category_icons  tyres_<type> (stock: <department>_<type>)

It compares the build with the shipped art/arrays/ and erp/rnd_tyres_card_art.json and
exits 1 on any difference. --write replaces the shipped files instead.
"""
import argparse
import json
import os
import shutil
import struct
import sys

import bc7enc
import build_icons
import build_tree
import common
from pkg import Pkg

ARRAYS = build_icons.ARRAYS
DATABASES = build_icons.DATABASES
OVERRIDES = build_icons.OVERRIDES
MANIFEST = os.path.join(common.ADDON, 'art', 'manifest.json')
OUT = 'art/arrays'
OPS = 'erp/rnd_tyres_card_art.json'

TYPE_KEYS = ['tyre_thermal', 'brake_cooling', 'tyre_life', 'pu_cooling']
STATES = [('available', '', ''), ('in_progress', '_progress', '_in_progress'), ('complete', '_complete', '_complete')]
TILES = 'ui_package/art/textures/tiles/'
CATEGORY_TIF = 'ui_package/art/textures/career/rnd_tree/category_icons/'


def png(path):
    with open(os.path.join(common.ADDON, 'art', path), 'rb') as f:
        return build_icons.png_rgba(f.read())


def fidelity(bc7, rgba, w, h):
    import numpy as np
    dec = np.frombuffer(common.bc7_decode(bc7, w, h), np.uint8).reshape(-1, 4).astype(np.float64)
    src = np.frombuffer(rgba, np.uint8).reshape(-1, 4).astype(np.float64)
    a = src[:, 3] / 255.0
    rms = (((dec[:, :3] - src[:, :3]) ** 2 * a[:, None]).sum() / max(3 * a.sum(), 1e-9)) ** 0.5
    return int(np.abs(dec[:, 3] - src[:, 3]).max()), float(rms)


def frames(manifest):
    names = [u[1] for u in build_tree.UPGRADES]
    lists = {k: [] for k in ('rnd_om_xm.rnd_part_image', 'rnd_part_image', 'rnd_om_xm.rnd_part_mask',
                             'rnd_part_mask', 'rnd_om_xm.rnd_category_icons', 'rnd_category_icons')}
    for name in names:
        for state, om_suffix, plain_suffix in STATES:
            key = f'om_xm_{name}{om_suffix}'
            m = manifest[key]
            if m['upgrade'] != name or m['state'] != state or m['list'] != 'overrides.career.rnd_om_xm.rnd_part_image':
                sys.exit(f'{key}: manifest says {m["upgrade"]} {m["state"]} in {m["list"]}')
            if m['plain_list_frame'] != f'{name}{plain_suffix}':
                sys.exit(f'{key}: plain frame {m["plain_list_frame"]}, expected {name}{plain_suffix}')
            lists['rnd_om_xm.rnd_part_image'].append((key, m['source_tif'], m['file']))
            lists['rnd_part_image'].append(
                (f'{name}{plain_suffix}', f'{TILES}tiles_rnd/rnd_part_image/{name}{plain_suffix}.tif', m['file']))
        key = f'om_xm_{name}_m'
        m = manifest[key]
        if m['state'] != 'mask' or m['list'] != 'overrides.career.rnd_om_xm.rnd_part_mask':
            sys.exit(f'{key}: manifest says {m["state"]} in {m["list"]}')
        lists['rnd_om_xm.rnd_part_mask'].append((key, m['source_tif'], m['file']))
        lists['rnd_part_mask'].append((name, f'{TILES}tiles_rnd/rnd_part_mask/{name}_m.tif', m['file']))
    for key in TYPE_KEYS:
        m = manifest[key]
        frame = f'tyres_{key}'
        for lst in ('rnd_om_xm.rnd_category_icons', 'rnd_category_icons'):
            lists[lst].append((frame, f'{CATEGORY_TIF}{frame}.tif', m['file']))
    used = {f for v in lists.values() for _n, _t, f in v}
    unused = sorted({m['file'] for m in manifest.values()} - used)
    if unused:
        sys.exit(f'manifest frames not placed: {unused}')
    return lists


def overrides_lists(db):
    root = Pkg(db.read(OVERRIDES)).root
    career = root['overrides']['Contents']['career']['Contents']
    out = {}
    for lst in ('rnd_part_image', 'rnd_part_mask', 'rnd_category_icons'):
        for prefix in ('', 'rnd_om_xm.'):
            node = career['rnd_om_xm']['Contents'] if prefix else career
            out[prefix + lst] = [e['Name'].value for e in node[lst]['Contents'].items]
    return out


def build(game, out):
    ui = common.ui_package(game)
    with open(MANIFEST) as f:
        manifest = json.load(f)['frames']
    lists = frames(manifest)
    names = overrides_lists(common.Archive(os.path.join(ui, DATABASES)))
    encoded = {}
    ops, edits, report = [], [], []
    for lst, entries in lists.items():
        view_name = f'{ARRAYS}{lst}.texarray'
        image_name = view_name + '.image'
        frags = None
        held = common.carriers(game, image_name)
        if not held:
            sys.exit(f'{lst}: no stock archive carries {image_name}')
        for erp, arc in held:
            got = (arc.read(image_name, 0), arc.read(image_name, 1), arc.read(view_name, 0))
            if frags is None:
                frags = got
            elif got != frags:
                sys.exit(f'{lst}: {erp} carries a different stock array')
            for i, part in enumerate(['image.f0', 'image.f1', 'texarray.f0']):
                ops.append({'op': 'replace-resource', 'erp': f'2025_asset_groups/ui_package/{erp}',
                            'resource': image_name if i < 2 else view_name, 'fragment': i if i < 2 else 0,
                            'source': f'{OUT}/{lst}.{part}'})
        head, data, view = frags
        fmt, w, h = struct.unpack_from('<III', head, 8)
        mips, slices = struct.unpack_from('<I', head, 24)[0], struct.unpack_from('<I', head, 28)[0]
        if (fmt != common.BC7_FORMAT or mips != 1 or len(data) != slices * w * h
                or struct.unpack_from('<I', view, 0x14)[0] != slices):
            sys.exit(f'{lst}: unexpected stock array (format {fmt}, {w}x{h}, {mips} mips, {slices} slices)')
        stock_names = names[lst]
        if len(stock_names) != slices or any(n in stock_names for n, _t, _f in entries):
            sys.exit(f'{lst}: overrides.uidb lists {len(stock_names)} entries for {slices} slices, or a name clashes')
        pixels = b''
        for frame, _tif, src in entries:
            if src not in encoded:
                pw, ph, rgba = png(src)
                if (pw, ph) != (w, h):
                    sys.exit(f'{src}: {pw}x{ph}, the {lst} slices are {w}x{h}')
                bc7 = bc7enc.encode_rgba(rgba, w, h)
                encoded[src] = (bc7,) + fidelity(bc7, rgba, w, h)
            pixels += encoded[src][0]
        head2 = bytearray(head)
        struct.pack_into('<I', head2, 28, slices + len(entries))
        view2 = bytearray(view)
        struct.pack_into('<I', view2, 0x14, slices + len(entries))
        for part, blob in (('image.f0', bytes(head2)), ('image.f1', data + pixels), ('texarray.f0', bytes(view2))):
            dst = os.path.join(out, OUT, f'{lst}.{part}')
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            with open(dst, 'wb') as f:
                f.write(blob)
        at = 'overrides.Contents.career.Contents.' + lst.replace('.', '.Contents.') + '.Contents'
        src_entry = stock_names[-1]
        for frame, tif, _src in entries:
            edits.append({'op': 'clone-entry', 'at': at, 'src': f'[Name={src_entry}]', 'as': frame,
                          'set': {'Value': tif}})
        report.append(f'{lst}: {w}x{h}, slices {slices}..{slices + len(entries) - 1} '
                      f'({entries[0][0]} .. {entries[-1][0]}) in {", ".join(e for e, _a in held)}')
    ops.append({'op': 'patch-pkg', 'erp': f'2025_asset_groups/ui_package/{DATABASES}',
                'resource': OVERRIDES, 'fragment': 0, 'edits': edits})
    with open(os.path.join(out, OPS), 'w') as f:
        json.dump({'schema_version': 1, 'ops': ops}, f, indent=2)
        f.write('\n')
    alpha = max(v[1] for v in encoded.values())
    colour = max(v[2] for v in encoded.values())
    report.append(f'{len(encoded)} frames BC7-encoded; round trip: alpha max {alpha}/255, '
                  f'colour RMS of the visible texels at most {colour:.2f}/255')
    return report


def files(root):
    found = {OPS} if os.path.isfile(os.path.join(root, OPS)) else set()
    for d, _dirs, names in os.walk(os.path.join(root, OUT)):
        found |= {os.path.relpath(os.path.join(d, n), root) for n in names}
    return found


def main():
    ap = argparse.ArgumentParser(description='Build the Tyres card art arrays and erp/rnd_tyres_card_art.json.')
    common.add_game_arg(ap)
    ap.add_argument('--work', default=os.path.join(common.BUILD, 'card_art'), help='work folder (default: build/card_art)')
    ap.add_argument('--write', action='store_true', help='replace the shipped art/arrays/ and erp/rnd_tyres_card_art.json')
    a = ap.parse_args()
    shutil.rmtree(a.work, ignore_errors=True)
    os.makedirs(os.path.join(a.work, 'erp'))
    for line in build(a.game, a.work):
        print(line)
    if a.write:
        shutil.rmtree(os.path.join(common.ADDON, OUT), ignore_errors=True)
        shutil.copytree(os.path.join(a.work, OUT), os.path.join(common.ADDON, OUT))
        shutil.copyfile(os.path.join(a.work, OPS), os.path.join(common.ADDON, OPS))
        print(f'wrote {OUT}/ and {OPS}')
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
