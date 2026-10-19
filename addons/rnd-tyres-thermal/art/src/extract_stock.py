import argparse
import os
import struct
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.join(os.path.dirname(os.path.dirname(HERE)), 'tools')
sys.path.insert(0, TOOLS)

import common
from pkg import Pkg

ARRAYS = 'eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.career.'
OVERRIDES = 'eaid://ui_package/databases/overrides.uidb'
PARTS = ['redesigned_brake_ducts', 'brake_disc_cooling_structure', 'calipers', 'brake_duct_winglets', 'pad_material',
         'tyre_analytics_data', 'tyre_blankets', 'lightweight_wheel_rims', 'suspension_geometry',
         'streamlined_suspension_arms', 'ers_cooling', 'intercooler_power', 'energy_store_cooling',
         'side_pod_profile_drag', 'turbo_charger_wear_1']
SOURCES = [
    ('common_flow_misc.erp', 'rnd_om_xm.rnd_part_image',
     [f'om_xm_{p}{s}' for p in PARTS for s in ('', '_progress', '_complete')]),
    ('flow_r_and_d.erp', 'rnd_om_xm.rnd_part_mask', [f'om_xm_{p}_m' for p in PARTS]),
    ('flow_career_hub.erp', 'rnd_om_xm.rnd_category_icons',
     ['chassis_brakes', 'chassis_tyre_wear', 'maintenance_general_maintenance', 'maintenance_energy_store']),
    ('flow_r_and_d.erp', 'rnd_om_xm.rnd_card_bg', ['part_available', 'part_in_progress', 'part_complete', 'department']),
]


def entries(db, lst):
    node = Pkg(db.read(OVERRIDES)).root['overrides']['Contents']['career']['Contents']
    for part in lst.split('.'):
        node = node[part]
        if part != lst.split('.')[-1]:
            node = node['Contents']
    return [e['Name'].value for e in node['Contents'].items]


def main():
    ap = argparse.ArgumentParser(description='Decode the stock R&D frames the preview compares against.')
    common.add_game_arg(ap)
    ap.add_argument('--out', default=os.path.join(os.path.dirname(HERE), 'stock'))
    a = ap.parse_args()
    db = common.stock(a.game, 'databases.erp')
    os.makedirs(a.out, exist_ok=True)
    for erp, lst, wanted in SOURCES:
        arc = common.stock(a.game, erp)
        image = ARRAYS + lst + '.texarray.image'
        head = arc.read(image, 0)
        fmt, w, h = struct.unpack_from('<III', head, 8)
        slices = struct.unpack_from('<I', head, 28)[0]
        names = entries(db, lst)
        if fmt != common.BC7_FORMAT or len(names) != slices:
            sys.exit(f'{lst}: unexpected array (format {fmt}, {slices} slices, {len(names)} names)')
        data = arc.read(image, 1)
        for name in wanted:
            i = names.index(name)
            px = common.bc7_decode(data[i * w * h:(i + 1) * w * h], w, h)
            Image.fromarray(np.frombuffer(px, np.uint8).reshape(h, w, 4), 'RGBA').save(os.path.join(a.out, name + '.png'))
        print(f'{lst}: {len(wanted)} of {slices} slices ({w}x{h})')


if __name__ == '__main__':
    main()
