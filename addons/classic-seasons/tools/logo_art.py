#!/usr/bin/env python3
import argparse
import io
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
ART = HERE.parent / "art/logos"
SRC = ART / "src"
LOCKUP_W = 1200
MARK_W = 600
KINDS = ("lockup_light", "lockup_dark", "mark")
SAUBER_RING = (-2587.1, 1024.7, 722.9)
SAUBER_TAIL = ("v -400.3078", "A 679 679 0 0 1 -3263.3,973.6 L -3043.1801,1126.9983")
SOURCES = {
    "force_india.svg": "https://en.wikipedia.org/wiki/File:Force_India.svg",
    "racing_point.svg": "https://commons.wikimedia.org/wiki/File:Racing_Point.svg",
    "racing_point_bwt.svg": "https://commons.wikimedia.org/wiki/File:BWT_Racing_Point_Logo.svg",
    "renault.svg": "https://en.wikipedia.org/wiki/File:Renault_F1_Team_logo_2019.svg",
    "toro_rosso.svg": "https://en.wikipedia.org/wiki/File:Scuderia_Toro_Rosso_logo.svg",
    "alphatauri.svg": "https://en.wikipedia.org/wiki/File:Scuderia_Alpha-Tauri.svg",
    "sauber.svg": "https://commons.wikimedia.org/wiki/File:Sauber_F1_Team_logo.svg",
    "alfa_romeo.svg": "https://commons.wikimedia.org/wiki/File:AlfaRomeoF1Stake.svg",
}


def hexrgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rasterise(text, width):
    with tempfile.TemporaryDirectory() as d:
        src, out = os.path.join(d, "in.svg"), os.path.join(d, "out.png")
        Path(src).write_text(text)
        subprocess.run(["resvg", "-w", str(width), src, out], check=True, capture_output=True)
        return Image.open(out).convert("RGBA").copy()


def render(name, width, swaps=()):
    text = (SRC / name).read_text()
    for old, new in swaps:
        text = text.replace(old, new)
    return rasterise(text, width)


def trim(im, pad=2):
    box = im.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
    if box is None:
        return im
    l, t, r, b = box
    return im.crop((max(0, l - pad), max(0, t - pad), min(im.width, r + pad), min(im.height, b + pad)))


def crop(im, l, t, r, b):
    return trim(im.crop((round(im.width * l), round(im.height * t), round(im.width * r), round(im.height * b))))


def recolour(im, mapping, tol=90):
    a = np.asarray(im).astype(np.float32)
    rgb = a[..., :3].copy()
    src = np.array([hexrgb(s) for s, _ in mapping], np.float32)
    dst = np.array([hexrgb(d) for _, d in mapping], np.float32)
    dist = np.sqrt(((rgb[..., None, :] - src[None, None]) ** 2).sum(-1))
    near = dist.min(-1) < tol
    rgb[near] = dst[dist.argmin(-1)[near]]
    return Image.fromarray(np.dstack([rgb, a[..., 3:]]).astype(np.uint8), "RGBA")


def to_white(im, *colours, tol=90):
    return recolour(im, [(c, "#FFFFFF") for c in colours], tol)


def drop(im, *colours, tol=60):
    a = np.asarray(im).copy()
    for c in colours:
        a[np.sqrt(((a[..., :3].astype(np.float32) - hexrgb(c)) ** 2).sum(-1)) < tol, 3] = 0
    return Image.fromarray(a, "RGBA")


def force_india():
    logo = trim(render("force_india.svg", LOCKUP_W))
    dark = to_white(logo, "#737375", "#383336", tol=60)
    stemless = [("c-8.634 0.952-11.734 3.418-15.717 6.622l-39.706 45.08 27.506-46.533", "l-27.917 5.169"),
                ("-78.644 24.252-69.035 8.332z", "-70 14-69.035 8.332z")]
    flag = render("force_india.svg", LOCKUP_W, stemless)
    return logo, dark, trim(drop(crop(flag, 0, 0, 0.37, 1), "#737375", "#737378"))


def racing_point():
    logo = trim(render("racing_point_bwt.svg", LOCKUP_W))
    return logo, to_white(logo, "#05233d", tol=60), trim(render("racing_point.svg", MARK_W))


def renault():
    logo = trim(render("renault.svg", LOCKUP_W))
    white = to_white(logo, "#000000", tol=120)
    return logo, white, crop(white, 0, 0, 0.31, 1)


def toro_rosso():
    logo = trim(render("toro_rosso.svg", LOCKUP_W))
    return logo, logo, crop(logo, 0.35, 0, 0.71, 0.45)


def alphatauri():
    logo = trim(render("alphatauri.svg", LOCKUP_W // 2))
    white = to_white(logo, "#011321", tol=120)
    return logo, white, crop(white, 0, 0.13, 1, 0.80)


def sauber_s(text):
    d = re.search(r'id="path13".*?\sd="([^"]*)"', text, re.S).group(1)
    inside = "M" + d[1:d.index(" m -703.8525")].replace(*SAUBER_TAIL)
    cx, cy, r = SAUBER_RING
    ring = f"M {cx - r},{cy} a {r},{r} 0 1 0 {2 * r},0 a {r},{r} 0 1 0 {-2 * r},0 z"
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{cx - r - 4} {cy - r - 4} {2 * r + 8} {2 * r + 8}">'
            f'<path fill="#fff" fill-rule="evenodd" d="{ring} {inside}"/></svg>')


def sauber():
    text = (SRC / "sauber.svg").read_text()
    bare = rasterise(text.replace("fill:#ffffff;fill-opacity:1", "fill:#ffffff;fill-opacity:0"), LOCKUP_W)
    a = np.asarray(bare).astype(np.int32)
    red = (np.abs(a[..., :3] - hexrgb("#d0180d")).sum(-1) < 90) & (a[..., 3] > 128)
    l, t, r, b = Image.fromarray(red.astype(np.uint8) * 255).getbbox()
    logo = Image.new("RGBA", bare.size, (0, 0, 0, 0))
    logo.paste((255, 255, 255, 255), (l + 1, t + 1, r - 1, b - 1))
    logo.alpha_composite(bare)
    logo = trim(logo)
    return logo, to_white(logo, "#000000", tol=120), trim(rasterise(sauber_s(text), MARK_W))


def alfa_romeo():
    logo = trim(render("alfa_romeo.svg", LOCKUP_W))
    white = to_white(logo, "#221F20", tol=120)
    return logo, white, crop(white, 0, 0, 0.58, 1)


TEAMS = {f.__name__: f for f in (force_india, racing_point, renault, toro_rosso, alphatauri, sauber, alfa_romeo)}


def fit_width(im, width):
    im = trim(im)
    if im.width <= width:
        return im
    return im.resize((width, max(1, round(im.height * width / im.width))), Image.LANCZOS)


def png(im):
    b = io.BytesIO()
    im.save(b, "PNG", optimize=True)
    return b.getvalue()


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="exit 1 when a shipped file differs from a fresh build")
    a = ap.parse_args(argv[1:])
    stale = []
    for team, make in TEAMS.items():
        for kind, im in zip(KINDS, make()):
            data = png(fit_width(im, MARK_W if kind == "mark" else LOCKUP_W))
            path = ART / team / f"{kind}.png"
            if a.check:
                if not path.exists() or path.read_bytes() != data:
                    stale.append(str(path.relative_to(ART)))
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        print(team)
    if stale:
        sys.exit("stale: " + ", ".join(stale))


if __name__ == "__main__":
    main(sys.argv)
