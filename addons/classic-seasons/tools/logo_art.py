#!/usr/bin/env python3
import argparse
import io
import os
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


def render(name, width):
    with tempfile.TemporaryDirectory() as d:
        out = os.path.join(d, "out.png")
        subprocess.run(["resvg", "-w", str(width), str(SRC / name), out], check=True, capture_output=True)
        return Image.open(out).convert("RGBA").copy()


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


def key_white(im, lo=30, hi=90):
    a = np.asarray(im).astype(np.float32)
    dist = np.sqrt(((a[..., :3] - 255) ** 2).sum(-1))
    alpha = np.clip((dist - lo) / (hi - lo), 0, 1) * a[..., 3] / 255
    f = np.where(alpha > 0, alpha, 1)[..., None]
    rgb = np.clip((a[..., :3] - (1 - f) * 255) / f, 0, 255)
    return Image.fromarray(np.dstack([rgb, alpha * 255]).astype(np.uint8), "RGBA")


def force_india():
    logo = trim(render("force_india.svg", LOCKUP_W))
    dark = to_white(logo, "#737375", "#383336", tol=60)
    return logo, dark, crop(logo, 0, 0, 0.27, 1)


def racing_point():
    logo = trim(render("racing_point_bwt.svg", LOCKUP_W))
    return logo, to_white(logo, "#05233d", tol=60), trim(render("racing_point.svg", MARK_W))


def renault():
    logo = trim(render("renault.svg", LOCKUP_W))
    white = to_white(logo, "#000000", tol=120)
    return logo, white, crop(white, 0, 0, 0.21, 1)


def toro_rosso():
    logo = trim(render("toro_rosso.svg", LOCKUP_W))
    return logo, logo, crop(logo, 0.35, 0, 0.66, 0.45)


def alphatauri():
    logo = trim(render("alphatauri.svg", LOCKUP_W // 2))
    white = to_white(logo, "#011321", tol=120)
    return logo, white, crop(white, 0, 0.13, 1, 0.80)


def sauber():
    logo = trim(render("sauber.svg", LOCKUP_W))
    keyed = trim(key_white(logo))
    return logo, to_white(keyed, "#000000", tol=120), crop(logo, 0, 0, 0.245, 1)


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
