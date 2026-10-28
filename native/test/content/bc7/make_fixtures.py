#!/usr/bin/env python3
import math
import random
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[4]
sys.path[:0] = [str(ROOT / "addons/classicseasons/classic-seasons/tools"), str(ROOT / "tools"),
                str(ROOT / "addons/rnd-tyres-thermal/tools")]
import logos

OUT = Path(__file__).resolve().parent


def noise(w, h, seed, alpha_levels):
    rng = random.Random(seed)
    data = bytearray()
    for _ in range(w * h):
        data += bytes([rng.randrange(256), rng.randrange(256), rng.randrange(256), rng.choice(alpha_levels)])
    return Image.frombytes("RGBA", (w, h), bytes(data))


def shape(w, h):
    im = Image.new("RGBA", (w * 4, h * 4), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.ellipse((w * 0.4, h * 0.4, w * 3.6, h * 3.6), fill=(210, 20, 40, 255))
    d.polygon([(w * 0.8, h * 3.2), (w * 2, h * 0.6), (w * 3.2, h * 3.2)], fill=(250, 250, 250, 255))
    d.rectangle((w * 1.7, h * 1.4, w * 2.3, h * 2.8), fill=(10, 20, 120, 255))
    return im.resize((w, h), Image.LANCZOS)


def gradient(w, h):
    a = np.zeros((h, w, 4), dtype=np.uint8)
    for y in range(h):
        for x in range(w):
            a[y, x] = (x * 255 // (w - 1), y * 255 // (h - 1), (x + y) * 255 // (w + h - 2), 255 - (x * y * 255) // ((w - 1) * (h - 1)))
    return Image.fromarray(a, "RGBA")


def text(w, h):
    im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.text((2, h // 4), "F1", fill=(255, 255, 255, 255), font=ImageFont.load_default(size=h // 2))
    d.line((0, h - 3, w, 3), fill=(0, 160, 80, 200), width=3)
    return im


def clear_varied(w, h):
    rng = random.Random(7)
    im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    px = im.load()
    for y in range(h):
        for x in range(w):
            if (x // 4 + y // 4) % 3 == 0:
                px[x, y] = (rng.randrange(256), rng.randrange(256), rng.randrange(256), 0)
            elif (x // 4 + y // 4) % 3 == 1:
                px[x, y] = (200, 30, 30, 255)
            else:
                px[x, y] = (rng.randrange(256), 40, 200, rng.choice((0, 255)))
    return im


CASES = {
    "shape64": shape(64, 64),
    "noise32": noise(32, 32, 1, [255]),
    "alpha32": noise(32, 32, 2, [0, 1, 17, 128, 254, 255]),
    "gradient48": gradient(48, 48),
    "text64x32": text(64, 32),
    "flatmix32": clear_varied(32, 32),
}


def stock_array(w, h, count):
    import struct
    head = bytearray(40)
    struct.pack_into("<III", head, 8, 70, w, h)
    struct.pack_into("<II", head, 24, 1, count)
    data = bytes((i * 7 + 3) & 255 for i in range(count * w * h))
    view = bytearray(48)
    struct.pack_into("<I", view, 0x14, count)
    return bytes(head), data, bytes(view)


def grown_array(head, data, view, count, added):
    import struct
    head2 = bytearray(head)
    struct.pack_into("<I", head2, 28, count + len(added))
    view2 = bytearray(view)
    struct.pack_into("<I", view2, 0x14, count + len(added))
    return bytes(head2), data + b"".join(added), bytes(view2)


def main():
    for name, im in CASES.items():
        im = im.convert("RGBA")
        a = np.array(im)
        a[a[..., 3] == 0] = 0
        im = Image.fromarray(a, "RGBA")
        (OUT / f"{name}.rgba").write_bytes(f"{im.width} {im.height}\n".encode() + im.tobytes())
        _, _, data = logos.encode(("x", "x", im))
        (OUT / f"{name}.bc7").write_bytes(data)
        print(name, im.size, len(data))
    head, data, view = stock_array(64, 64, 3)
    added = [(OUT / "shape64.bc7").read_bytes(), (OUT / "shape64.bc7").read_bytes()[::-1]]
    g = grown_array(head, data, view, 3, added)
    for n, b in zip(("head", "data", "view"), (head, data, view)):
        (OUT / f"stock_{n}.bin").write_bytes(b)
    for n, b in zip(("head", "data", "view"), g):
        (OUT / f"grown_{n}.bin").write_bytes(b)


if __name__ == "__main__":
    main()
