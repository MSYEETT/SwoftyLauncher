"""Build the 1990 handling files, their ERP ops and the physics database ops.

Each of the 11 stock 2026 team cars gets a 1990 handling file built from its stock VTF:

  EnergyRecoverySystem   removed, and the EnergyRecoverySystem parts of the radUpgrade* blocks
                         (the blocks stay, empty, like the stock radUpgradeWearMGUH)
  Engine                 naturally aspirated (TurboMode Off), a V8/V10/V12 RevsPowerSpline on the stock
                         15-vertex rev grid (so radUpgradeEngine still lines up), the family's rev limit
  GearBox                stock 8 ratios; the final drive keeps the stock top-gear speed at the limiter
  FuelTank               BSFC scaled so fuel per second at full power stays stock (baked AI fuel data)
  FuelECU                power scales divided by the stock standard scale, so the team delta is the season pack's
  Chassis                mass from the season pack
  F1Aero                 drag scaled by the season pack, no moving front wing; downforce stays stock

Option tyre_size: stock keeps the stock tyres (the Pirelli 2018 set ships the stock 18-inch meshes, which match
the stock TyreRadius 0.354/0.351). 13in matches the MP4/5B 13-inch shared tyre mesh: TyreRadius 0.323 front /
0.330 rear, AlloyRadius at its 0.175 m bead, the tyre damage node graphs remapped radially from the stock bead and
tread onto the new ones and widened to the new mesh, and the final drive scaled by the rear radius so road speed
per engine rev stays the same. TyreWidth, tyre mass, inertia and compression stay stock. 13in_raised also adds the
radius drop to each axle's RideHeight and setup ride-height range, for if the chassis sits lower on the smaller tyres.

Team power, mass and drag come from the season pack's per-team scalars (season.py), applied around a
common base. Teams that share a car folder (season.slots(): Larrousse on T12 drives the Williams car,
Dallara on T13 the Haas car) get the mean of their scalars and the engine of the team on the stock row.
T12 takes the audio id of the Lotus row, so Larrousse plays the V12.

Outputs (relative to the addon folder): erp/cars/vtf/<team>.vtf.f0/.f1, erp/cars/handling.json,
db/physics.json. The build compares with the shipped files and exits 1 on any difference; --write
replaces them.
"""
import argparse
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ADDON = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(ADDON))
for _p in (os.path.join(REPO, 'tools', 'swofty-ui'), os.path.join(REPO, 'tools'), HERE):
    if _p not in sys.path:
        sys.path.insert(0, _p)

import gamedb
import gamedir
import season
import vtf
from erp import parse as erp_parse
from erpsource import STORED, ZSTD, ZSTD_MAGIC, decompress

GROUPS = '2025_asset_groups'
GAMEDB = 'game_package/gamedatabases/gamedatabases.2025.bin'
PACKAGE = 'f1_2026_vehicle_package'

BASE_HP = 650.0
BASE_MASS = 575.0
SPREAD = 1.0
HP_W = 745.7
TORQUE_FLOOR = 0.6

FAMILIES = {
    'V12': {'limiter_rpm': 13500, 'peak_rpm': 13000, 'torque_peak': 0.82, 'bundle': 'ferrari_2026.bdl'},
    'V10': {'limiter_rpm': 13300, 'peak_rpm': 12500, 'torque_peak': 0.78, 'bundle': 'renault_2026.bdl'},
    'V8': {'limiter_rpm': 12500, 'peak_rpm': 11500, 'torque_peak': 0.75, 'bundle': 'ford_2026.bdl'},
}

ENGINES = {
    'mclaren': 'V10', 'williams': 'V10',
    'ferrari': 'V12', 'lotus': 'V12', 'larrousse': 'V12',
    'benetton': 'V8', 'tyrrell': 'V8', 'march': 'V8', 'ligier_prost': 'V8', 'minardi': 'V8',
    'brabham': 'V8', 'dallara': 'V8', 'arrows': 'V8',
}

AUDIO = {
    'mclaren': ['f1_m_mclaren_26'], 'ferrari': ['f1_m_ferrari_26'], 'williams': ['f1_m_williams_26'],
    'lotus': ['f1_m_alpine_26'], 'mercedes': ['f1_m_mercedes_26'], 'redbull': ['f1_m_red_bull_26'],
    'force_india': ['f1_m_aston_martin_26'], 'sauber': ['f1_m_sauber_26', 'f1_m_audi_26'],
    'toro_rosso': ['f1_m_rb_26'], 'gm_cadillac': ['f1_m_cadillac_26'], 'haas': ['f1_m_haas_26'],
}

