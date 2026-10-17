import argparse, hashlib, json, os, struct, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import erp, gamedb, vtf

ZSTD_MAGIC = b'\x28\xb5\x2f\xfd'

try:
    import zstandard
    _dz = zstandard.ZstdDecompressor()

    def unzstd(b, size):
        return _dz.decompress(b, max_output_size=size)
except ImportError:
    def unzstd(b, size):
        return subprocess.run(['zstd', '-dc'], input=b, capture_output=True, check=True).stdout


class Archive:
    def __init__(self, path):
        self.path = path
        self.meta, self.res = erp.parse(path)
        self.by = {r[0]: r for r in self.res}

    def names(self):
        return [r[0] for r in self.res]

    def raw(self, r):
        out = []
        with open(self.path, 'rb') as f:
            for fn, off, size, flags, comp, packed in r[3]:
                f.seek(self.meta['data_off'] + off)
                out.append((f.read(packed), size))
        return out

    def data(self, r):
        return [unzstd(b, size) if b[:4] == ZSTD_MAGIC else b for b, size in self.raw(r)]

    def md5_ok(self, r):
        return hashlib.md5(b''.join(self.data(r))).hexdigest() == r[4]


class Report:
    def __init__(self):
        self.fails = 0

    def check(self, ok, msg):
        print(('ok   ' if ok else 'FAIL ') + msg)
        if not ok:
            self.fails += 1


def fnv(path, h=None):
    return gamedb.fnv1a64(path.encode()) if h is None else gamedb.fnv1a64(path.encode(), h)


def check_db(rep, path, codes, slots):
    data = open(path, 'rb').read()
    recs = gamedb.read(data)
    rep.check(gamedb.write(recs) == data, f'database re-serialises byte-identically ({len(recs)} records)')
    db = {r.key: r.value.decode('utf-8', 'replace') for r in recs}
    n = int(db[fnv('tracks._Track_instancecount_')])
    tracks = {db.get(fnv(f'tracks.Track_{i}.instanceName')): i for i in range(n)}
    res_n = int(db[fnv('tracks.track_resource._TrackResource_instancecount_')])
    resources = {db.get(fnv(f'tracks.track_resource.TrackResource_{i}._metadata_')): i for i in range(res_n)}
    for code in codes:
        i = tracks.get(code)
        rep.check(i is not None, f'{code}: has a track row inside the {n}-row count')
        if i is None:
            continue
        label = db.get(fnv(f'tracks.Track_{i}.track_resource'))
        j = resources.get(label)
        rep.check(j is not None, f'{code}: track resource {label} exists inside the {res_n}-row count')
        if j is not None:
            group = db.get(fnv(f'tracks.track_resource.TrackResource_{j}.worldAssetGroup'))
            rep.check(any(s in group for s in slots.get(code, [])),
                      f'{code}: world asset group {group} is the slot the plan fills')
        listed = [v for k, v in db.items() if ',' in v and code in v.split(',') and 'melbourne' in v.split(',')]
        rep.check(len(listed) > 0, f'{code}: listed in {len(listed)} available_tracks list(s)')
    return db


def load_db(path):
    return {r.key: r.value.decode('utf-8', 'replace') for r in gamedb.read(open(path, 'rb').read())}


