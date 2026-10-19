import os
import shutil
import subprocess
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
TYPES = ['tyre_thermal', 'brake_cooling', 'tyre_life', 'pu_cooling']


def resvg():
    exe = shutil.which('resvg')
    return [exe] if exe else ['nix', 'run', 'nixpkgs#resvg', '--']


def main(out):
    os.makedirs(out, exist_ok=True)
    for t in TYPES:
        dst = os.path.join(out, t + '.png')
        subprocess.run(resvg() + ['-w', '32', '-h', '32', os.path.join(HERE, 'icons', t + '.svg'), dst], check=True,
                       capture_output=True)
        px = np.asarray(Image.open(dst).convert('RGBA')).copy()
        px[..., :3] = 255
        px[px[..., 3] == 0, :3] = 0
        Image.fromarray(px, 'RGBA').save(dst, optimize=True)
        print(t)


if __name__ == '__main__':
    main(sys.argv[1])