SHARED_AUDIO_ID = {'team12_26': 'lotus', 'team13_26': None}

FALLBACK_AUDIO = [('f1_m_fom_car_26', 'V8')]

COMPONENT_SETS = [0x0516baca528f3258, 0xf818582c90f40d78]
PACKAGE_ROW = '{df75292f0fdc1a5c}[_metadata_=f1_2026]'

ERS_UPGRADES = [b'radUpgradeERS', b'radUpgradeWearES', b'radUpgradeWearMGUK']
ERS_ZERO = {b'MaxDeployPower': 0, b'MaxDeratePower': 0, b'MinInitialDeployPower': 0, b'HarvestLimitMGUK': 0,
            b'HarvestOverrideBonusMGUK': 0, b'HarvestLimitQualifyingMGUK': 0, b'DeployLimitMGUK': 0,
            b'MaxBrakeTorqueMGUK': 0, b'OverrideModeAvailable': 'false'}
VTF_DIRS = {'removed': 'erp/cars/vtf', 'zeroed': 'erp/cars/vtf_ers_zeroed'}

TYRE_SIZES = {'stock': '', '13in': '_13in', '13in_raised': '_13in_raised'}
TYRES_13 = {'Front': {'radius': 0.323, 'width': 0.311}, 'Rear': {'radius': 0.330, 'width': 0.454}}
STOCK_MESH_WIDTH = {'Front': 0.339, 'Rear': 0.418}
BEAD_13 = 0.175
ALLOY_13 = 0.182
TYRE_GRAPHS = {b'x0_tyre_fl': 'Front', b'x0_tyre_fr': 'Front', b'x0_tyre_bl': 'Rear', b'x0_tyre_br': 'Rear'}


class Archive:
    def __init__(self, path):
        self.path = path
        self.meta, res = erp_parse(path)
        self.res = {r[0]: r for r in res}

    def read(self, name, frag=0):
        _fn, off, _size, _flags, comp, packed = self.res[name][3][frag]
        with open(self.path, 'rb') as f:
            f.seek(self.meta['data_off'] + off)
            raw = f.read(packed)
        if comp == ZSTD or (comp != STORED and raw[:4] == ZSTD_MAGIC):
            return decompress(raw)
        return raw


def game_root(game):
    if game and os.path.isdir(os.path.join(game, GROUPS, PACKAGE)) and os.path.isfile(os.path.join(game, GAMEDB)):
        return game
    try:
        ui = gamedir.resolve(game)[0]
    except gamedir.NotFound as ex:
        sys.exit(f'{ex}: pass --game')
    return os.path.dirname(os.path.dirname(ui))


def cars():
    pack = season.Season.load()
    out = {}
    for sl in season.slots():
        out.setdefault(sl.asset, []).append(sl)
    rows = []
    for asset, group in out.items():
        own = [sl for sl in group if sl.team_row not in SHARED_AUDIO_ID] or group
        teams = [pack.teams[sl.team] for sl in group]
        rows.append({
            'car': asset,
            'vehicles': [sl.vehicle for sl in group],
            'teams': [t.id for t in teams],
            'name': ' / '.join(t.name for t in teams),
            'engine': ENGINES[own[0].team],
            'power': sum(t.power for t in teams) / len(teams),
            'weight': sum(t.weight for t in teams) / len(teams),
            'drag': sum(t.drag for t in teams) / len(teams),
            'audio': AUDIO[asset],
        })
    return rows


def spread(s):
    return 1.0 + (s - 1.0) * SPREAD


def rpm(x):
    return x * 60.0 / (2.0 * math.pi)


def rads(r):
    return r * 2.0 * math.pi / 60.0


def power_fraction(fam, omega):
    peak = rads(fam['peak_rpm'])
    rt = fam['torque_peak']
    k = 1.0 / ((1.0 - rt) ** 2 + 2.0 * (1.0 - rt))

    def torque(r):
        return max(1.0 - k * (r - rt) ** 2, TORQUE_FLOOR)

    r = omega / peak
    return r * torque(r) / torque(1.0)


def child(node, name):
    hits = [c for c in node.children if c.kind == 0 and c.name == name]
    if len(hits) != 1:
        raise KeyError(f'{name.decode()}: {len(hits)} matches under {node.name.decode()}')
    return hits[0]


def section(root, name):
    return child(root, name)


def value(node, name):
    return float(child(node, name).attr(b'value'))


def set_value(node, name, v):
    child(node, name).set_attr(b'value', fmt(v))


