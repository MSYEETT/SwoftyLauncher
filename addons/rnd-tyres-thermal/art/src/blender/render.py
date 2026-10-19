import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bpy

import lib
import parts

argv = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
out = argv[0]
size = int(argv[1])
names = argv[2:] or list(parts.PARTS)
os.makedirs(out, exist_ok=True)

for name in names:
    lib.MATS.clear()
    lib.reset()
    fn, opts = parts.PARTS[name]
    fn()
    lib.setup_render(size)
    lib.world()
    cam, radius = lib.camera(fill=opts.get('fill', 0.80), lens=opts.get('lens', 70.0),
                             offset=opts.get('offset', (0.0, 0.0)))
    lib.lights(scale=max(radius, 0.12) / 0.25)
    if opts.get('blend'):
        bpy.ops.wm.save_as_mainfile(filepath=os.path.join(out, name + '.blend'))
    lib.render_beauty(os.path.join(out, name + '_beauty.png'))
    lib.render_wire(os.path.join(out, name + '_wire.png'), thickness=opts.get('line', 1.1),
                    outline=opts.get('outline', 1.7))
    print('RENDERED', name)
