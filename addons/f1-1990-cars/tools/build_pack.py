"""Build the private 1990 car pack: the 13 car archives, the package's common.erp and its tyre compounds.

Each source archive is a porter output patched onto a stock 2026 team archive, so its resources are named
`eaid://f1_2026_vehicle_package/teams/<donor>/...`. The build renames them byte for byte to
`eaid://f1_1990_vehicle_package/teams/<stem>/...` (same length, so no offset in any payload moves) and leaves the
bare `.mipmaps` sidecar paths on the stock 2026 files. It drops the car's own handling file, which the f1-1990 addon
ships per option in `<folder>_handling.erp`, and gives every carmodel layer the car's 1990 engine audio label
(`f1_m_<name>_90`, addons/f1-1990/tools/audio.py) so it plays the 1990 sound rows of the f1-1990 addon. A car
whose folder differs from its stem also gets its World, event graph and layers under the folder name.

Outputs (relative to the addon folder, not committed): erp/teams/<folder>/wep/<folder>.erp, erp/teams/common.erp,
erp/tyrecompounds/tyrecompounds.erp, and assets/cars.json (the file.put and group.register ops, committed).
"""
import argparse
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ADDON = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(ADDON))
sys.path.insert(0, os.path.join(REPO, 'tools'))
sys.path.insert(0, os.path.join(REPO, 'addons', 'f1-1990', 'tools'))

import erpw
import audio
import cars as C

TRIES = '/mnt/work/.mp45b/porter_a/tries'
TEAM_ERP = 'F1 25/2025_asset_groups/f1_2026_vehicle_package/teams/{d}/wep/{d}.erp'
SOURCES = {
    'mclaren': 'gridn13_mclaren', 'ferrari': 'gridc13_ferrari', 'williams': 'gridc13_williams',
    'benetton': 'gridc13_lotus', 'tyrrell': 'gridc13_mercedes', 'lotus': 'gridc13_redbull',
    'march': 'gridn13_force_india', 'larrousse': 'gridn13_larrousse', 'ligier_prost': 'gridn13_sauber',
    'minardi': 'gridn13_toro_rosso', 'brabham': 'gridn13_gm_cadillac', 'dallara': 'gridn13_dallara',
    'arrows': 'gridn13_haas',
}
COMMON = '/mnt/work/.mp45b/porter_a/tries/tyres13/common.erp'
TYRES = '2025_asset_groups/f1_2026_vehicle_package/tyrecompounds/tyrecompounds.erp'

OLD = f'eaid://{C.SOURCE_PACKAGE}/'.encode()
NEW = f'eaid://{C.PACKAGE_FOLDER}/'.encode()
EAID = re.compile(re.escape(OLD) + rb'[^\x00]*')
FOLDER_COPIES = ('World', 'EventGraph', 'WOInstances')


def kind(r):
    return r.type.rstrip(b'\0').decode()


def rename_bytes(data, donor=None, stem=None):
    def sub(m):
        s = NEW + m.group(0)[len(OLD):]
        if donor and donor != stem:
            s = s.replace(donor.encode(), stem.encode())
        assert len(s) == len(m.group(0))
        return s
    return EAID.sub(sub, data)


def rename_archive(e, donor=None, stem=None):
    renamed = rewritten = 0
    for r in e.res:
        n = rename_bytes(r.name.encode(), donor, stem).decode()
        if n != r.name:
            r.name = n
            renamed += 1
        for i in range(len(r.frags)):
            d = e.frag_data(r, i)
            nd = rename_bytes(d, donor, stem)
            if nd != d:
                e.set_frag(r, i, nd)
                rewritten += 1
    return renamed, rewritten


def copy_res(e, r, name):
    q = erpw.Res()
    q.name, q.type, q.unk, q.hash = name, r.type, r.unk, r.hash
    q.frags = []
    for f in r.frags:
        g = erpw.Frag()
        g.name, g.off, g.size, g.flags, g.comp, g.packed, g.raw = f.name, f.off, f.size, f.flags, f.comp, f.packed, f.raw
        q.frags.append(g)
    return q


def leftovers(e, needle):
    n = 0
    for r in e.res:
        for i in range(len(r.frags)):
            for m in re.finditer(rb'eaid://[\x21-\x7e]*', e.frag_data(r, i)):
                n += needle in m.group(0)
        n += needle.decode() in r.name
    return n


def set_audio(e, c):
    layers = [r for r in e.res if kind(r) == 'WOInstances' and r.name.endswith('-carmodel.layer')]
    if not layers:
        raise ValueError(f'{c.team}: no carmodel layer')
    labels = set()
    for r in layers:
        d = e.frag_data(r, 0)
        old = audio.switch_name(d)
        new = audio.label_1990(c, old)
        e.set_frag(r, 0, audio.rewrite_switch(d, old, new))
        labels.add(new)
    if len(labels) != 1:
        raise ValueError(f'{c.team}: carmodel layers disagree on the audio label: {labels}')
    return labels.pop()