def check_reader_keys(rep, exe, original, built, manifests, codes):
    try:
        import track_keys as tk
        names = tk.exe_names(exe)
    except ImportError as e:
        print(f'skip reader keys: {e}; they need numpy, pefile and capstone')
        return
    reg = json.load(open(tk.REGISTRY))
    old, new = load_db(original), built
    print(f'reader keys: {len(names)} game-DB names from {exe}')
    cache = {}

    def need(label, src_db, src, dst):
        if (id(src_db), src) not in cache:
            cache[(id(src_db), src)] = tk.row_keys(src_db, src, names)
        keys = cache[(id(src_db), src)]
        gone = [k for k in keys if tk.fnv('.' + k, dst) not in new]
        rep.check(not gone, f'{label}: all {len(keys)} keys the game reads on the source row exist {gone[:4]}')

    track_base, track_count = tk.fnv('tracks.Track_'), tk.fnv('tracks._Track_instancecount_')
    res_base = tk.fnv('tracks.track_resource.TrackResource_')
    res_count = tk.fnv('tracks.track_resource._TrackResource_instancecount_')
    old_tracks = tk.table_rows(old, track_base, track_count, 'instanceName')
    new_tracks = tk.table_rows(new, track_base, track_count, 'instanceName')
    old_res = tk.table_rows(old, res_base, res_count, '_metadata_')
    new_res = tk.table_rows(new, res_base, res_count, '_metadata_')
    ds_base, ds_count = int(reg['datasets']['base'], 16), int(reg['datasets']['count'], 16)
    for code in codes:
        act = manifests[code].get('activation', {})
        base = act.get('copy_from_track') or act.get('copy_slot')
        slot = act.get('copy_slot') or code
        if code not in new_tracks:
            rep.check(False, f'{code}: has a track row')
            continue
        row = new_tracks[code]
        if code not in old_tracks:
            need(f'{code}: tracks.Track from {slot}', old, old_tracks[slot], row)
        src_res = old.get(tk.fnv('.track_resource', old_tracks.get(code, old_tracks.get(slot))))
        mine = new.get(tk.fnv('.track_resource', row))
        if src_res in old_res and mine in new_res:
            need(f'{code}: track resource {mine} from {src_res}', old, old_res[src_res], new_res[mine])
        for t in reg['per_track']:
            o = tk.table_rows(old, int(t['base'], 16), int(t['count'], 16), t['labels'][0])
            n = tk.table_rows(new, int(t['base'], 16), int(t['count'], 16), t['labels'][0])
            src = slot if slot in o else base
            if code in o or src not in o:
                continue
            if code not in n:
                rep.check(False, f'{code}: {t["id"]} has a row')
                continue
            need(f'{code}: {t["id"]} from {src}', old, o[src], n[code])
        for i in range(int(old.get(ds_count, '0'))):
            ds = tk.fnv(str(i), ds_base)
            label = old.get(tk.fnv('._metadata_', ds))
            o = tk.table_rows(old, tk.fnv('.TrackData_', ds), tk.fnv('._TrackData_instancecount_', ds), 'track')
            n = tk.table_rows(new, tk.fnv('.TrackData_', ds), tk.fnv('._TrackData_instancecount_', ds), 'track')
            if base not in o:
                continue
            if code not in n:
                rep.check(False, f'{code}: TrackData in {label} has a row')
                continue
            need(f'{code}: TrackData {label} from {base}', old, o[base], n[code])


def check_same_layout(rep, label, original, mine, allowed=None):
    a, b = Archive(original), Archive(mine)
    rep.check(a.names() == b.names(), f'{label}: same {len(a.names())} resources in the same order')
    changed = [n for n in a.names() if [x[0] for x in a.raw(a.by[n])] != [x[0] for x in b.raw(b.by[n])]]
    bad = [n for n in changed if not b.md5_ok(b.by[n])]
    rep.check(not bad, f'{label}: {len(changed)} rewritten resource(s), all with a correct MD5')
    if allowed is not None:
        stray = [n for n in changed if not allowed(n, b.by[n])]
        rep.check(not stray, f'{label}: only the intended resources changed {stray[:3]}')
    return b, changed


