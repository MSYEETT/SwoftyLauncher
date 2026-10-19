import json
import os

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ART = os.path.dirname(HERE)
ARRAYS = 'eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.career.'
PART = {
    'list': 'overrides.career.rnd_om_xm.rnd_part_image',
    'texarray': ARRAYS + 'rnd_om_xm.rnd_part_image.texarray',
    'archives': ['common_flow_misc.erp', 'flow_team_hq.erp'],
    'source_tif': 'ui_package/art/textures/tiles/tiles_rnd_om_xm/rnd_part_image/{frame}.tif',
}
MASK = {
    'list': 'overrides.career.rnd_om_xm.rnd_part_mask',
    'texarray': ARRAYS + 'rnd_om_xm.rnd_part_mask.texarray',
    'archives': ['flow_r_and_d.erp', 'flow_team_hq.erp'],
    'source_tif': 'ui_package/art/textures/tiles/tiles_rnd_om_xm/rnd_part_mask/{frame}.tif',
}
PLAIN_PART = {'list': 'overrides.career.rnd_part_image', 'texarray': ARRAYS + 'rnd_part_image.texarray',
              'archives': ['common_flow_misc.erp', 'flow_team_hq.erp']}
PLAIN_MASK = {'list': 'overrides.career.rnd_part_mask', 'texarray': ARRAYS + 'rnd_part_mask.texarray',
              'archives': ['flow_r_and_d.erp', 'flow_team_hq.erp']}
ICON = {'list': 'overrides.career.rnd_om_xm.rnd_category_icons',
        'texarray': ARRAYS + 'rnd_om_xm.rnd_category_icons.texarray',
        'archives': ['flow_career_hub.erp', 'flow_team_hq.erp'],
        'plain_list': 'overrides.career.rnd_category_icons',
        'plain_archives': ['flow_career_hub.erp', 'flow_r_and_d.erp', 'flow_team_hq.erp'],
        'source_tif': 'ui_package/art/textures/career/rnd_tree/category_icons/{frame}.tif'}
STATES = [
    ('', 'available', '', 'grey (128) hidden-line wireframe, alpha = line coverage'),
    ('_progress', 'in_progress', '_in_progress', 'the same wireframe in white (255), identical alpha'),
    ('_complete', 'complete', '_complete', 'PBR render on transparent background'),
]


def entry(path, spec, extra):
    w, h = Image.open(os.path.join(ART, path)).size
    d = {'file': path, 'size': [w, h], 'pixel_format': 'RGBA8 (stock arrays store BC7, format 70, 1 mip)'}
    d.update(spec)
    d.update(extra)
    if 'source_tif' in d:
        d['source_tif'] = d['source_tif'].format(frame=extra.get('frame', ''))
    d.pop('frame', None)
    return d


def main():
    tree = json.load(open(os.path.join(HERE, 'upgrades.json')))
    frames = {}
    for br in tree['branches']:
        for name, display, size, _desc in br['upgrades']:
            for suffix, state, plain, look in STATES:
                frame = f'om_xm_{name}{suffix}'
                frames[frame] = entry(f'out/{frame}.png', PART, {
                    'frame': frame, 'upgrade': name, 'state': state, 'look': look,
                    'plain_list_frame': f'{name}{plain}', 'plain_list': PLAIN_PART})
            frame = f'om_xm_{name}_m'
            frames[frame] = entry(f'out/{frame}.png', MASK, {
                'frame': frame, 'upgrade': name, 'state': 'mask',
                'look': 'white, alpha = silhouette of the complete render grown ~1.6 px and feathered',
                'plain_list_frame': name, 'plain_list': PLAIN_MASK})
        key = br['key']
        frames[key] = entry(f'out/{key}.png', ICON, {
            'frame': key, 'category': br['type'],
            'look': 'white two-tone glyph: main shape alpha 255, secondary alpha 128',
            'note': f'stock frames are named <department>_<type> (chassis_tyre_wear); alias as tyres_{key} if the '
                    'category-row hook builds that name'})
    out = {
        'schema': 1,
        'notes': [
            'The stock card has four frames per part: <name> (available), <name>_progress, <name>_complete and the '
            'mask <name>_m. The game builds <pkg>.rnd_part_image.<upgrade_name>[_progress|_complete] and '
            '<pkg>.rnd_part_mask.<upgrade_name>_m; the plain (non om_xm) lists use _in_progress and an unsuffixed mask.',
            'Every frame is one slice of a texture array: append the slice to the .texarray.image pixels, bump the '
            'slice count in the image head (offset 28) and the SRV view (offset 0x14), and clone an overrides.uidb '
            'entry with Name = frame, Value = source_tif, as tools/build_icons.py does.',
        ],
        'frames': frames,
    }
    with open(os.path.join(ART, 'manifest.json'), 'w') as f:
        json.dump(out, f, indent=2)
        f.write('\n')
    print(len(frames), 'frames')


if __name__ == '__main__':
    main()
