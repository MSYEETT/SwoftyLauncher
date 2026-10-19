import argparse
import os

import numpy as np
from PIL import Image
from scipy import ndimage

SIZE = 240
WIRE_GREY = 128
WIRE_GAIN = 1.0
MASK_GROW = 1.6
MASK_FEATHER = 1.4


def load(path):
    return np.asarray(Image.open(path).convert('RGBA')).astype(np.float64) / 255.0


def down(rgba):
    a = rgba[..., 3:4]
    pre = np.concatenate([rgba[..., :3] * a, a], axis=-1)
    img = Image.fromarray(np.clip(pre * 255 + 0.5, 0, 255).astype(np.uint8), 'RGBA')
    small = np.asarray(img.resize((SIZE, SIZE), Image.Resampling.LANCZOS)).astype(np.float64) / 255.0
    a = small[..., 3:4]
    rgb = np.where(a > 1e-4, small[..., :3] / np.maximum(a, 1e-4), 0)
    return np.concatenate([np.clip(rgb, 0, 1), np.clip(a, 0, 1)], axis=-1)


def to_img(rgba):
    out = np.clip(rgba * 255 + 0.5, 0, 255).astype(np.uint8)
    out[out[..., 3] == 0, :3] = 0
    return Image.fromarray(out, 'RGBA')


def wire_frames(wire):
    w = down(wire)
    lum = w[..., :3].mean(-1) * w[..., 3]
    alpha = np.clip(lum * WIRE_GAIN, 0, 1)
    base = np.zeros((SIZE, SIZE, 4))
    base[..., :3] = WIRE_GREY / 255.0
    base[..., 3] = alpha
    prog = base.copy()
    prog[..., :3] = 1.0
    return base, prog


def mask_frame(beauty):
    b = down(beauty)
    sil = b[..., 3] > 0.5
    dist = ndimage.distance_transform_edt(~sil)
    soft = np.clip(1.0 - (dist - MASK_GROW) / 2.0, 0, 1)
    soft = ndimage.gaussian_filter(soft, MASK_FEATHER)
    soft[sil] = 1.0
    m = np.zeros((SIZE, SIZE, 4))
    m[..., :3] = 1.0
    m[..., 3] = soft
    return m


def build(work, out, name):
    beauty = load(os.path.join(work, name + '_beauty.png'))
    wire = load(os.path.join(work, name + '_wire.png'))
    base, prog = wire_frames(wire)
    frames = {
        f'om_xm_{name}': base,
        f'om_xm_{name}_progress': prog,
        f'om_xm_{name}_complete': down(beauty),
        f'om_xm_{name}_m': mask_frame(beauty),
    }
    for frame, px in frames.items():
        to_img(px).save(os.path.join(out, frame + '.png'), optimize=True)
    return list(frames)


def main():
    ap = argparse.ArgumentParser(description='Turn the raw Blender passes into the four 240x240 card frames.')
    ap.add_argument('work')
    ap.add_argument('out')
    ap.add_argument('names', nargs='+')
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    for n in a.names:
        print(n, ' '.join(build(a.work, a.out, n)))


if __name__ == '__main__':
    main()