def fmt(v):
    if isinstance(v, str):
        return v.encode()
    s = f'{v:.6g}'
    if 'e' in s:
        s = f'{v:.9f}'.rstrip('0').rstrip('.')
    return s.encode()


def spline(node):
    return [(float(e.attr(b'x')), float(e.attr(b'y'))) for e in node.children if e.kind == 0]


def remove_ers(root):
    removed = [c for c in root.children if c.kind == 0 and c.name == b'EnergyRecoverySystem']
    if len(removed) != 1:
        raise ValueError('expected one EnergyRecoverySystem section')
    root.children = [c for c in root.children if c not in removed]
    for c in root.children:
        if c.kind == 0 and c.name == b'Upgrade':
            c.children = [k for k in c.children if not (k.kind == 0 and k.name == b'EnergyRecoverySystem')]
    for c in root.children:
        if c.kind == 0 and c.name == b'Upgrade' and c.attr(b'name') in ERS_UPGRADES:
            if any(k.kind == 0 for k in c.children):
                raise ValueError(f'{c.attr(b"name").decode()} keeps non-ERS parts')


def zero_ers(root):
    common = child(section(root, b'EnergyRecoverySystem'), b'Common')
    for k, v in ERS_ZERO.items():
        set_value(common, k, v)


ERS_MODES = {'removed': remove_ers, 'zeroed': zero_ers}


def named(node, name):
    return {c.attr(b'name').decode(): c for c in node.children if c.kind == 0 and c.name == name}


def remap_graph(graph, end, stock_radius):
    nodes = [n for n in graph.children if n.kind == 0 and n.name == b'node']
    radii = [math.hypot(float(n.attr(b'y')), float(n.attr(b'z'))) for n in nodes]
    bead, outer = min(radii), max(radii)
    new_outer = TYRES_13[end]['radius'] * outer / stock_radius
    widen = TYRES_13[end]['width'] / STOCK_MESH_WIDTH[end]
    for n, r in zip(nodes, radii):
        k = (BEAD_13 + (r - bead) * (new_outer - BEAD_13) / (outer - bead)) / r
        n.set_attr(b'x', fmt(float(n.attr(b'x')) * widen))
        n.set_attr(b'y', fmt(float(n.attr(b'y')) * k))
        n.set_attr(b'z', fmt(float(n.attr(b'z')) * k))


def tyres(root, size):
    if size == 'stock':
        return 1.0
    susp = section(root, b'SuspensionSystem')
    tyre = named(susp, b'Tyre')
    stock = {end: value(tyre[end], b'TyreRadius') for end in TYRES_13}
    for end, t in TYRES_13.items():
        set_value(tyre[end], b'TyreRadius', t['radius'])
        set_value(tyre[end], b'AlloyRadius', ALLOY_13)
    for graph in section(root, b'F1TyreDamage').children:
        if graph.kind == 0 and graph.name == b'PhysicsNodeGraph':
            end = TYRE_GRAPHS[graph.attr(b'name')]
            remap_graph(graph, end, stock[end])
    if size == '13in_raised':
        tuning = section(root, b'F1Tuning')
        for end, node in named(susp, b'Suspension').items():
            drop = stock[end] - TYRES_13[end]['radius']
            set_value(node, b'RideHeight', round(value(node, b'RideHeight') + drop, 4))
            for k in (f'{end}RideHeightMin', f'{end}RideHeightMax'):
                set_value(tuning, k.encode(), round(value(tuning, k.encode()) + drop, 4))
    return TYRES_13['Rear']['radius'] / stock['Rear']