def build_car(c, src, out):
    e = erpw.Erp(src)
    renamed, rewritten = rename_archive(e, c.donor, c.stem)
    vtf = [f'eaid://{C.PACKAGE_FOLDER}/teams/{c.stem}/handling/{c.stem}.vtf']
    before = len(e.res)
    e.res = [r for r in e.res if r.name not in vtf]
    dropped = before - len(e.res)
    extra = 0
    if c.stem != c.folder:
        old = f'/teams/{c.stem}/wep/{c.stem}'
        new = f'/teams/{c.folder}/wep/{c.folder}'
        for r in list(e.res):
            if kind(r) in FOLDER_COPIES and old in r.name:
                e.res.append(copy_res(e, r, r.name.replace(old, new)))
                extra += 1
    world = [r.name for r in e.res if kind(r) == 'World']
    want = f'eaid://{C.PACKAGE_FOLDER}/teams/{c.folder}/wep/{c.folder}.wep'
    if want not in world:
        raise ValueError(f'{c.team}: no World {want} (have {world})')
    left = leftovers(e, OLD)
    if left:
        raise ValueError(f'{c.team}: {left} eaid reference(s) still name {OLD.decode()}')
    sound = set_audio(e, c)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    e.to_file(out)
    return {'team': c.team, 'folder': c.folder, 'stem': c.stem, 'source': os.path.relpath(src, TRIES), 'audio': sound,
            'resources': len(e.res), 'renamed': renamed, 'rewritten': rewritten, 'handling_dropped': dropped,
            'folder_copies': extra, 'bytes': os.path.getsize(out)}


def build_plain(src, out):
    e = erpw.Erp(src)
    renamed, rewritten = rename_archive(e)
    left = leftovers(e, OLD)
    if left:
        raise ValueError(f'{src}: {left} eaid reference(s) still name {OLD.decode()}')
    os.makedirs(os.path.dirname(out), exist_ok=True)
    e.to_file(out)
    return {'source': src, 'resources': len(e.res), 'renamed': renamed, 'rewritten': rewritten,
            'bytes': os.path.getsize(out)}


def ops():
    out = [{'op': 'file.put', 'source': 'erp/teams/common.erp', 'target': C.COMMON_ARCHIVE},
           {'op': 'file.put', 'source': 'erp/tyrecompounds/tyrecompounds.erp', 'target': C.TYRE_ARCHIVE},
           {'op': 'group.register', 'group': C.TYRE_GROUP, 'erp': C.TYRE_ARCHIVE,
            'template': f'asset_groups/{C.SOURCE_PACKAGE}/tyrecompounds/tyrecompounds.assetgroup'}]
    for c in C.CARS:
        out.append({'op': 'file.put', 'source': f'erp/teams/{c.folder}/wep/{c.folder}.erp', 'target': C.archive(c)})
    for c in C.CARS:
        out.append({'op': 'group.register', 'group': C.group(c),
                    'erps': [C.COMMON_ARCHIVE, C.archive(c), C.handling_archive(c)],
                    'template': f'asset_groups/{C.SOURCE_PACKAGE}/teams/{c.folder}/wep/{c.folder}.assetgroup'})
    return {'schema_version': 1, 'ops': out}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    ap.add_argument('--game', default='/mnt/gaming/SteamLibrary/steamapps/common/F1 25',
                    help='F1 25 folder (read only), for the stock tyre compounds')
    ap.add_argument('--tries', default=TRIES, help='folder with the porter tries named in SOURCES')
    ap.add_argument('--common', default=COMMON, help='the 13-inch common.erp (f1_2026 names)')
    ap.add_argument('--only', help='comma-separated teams to build')
    ap.add_argument('--ops-only', action='store_true', help='write assets/cars.json and stop')
    args = ap.parse_args()
    with open(os.path.join(ADDON, 'assets', 'cars.json'), 'w') as f:
        f.write(json.dumps(ops(), indent=1) + '\n')
    if args.ops_only:
        return 0
    report = {'cars': []}
    only = set(args.only.split(',')) if args.only else None
    for c in C.CARS:
        if only and c.team not in only:
            continue
        src = os.path.join(args.tries, SOURCES[c.team], TEAM_ERP.format(d=c.donor))
        r = build_car(c, src, os.path.join(ADDON, 'erp', 'teams', c.folder, 'wep', f'{c.folder}.erp'))
        report['cars'].append(r)
        print(f'{c.team:13} {c.folder:15} stem {c.stem:12} {r["resources"]:5} res, {r["renamed"]} renamed, '
              f'{r["rewritten"]} fragments, handling -{r["handling_dropped"]}, +{r["folder_copies"]} folder copies, '
              f'audio {r["audio"]}, '
              f'{r["bytes"] / 1e6:.1f} MB')
    if not only:
        report['common'] = build_plain(args.common, os.path.join(ADDON, 'erp', 'teams', 'common.erp'))
        report['tyres'] = build_plain(os.path.join(args.game, TYRES),
                                      os.path.join(ADDON, 'erp', 'tyrecompounds', 'tyrecompounds.erp'))
        print('common', report['common'])
        print('tyres', report['tyres'])
    with open(os.path.join(ADDON, 'erp', 'build_report.json'), 'w') as f:
        json.dump(report, f, indent=1)
    return 0


if __name__ == '__main__':
    sys.exit(main())
