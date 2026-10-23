import sys
from functools import lru_cache
from pathlib import Path

import numpy as np
from PIL import Image

ART = Path(__file__).resolve().parent.parent / "art/logos"

CARD = {
    "force_india": ("#F2F2F2", "light"),
    "racing_point": ("#F596C8", "light"),
    "renault": ("#FFD800", "light"),
    "toro_rosso": ("#0C1E4A", "dark"),
    "alphatauri": ("#0F1E36", "dark"),
    "sauber": ("#F2F2F2", "light"),
    "alfa_romeo": ("#8E0D14", "dark"),
    "manor": ("#F2F2F2", "light"),
}
KEEP_LIGHT = {"force_india", "racing_point", "sauber", "manor"}


def rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def lum(c):
    return (0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]) / 255


@lru_cache(None)
def art(token, kind):
    im = Image.open(ART / token / f"{kind}.png").convert("RGBA")
    a = np.asarray(im)
    edge = np.concatenate([a[0], a[-1], a[:, 0], a[:, -1]])
    if ((edge[:, :3].min(-1) > 230) & (edge[:, 3] > 200)).mean() > 0.5:
        sys.exit(f"{token}/{kind}.png: opaque white background, key it to alpha in logo_art.py")
    return im.crop(alpha_box(im))


def fit(im, w, h):
    s = min(w / im.width, h / im.height)
    nw, nh = max(1, round(im.width * s)), max(1, round(im.height * s))
    if s >= 1:
        return im.resize((nw, nh), Image.LANCZOS)
    a = np.asarray(im).astype(np.float32) / 255
    pre = Image.fromarray((np.dstack([a[..., :3] * a[..., 3:], a[..., 3:]]) * 255).round().astype(np.uint8), "RGBA")
    b = np.asarray(pre.resize((nw, nh), Image.LANCZOS, reducing_gap=3.0)).astype(np.float32) / 255
    al = b[..., 3:]
    col = np.where(al > 0, b[..., :3] / np.maximum(al, 1e-6), 0)
    return Image.fromarray((np.dstack([np.clip(col, 0, 1), al]) * 255).round().astype(np.uint8), "RGBA")


def place(canvas, logo, cx, cy):
    canvas.alpha_composite(logo, (round(cx - logo.width / 2), round(cy - logo.height / 2)))
    return canvas


def box_of(mask):
    ys, xs = np.where(mask)
    return xs.min(), ys.min(), xs.max() + 1, ys.max() + 1


def alpha_box(im):
    a = np.asarray(im.getchannel("A")) > 8
    return box_of(a) if a.any() else None


def white_box(im):
    a = np.asarray(im).astype(np.int32)
    return box_of((a[..., :3].min(-1) > 200) & (a[..., 3] > 128))


def colour_box(im):
    a = np.asarray(im.convert("RGB")).astype(np.int32)
    return box_of(np.abs(a - a[2, 2]).sum(-1) > 60)


def stock_size(slices):
    sizes = [(r - l, b - t) for l, t, r, b in filter(None, map(alpha_box, slices))]
    return float(np.median([w * h for w, h in sizes])), max(w for w, _ in sizes), max(h for _, h in sizes)


def profile(im, box, axis):
    a = np.asarray(im).astype(np.float32)
    keep = np.ones(a.shape[:2], bool)
    l, t, r, b = box
    keep[max(0, t - 6):b + 6, max(0, l - 6):r + 6] = False
    out = np.empty_like(a)
    if axis == "y":
        for y in range(a.shape[0]):
            out[y] = np.median(a[y][keep[y]] if keep[y].any() else a[y], 0)
    else:
        for x in range(a.shape[1]):
            out[:, x] = np.median(a[:, x][keep[:, x]] if keep[:, x].any() else a[:, x], 0)
    return Image.fromarray(np.clip(out, 0, 255).astype(np.uint8), "RGBA")


def tint(im, colour):
    a = np.asarray(im).astype(np.float32)
    lumin = (0.299 * a[..., 0] + 0.587 * a[..., 1] + 0.114 * a[..., 2]) / 255
    solid = a[..., 3] > 128
    ref = np.percentile(lumin[solid], 92) if solid.any() else 1
    shade = np.clip(lumin / max(ref, 1e-3), 0, 1.3)[..., None]
    col = np.array(colour, np.float32) * shade
    col += (255 - col) * np.clip(shade - 1, 0, 1)
    return Image.fromarray(np.dstack([np.clip(col, 0, 255), a[..., 3:]]).astype(np.uint8), "RGBA")


def on_colour(token, out, logo_box, centre):
    x, y = centre
    px = np.asarray(out)[min(round(y), out.height - 1), min(round(x), out.width - 1)].astype(np.float32)
    local = lum(px[:3]) * px[3] / 255
    kind = "lockup_light" if local > 0.55 or token in KEEP_LIGHT else (
        "lockup_" + CARD[token][1] if local > 0.3 else "lockup_dark")
    return place(out, fit(art(token, kind), *logo_box), x, y)


class Surface:
    def __init__(self, style, stock, ref):
        self.style = style
        self.stock = stock
        self.ref = ref
        self.template = getattr(self, "prepare_" + style)()

    def prepare_mark(self):
        return stock_size([im for k, im in self.stock.items() if k != self.ref or len(self.stock) == 1])

    def prepare_card(self):
        return colour_box(self.stock[self.ref])

    def prepare_mp(self):
        st = self.stock[self.ref]
        box = white_box(st)
        return profile(st, box, "y"), box

    def render(self, token, w, h):
        return getattr(self, "render_" + self.style)(token, w, h)

    def render_mark(self, token, w, h):
        area, most_w, most_h = self.template
        logo = art(token, "mark")
        s = (area / (logo.width * logo.height)) ** 0.5
        box = min(logo.width * s, most_w, w * 0.96), min(logo.height * s, most_h, h * 0.96)
        return place(Image.new("RGBA", (w, h), (0, 0, 0, 0)), fit(logo, *box), w / 2, h / 2)

    def render_card(self, token, w, h):
        l, t, r, b = self.template
        out = Image.new("RGBA", (w, h), rgb(CARD[token][0]) + (255,))
        return place(out, fit(art(token, "lockup_" + CARD[token][1]), (r - l) * 1.05, (b - t) * 1.15),
                     (l + r) / 2, (t + b) / 2)

    def render_mp(self, token, w, h):
        bg, (l, t, r, b) = self.template
        box, centre = ((r - l) * 1.05, (b - t) * 1.2), ((l + r) / 2, (t + b) / 2)
        return on_colour(token, tint(bg, rgb(CARD[token][0])), box, centre)