def convert(root, row, ers, size='stock'):
    fam = FAMILIES[row['engine']]
    ERS_MODES[ers](root)
    rolling = tyres(root, size)

    eng = section(root, b'Engine')
    curve = child(eng, b'RevsPowerSpline')
    stock_curve = spline(curve)
    stock_peak = max(y for _, y in stock_curve)
    peak_w = BASE_HP * spread(row['power']) * HP_W
    for e in [e for e in curve.children if e.kind == 0]:
        x = float(e.attr(b'x'))
        e.set_attr(b'y', fmt(round(peak_w * power_fraction(fam, x))))
    stock_limiter = value(eng, b'LimiterRotationRate')
    limiter = rads(fam['limiter_rpm'])
    set_value(eng, b'LimiterRotationRate', round(limiter, 1))
    set_value(eng, b'TurboMode', 'Off')

    box = section(root, b'GearBox')
    final = value(box, b'FinalDriveRatio') * limiter / stock_limiter * rolling
    set_value(box, b'FinalDriveRatio', round(final, 3))

    real_peak = max(peak_w * power_fraction(fam, x) for x, _ in stock_curve)
    tank = section(root, b'FuelTank')
    for k in (b'FuelBsfcMin', b'FuelBsfcMax'):
        set_value(tank, k, value(tank, k) * stock_peak / real_peak)

    ecu = section(root, b'FuelECU')
    std = value(ecu, b'FuelPowerScaleStandard')
    for k in (b'FuelPowerScaleLean', b'FuelPowerScaleStandard', b'FuelPowerScaleRich', b'FuelPowerScaleMax'):
        set_value(ecu, k, round(value(ecu, k) / std, 4))

    chassis = section(root, b'Chassis')
    mass = BASE_MASS * spread(row['weight'])
    set_value(chassis, b'Mass', round(mass, 1))

    aero = section(root, b'F1Aero')
    drag = spread(row['drag'])
    for k in (b'BodyDragCoeffMin', b'BodyDragCoeffMax', b'FrontWingCoD0', b'FrontWingCoD1',
              b'RearWingCoD0', b'RearWingCoD1'):
        set_value(aero, k, value(aero, k) * drag)
    set_value(aero, b'FrontWingRotationAngleRadians', 0)

    return {'team': row['name'], 'engine': row['engine'], 'hp': round(real_peak / HP_W),
            'limiter_rpm': fam['limiter_rpm'], 'mass': round(mass, 1), 'drag': round(drag, 3),
            'final_drive': round(final, 3)}


def collect(root_dir, rows):
    found = {}
    top = os.path.join(root_dir, GROUPS, PACKAGE, 'teams')
    for row in rows:
        car = row['car']
        name = f'eaid://{PACKAGE}/teams/{car}/handling/{car}.vtf'
        d = os.path.join(top, car, 'wep')
        for n in sorted(os.listdir(d)):
            if not n.endswith('.erp'):
                continue
            arc = Archive(os.path.join(d, n))
            if name in arc.res:
                rel = os.path.relpath(arc.path, root_dir).replace(os.sep, '/')
                found.setdefault(car, []).append((rel, name, arc.read(name, 0), arc.read(name, 1)))
    return found