def main():
    ap = argparse.ArgumentParser(description='Check a profile built with swofty build against the game it came from')
    ap.add_argument('overlay', help='the overlay folder swofty build printed (holds plan.json)')
    ap.add_argument('--game', required=True, help='game folder or pristine copy the build read')
    data = os.environ.get('SWOFTY_DATA_DIR') or os.path.expanduser('~/.local/share/swofty')
    ap.add_argument('--maps', default=os.path.join(data, 'addon-data/teamsimplified.mods/library'),
                    help='the Team Simplified library, or a folder of track packages')
    ap.add_argument('--exe', help='F1_25.exe for the reader-key check (default: the game folder\'s)')
    a = ap.parse_args()
    exe = a.exe or os.path.join(a.game, 'F1_25.exe')
    plan = json.load(open(os.path.join(a.overlay, 'plan.json')))
    rep = Report()
    dirs, manifests = {}, {}
    for c in os.listdir(a.maps):
        for d in (os.path.join(a.maps, c, 'track'), os.path.join(a.maps, c)):
            if os.path.isfile(os.path.join(d, 'manifest.json')):
                m = json.load(open(os.path.join(d, 'manifest.json')))
                code = m.get('track_code') or c
                dirs[code], manifests[code] = d, m
                break
    codes = list(dirs)
    slot_dir = 'ui_package/texture_arrays/invitational_events/event_images/'
    slots = {}
    for rel, src in plan['replace'].items():
        if slot_dir in rel and rel.endswith('.erp'):
            names = set(Archive(src).names())
            for c in codes:
                if f'eaid://environment_package/tracks/{c}/wep/{c}_common.wep' in names:
                    slots.setdefault(c, []).append(os.path.basename(rel)[:-4])
    codes = [c for c in codes if c in slots]
    print('maps in this build:', ', '.join(f'{c} -> {slots[c]}' for c in codes))
    for rel, src in sorted(plan['replace'].items()):
        orig = os.path.join(a.game, rel)
        if rel.endswith('gamedatabases.2025.bin'):
            built = check_db(rep, src, codes, slots)
            if os.path.isfile(exe):
                check_reader_keys(rep, exe, orig, built, manifests, codes)
            else:
                print(f'skip reader keys: no {exe}')
        elif any(rel.endswith(f'/{s[0]}.erp') for s in slots.values()):
            code = next(c for c, s in slots.items() if rel.endswith(f'/{s[0]}.erp'))
            mine, stock = Archive(src), Archive(orig)
            pkg = Archive(os.path.join(dirs[code], manifests[code]['erp']))
            names = set(mine.names())
            rep.check(set(pkg.names()) <= names, f'{rel}: holds all {len(pkg.names())} {code} resources')
            rep.check(set(stock.names()) <= names, f'{rel}: keeps its {len(stock.names())} stock resource(s)')
            rewritten = [n for n in pkg.names() if [x[0] for x in pkg.raw(pkg.by[n])] != [x[0] for x in mine.raw(mine.by[n])]]
            rep.check(all(mine.md5_ok(mine.by[n]) for n in rewritten),
                      f'{rel}: {len(rewritten)} repointed resource(s) carry a correct MD5')
        elif '/f1_2026_vehicle_package/' in rel:
            mine, changed = check_same_layout(rep, rel, orig, src, lambda n, r: r[1] == 'VTF')
            for n in changed:
                t = vtf.parse(mine.data(mine.by[n])[0])
                have = vtf.track_settings(t)
                rep.check(all(c in have for c in codes if manifests[c].get('ers')), f'{n}: has ERS settings for every map')
        elif rel.startswith('2025_asset_groups/ui_package/') and slot_dir not in rel:
            check_same_layout(rep, rel, orig, src, lambda n, r: n.endswith(('.uidb', '.uidta', '.texarray', '.texarray.image')))
        elif slot_dir in rel:
            mine, stock = Archive(src), Archive(orig)
            rep.check(set(stock.names()) <= set(mine.names()), f'{rel}: keeps its stock pictures and adds {len(mine.names()) - len(stock.names())}')
    for rel, src in sorted(plan['add'].items()):
        rep.check(os.path.isfile(src), f'{rel}: source exists')
    print('failures', rep.fails)
    sys.exit(1 if rep.fails else 0)


if __name__ == '__main__':
    main()
