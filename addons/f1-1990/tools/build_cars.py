"""Build the 1990 handling archives, their asset ops and the 1990 car database ops.

Each 1990 team drives its own vehicle row in the `f1_1990` vehicle package (tools/cars.py). Its handling file is built
from the stock 2026 handling file of the car's donor folder and shipped as a small archive,
`f1_1990_vehicle_package/teams/<folder>/wep/<folder>_handling.erp`, one per `ers` and `tyre_size` value. The car pack
(swofty.f1-1990-cars) lists that archive in the car's asset group next to the car archive and the package's
common.erp. No 2026 archive, row or file is touched.

  EnergyRecoverySystem   removed, and the EnergyRecoverySystem parts of the radUpgrade* blocks
                         (the blocks stay, empty, like the stock radUpgradeWearMGUH)
  Engine                 naturally aspirated (TurboMode Off), a V8/V10/V12 RevsPowerSpline on the stock
                         15-vertex rev grid (so radUpgradeEngine still lines up), the family's rev limit
  GearBox                stock 8 ratios; the final drive keeps the stock top-gear speed at the limiter
  FuelTank               BSFC scaled so fuel per second at full power stays stock (baked AI fuel data)
  FuelECU                power scales divided by the stock standard scale, so the team delta is the season pack's
  Chassis                mass from the season pack
  F1Aero                 drag scaled by the season pack, no moving front wing; downforce stays stock; no DRS
                         hardware: the DRS failure/joint keys removed, flap angle, open/close speed and
                         DRS-open wing factors as on the safety car (0, 1, 1.0), radUpgradeDRS emptied

Option tyre_size: the f1_1990 package carries the MP4/5B 13-inch tyre set, so both values use TyreRadius 0.323 front /
0.330 rear, AlloyRadius at its 0.175 m bead, the tyre damage node graphs remapped radially from the stock bead and
tread onto the new ones and widened to the new mesh, and the final drive scaled by the rear radius so road speed per
engine rev stays the same. TyreWidth, tyre mass, inertia and compression stay stock. 13in_raised also adds the radius
drop to each axle's RideHeight and setup ride-height range.

Team power, mass and drag come from the season pack's per-team scalars (season.py), applied around a common base.

Outputs (relative to the addon folder): cars/handling/<ers>_<tyre_size>/<folder>_handling.erp, cars/handling.json
(file.put ops keyed by the options) and db/cars.json (the f1_1990 and f1_1990b package rows, the 13 vehicle rows
labelled with the grid lane's tokens and their 13 B-spec copies `<token>_1990b`; the name strings come from the grid
lane's lang ops). The build compares with the shipped files and exits 1 on any difference; --write replaces them.
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

import cars as C
import erpw
import gamedir
import season
import vtf

GROUPS = '2025_asset_groups'
GAMEDB = 'game_package/gamedatabases/gamedatabases.2025.bin'
PACKAGE = C.SOURCE_PACKAGE
VEHICLE_FIELDS = os.path.join(REPO, 'addons', 'custom-grid-size', 'db', 'grid.json')

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

PACKAGE_FIELDS = ['_metadata_', 'ai_package_folder', 'available', 'blob_shadow_decal', 'crew_livery_asset_group',
                  'driver_animation_set', 'garage_asset_group', 'has_cockpit_halo', 'is_26_aero_regulations',
                  'is_equal_performance_vehicle_package', 'is_story_vehicle_package', 'pit_stop_asset_group',
                  'pit_wall_asset_group', 'resource_path', 'resource_path_specialisation', 'stat_id',
                  'tyre_compounds_asset_group']
PACKAGE_SET = {
    'ai_package_folder': C.AI_FOLDER,
    'tyre_compounds_asset_group': f'{C.PACKAGE_FOLDER}/tyrecompounds/tyrecompounds',
    'blob_shadow_decal': f'eaid://{C.PACKAGE_FOLDER}/shared/ao/materials/blobshadowdecal.material',
    'has_cockpit_halo': 'False',
}

ERS_UPGRADES = [b'radUpgradeERS', b'radUpgradeWearES', b'radUpgradeWearMGUK']
DRS_REMOVED = [b'DistanceDrivenCumulativeDRSFailureChance', b'DRSRotationJointAngleRatio', b'DRSFailTimeMin',
               b'DRSFailTimeMax']
DRS_SET = {b'DRSRotationAngleRadians': '0.0', b'DRSOpenCloseSpeed': '1', b'm_aero_wing_front_drag': '1.0',
           b'm_aero_wing_front_force': '1.0', b'm_aero_wing_rear_drag': '1.0', b'm_aero_wing_rear_force': '1.0'}
ERS_ZERO = {b'MaxDeployPower': 0, b'MaxDeratePower': 0, b'MinInitialDeployPower': 0, b'HarvestLimitMGUK': 0,
            b'HarvestOverrideBonusMGUK': 0, b'HarvestLimitQualifyingMGUK': 0, b'DeployLimitMGUK': 0,
            b'MaxBrakeTorqueMGUK': 0, b'OverrideModeAvailable': 'false'}
ERS_VALUES = ('removed', 'zeroed')
TYRE_SIZES = ('13in', '13in_raised')
TYRES_13 = {'Front': {'radius': 0.323, 'width': 0.311}, 'Rear': {'radius': 0.330, 'width': 0.454}}
STOCK_MESH_WIDTH = {'Front': 0.339, 'Rear': 0.418}
BEAD_13 = 0.175
ALLOY_13 = 0.182
TYRE_GRAPHS = {b'x0_tyre_fl': 'Front', b'x0_tyre_fr': 'Front', b'x0_tyre_bl': 'Rear', b'x0_tyre_br': 'Rear'}


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
    rows = []
    for c in C.CARS:
        t = pack.teams[c.team]
        rows.append({'car': c, 'name': t.name, 'engine': c.family, 'power': t.power, 'weight': t.weight,
                     'drag': t.drag})
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


def remove_drs(root, aero):
    aero.children = [k for k in aero.children if not (k.kind == 0 and k.name in DRS_REMOVED)]
    for k, v in DRS_SET.items():
        set_value(aero, k, v)
    for c in root.children:
        if c.kind == 0 and c.name == b'Upgrade' and c.attr(b'name') == b'radUpgradeDRS':
            c.children = [k for k in c.children if k.kind != 0]


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


def convert(root, row, ers, size):
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
    remove_drs(root, aero)

    return {'team': row['name'], 'engine': row['engine'], 'hp': round(real_peak / HP_W),
            'limiter_rpm': fam['limiter_rpm'], 'mass': round(mass, 1), 'drag': round(drag, 3),
            'final_drive': round(final, 3)}


def stock_archive(root_dir, c):
    return os.path.join(root_dir, GROUPS, PACKAGE, 'teams', c.donor, 'wep', f'{c.donor}.erp')


def stock_vtf(root_dir, c):
    e = erpw.Erp(stock_archive(root_dir, c))
    r = e.find(f'eaid://{PACKAGE}/teams/{c.donor}/handling/{c.donor}.vtf')
    return e, r


def handling_archive(e, r, names, f0, f1):
    out = []
    for name in names:
        q = erpw.Res()
        q.name, q.type, q.unk, q.hash, q.frags = name, r.type, r.unk, r.hash, []
        for fr in r.frags:
            g = erpw.Frag()
            g.name, g.off, g.size, g.flags, g.comp, g.packed, g.raw = (fr.name, fr.off, fr.size, fr.flags, fr.comp,
                                                                       fr.packed, fr.raw)
            q.frags.append(g)
        out.append(q)
    e.res = out
    for q in out:
        e.set_frag(q, 0, f0)
        e.set_frag(q, 1, f1)
    tmp = os.path.join(ADDON, 'build', '.handling.erp')
    os.makedirs(os.path.dirname(tmp), exist_ok=True)
    e.to_file(tmp)
    with open(tmp, 'rb') as f:
        blob = f.read()
    os.remove(tmp)
    return blob


def build(game, out):
    root_dir = game_root(game)
    rows = cars()
    files, ops, report = {}, [], []
    for row in rows:
        c = row['car']
        e, r = stock_vtf(root_dir, c)
        f0, f1 = e.frag_data(r, 0), e.frag_data(r, 1)
        stock = vtf.parse(f0)
        version = int.from_bytes(f1[4:8], 'little')
        if vtf.write(stock) != f0 or vtf.tsmd(stock, version) != f1:
            raise ValueError(f'{c.team}: stock VTF of {c.donor} does not round-trip')
        for ers in ERS_VALUES:
            for tyre_size in TYRE_SIZES:
                root = vtf.parse(f0)
                got = convert(root, row, ers, tyre_size)
                n0 = vtf.write(root)
                n1 = vtf.tsmd(root, version)
                if ers == 'removed' and tyre_size == '13in':
                    info, size = got, len(n0)
                rel = f'cars/handling/{ers}_{tyre_size}/{c.folder}_handling.erp'
                files[rel] = handling_archive(e, r, C.handling_names(c), n0, n1)
                ops.append({'op': 'file.put', 'when': {'ers': ers, 'tyre_size': tyre_size}, 'source': rel,
                            'target': C.handling_archive(c)})
        info['car'] = f'{c.team}@{c.folder}'
        info['bytes'] = (len(f0), size)
        report.append(info)
    files['cars/handling.json'] = (json.dumps({'schema_version': 1, 'ops': ops}, indent=1) + '\n').encode()
    files['db/cars.json'] = (json.dumps(db_ops(), indent=1) + '\n').encode()
    for rel, blob in files.items():
        dst = os.path.join(out, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        with open(dst, 'wb') as f:
            f.write(blob)
    return files, report


def chassis_key(c):
    return f'lng_f1990_chassis_{c.team}'


def engine_key(c):
    return f'lng_f1990_engine_{c.team}'


def package_ops(name, stat_id, folder):
    values = dict(PACKAGE_SET, _metadata_=name, stat_id=stat_id,
                  resource_path=f'{folder}/teams/[FILENAME]/wep/[FILENAME]')
    return [{'op': 'add-row', 'table': 'entrants.vehicles.vehicle_packages',
             'from': f'entrants.vehicles.vehicle_packages.vehicle_package[_metadata_={C.SOURCE_PACKAGE_ROW}]',
             'set': values},
            {'op': 'set', 'select': f'entrants.vehicles.vehicle_packages.vehicle_package[_metadata_={name}]',
             'field': 'is_26_aero_regulations', 'value': 'False', 'when': {'classic_aero': True}}]


def db_ops():
    with open(VEHICLE_FIELDS) as f:
        vehicle_fields = json.load(f)['fields']['entrants.vehicles.vehicle']
    ops = package_ops(C.PACKAGE, C.PACKAGE_STAT_ID, C.PACKAGE_FOLDER)
    ops += package_ops(C.PACKAGE_B, C.PACKAGE_B_STAT_ID, C.PACKAGE_B_FOLDER)
    for c in C.CARS:
        ops.append({'op': 'add-row', 'table': 'entrants.vehicles',
                    'from': f'entrants.vehicles.vehicle[_metadata_={c.donor_vehicle}]',
                    'set': {'_metadata_': c.vehicle, 'stat_id': str(c.stat_id), 'asset_filename': c.folder,
                            'vehicle_package': C.PACKAGE, 'hud_dial': '{{hud_dial}}', 'drs_capable': 'False',
                            'display_name': f'lng_f1990_team_{c.team}', 'chassis_name': chassis_key(c),
                            'engine_name_override': engine_key(c), 'equal_performance_vehicle': '',
                            'performance_tier': str(c.tier)}})
    for c in C.CARS:
        ops.append({'op': 'add-row', 'table': 'entrants.vehicles',
                    'from': f'entrants.vehicles.vehicle[_metadata_={c.vehicle}]',
                    'set': {'_metadata_': c.vehicle_b, 'stat_id': str(c.stat_id_b), 'vehicle_package': C.PACKAGE_B}})
    return {'schema_version': 1,
            'fields': {'entrants.vehicles.vehicle_packages.vehicle_package': PACKAGE_FIELDS,
                       'entrants.vehicles.vehicle': vehicle_fields},
            'ops': ops}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    ap.add_argument('--game', help='the F1 25 folder with stock 2026 team archives')
    ap.add_argument('--write', action='store_true', help='replace the shipped files with the build')
    args = ap.parse_args()
    out = ADDON if args.write else os.path.join(ADDON, 'build')
    files, report = build(args.game, out)
    for r in report:
        print(f'{r["car"]:28} {r["team"]:22} {r["engine"]:4} {r["hp"]:4} hp  {r["limiter_rpm"]} rpm  '
              f'{r["mass"]:6} kg  drag x{r["drag"]}  final {r["final_drive"]}  {r["bytes"][0]} -> {r["bytes"][1]} bytes')
    if args.write:
        return 0
    diff = [rel for rel, blob in files.items()
            if not os.path.exists(os.path.join(ADDON, rel)) or open(os.path.join(ADDON, rel), 'rb').read() != blob]
    for rel in diff:
        print(f'differs: {rel}')
    return 1 if diff else 0


if __name__ == '__main__':
    sys.exit(main())