def build(game, out):
    root_dir = game_root(game)
    rows = cars()
    found = collect(root_dir, rows)
    files, ops, report = {}, [], []
    for row in rows:
        copies = found[row['car']]
        variants = {(c[2], c[3]) for c in copies}
        if len(variants) != 1:
            raise ValueError(f'{row["car"]}: {len(variants)} different stock handling files')
        f0, f1 = variants.pop()
        stock = vtf.parse(f0)
        version = int.from_bytes(f1[4:8], 'little')
        if vtf.write(stock) != f0 or vtf.tsmd(stock, version) != f1:
            raise ValueError(f'{row["car"]}: stock VTF does not round-trip')
        for ers, folder in VTF_DIRS.items():
            for tyre_size, suffix in TYRE_SIZES.items():
                root = vtf.parse(f0)
                got = convert(root, row, ers, tyre_size)
                n0 = vtf.write(root)
                n1 = vtf.tsmd(root, version)
                if ers == 'removed' and tyre_size == 'stock':
                    info, size = got, len(n0)
                for i, blob in enumerate((n0, n1)):
                    rel = f'{folder}{suffix}/{row["car"]}.vtf.f{i}'
                    files[rel] = blob
                    for erp, name, _a, _b in copies:
                        ops.append({'op': 'replace-resource', 'when': {'ers': ers, 'tyre_size': tyre_size},
                                    'erp': erp, 'resource': name, 'fragment': i, 'source': rel})
        info['car'] = row['car']
        info['archives'] = len(copies)
        info['bytes'] = (len(f0), size)
        report.append(info)
    ops.sort(key=lambda o: (o['when']['ers'] != 'removed', list(TYRE_SIZES).index(o['when']['tyre_size']),
                            o['erp'], o['resource'], o['fragment']))
    files['erp/cars/handling.json'] = (json.dumps({'schema_version': 1, 'ops': ops}, indent=1) + '\n').encode()
    files['db/physics.json'] = (json.dumps(db_ops(root_dir, rows), indent=1) + '\n').encode()
    for rel, blob in files.items():
        dst = os.path.join(out, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        with open(dst, 'wb') as f:
            f.write(blob)
    return files, report


def load_gamedb(root_dir):
    with open(os.path.join(root_dir, GAMEDB), 'rb') as f:
        return gamedb.read(f.read())


def component_rows(recs):
    by = {r.key: r for r in recs}
    out = []
    for prefix in COMPONENT_SETS:
        for n in range(16):
            r = by.get(gamedb.fnv1a64(f'{n}._metadata_', prefix))
            if r is None:
                continue
            label = r.value.decode('latin-1')
            row = gamedb.fnv1a64_unwind(r.key, '0._metadata_')
            if gamedb.fnv1a64('0.parc_ferme_grid_drop', row) not in by:
                raise ValueError(f'{label}: not a component row')
            out.append(f'{{{row:016x}}}[_metadata_={label}]')
    return out


def audio_rows(recs, labels):
    keys = {r.key for r in recs}
    out = {}
    for r in recs:
        if r.type != 2 or r.value.decode('latin-1') not in labels:
            continue
        label = r.value.decode('latin-1')
        prefix = gamedb.fnv1a64_unwind(r.key, '0._metadata_')
        if gamedb.fnv1a64('0.bundle_name', prefix) in keys:
            if label in out:
                raise ValueError(f'{label}: more than one audio row')
            out[label] = f'{{{prefix:016x}}}[_metadata_={label}]'
    missing = sorted(set(labels) - set(out))
    if missing:
        raise ValueError(f'no audio row for {", ".join(missing)}')
    return out


def db_ops(root_dir, rows):
    recs = load_gamedb(root_dir)
    labels = [a for row in rows for a in row['audio']] + [a for a, _ in FALLBACK_AUDIO]
    audio = audio_rows(recs, labels)
    ops = []
    for row in rows:
        for v in row['vehicles']:
            sel = f'entrants.vehicles.vehicle[_metadata_={v}]'
            ops.append({'op': 'set', 'select': sel, 'field': 'hud_dial', 'value': '{{hud_dial}}'})
            ops.append({'op': 'set', 'select': sel, 'field': 'drs_capable', 'value': 'False'})
        for a in row['audio']:
            ops.append({'op': 'set', 'select': audio[a], 'field': 'bundle_name',
                        'value': FAMILIES[row['engine']]['bundle']})
    for a, eng in FALLBACK_AUDIO:
        ops.append({'op': 'set', 'select': audio[a], 'field': 'bundle_name', 'value': FAMILIES[eng]['bundle']})
    for team_row, same_as in SHARED_AUDIO_ID.items():
        if same_as:
            src = season.slot(same_as).team_row
            ops.append({'op': 'set', 'select': f'entrants.teams.team[_metadata_={team_row}]', 'field': 'audio_id',
                        'value_from': f'entrants.teams.team[_metadata_={src}].audio_id'})
    ops.append({'op': 'set', 'select': PACKAGE_ROW, 'field': 'is_26_aero_regulations', 'value': 'False',
                'when': {'classic_aero': True}})
    for sel in component_rows(recs):
        ops.append({'op': 'set', 'select': sel, 'field': 'allocation', 'value': '99'})
        for k in ('first_new_grid_drop', 'subsequent_grid_drop', 'parc_ferme_grid_drop'):
            ops.append({'op': 'set', 'select': sel, 'field': k, 'value': '0'})
        for k in ('ai_pq_max', 'ai_pq_min', 'ai_race_max', 'ai_race_min', 'human_max', 'human_min'):
            ops.append({'op': 'set', 'select': sel, 'field': f'wear_rates.{k}', 'value': '0.0'})
    return {'schema_version': 1, 'ops': ops}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    ap.add_argument('--game', help='the F1 25 folder with stock 2026 team archives')
    ap.add_argument('--write', action='store_true', help='replace the shipped files with the build')
    args = ap.parse_args()
    out = ADDON if args.write else os.path.join(ADDON, 'build')
    files, report = build(args.game, out)
    for r in report:
        print(f'{r["car"]:12} {r["team"]:24} {r["engine"]:4} {r["hp"]:4} hp  {r["limiter_rpm"]} rpm  '
              f'{r["mass"]:6} kg  drag x{r["drag"]}  final {r["final_drive"]}  '
              f'{r["bytes"][0]} -> {r["bytes"][1]} bytes, {r["archives"]} archive(s)')
    if args.write:
        return 0
    diff = [rel for rel, blob in files.items()
            if not os.path.exists(os.path.join(ADDON, rel)) or open(os.path.join(ADDON, rel), 'rb').read() != blob]
    for rel in diff:
        print(f'differs: {rel}')
    return 1 if diff else 0


if __name__ == '__main__':
    sys.exit(main())
