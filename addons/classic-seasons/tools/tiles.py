#!/usr/bin/env python3
import argparse
import json
import struct
import sys
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))

from logos import ARRAY, BC7, DATABASES, OVERRIDES, carriers, overrides_list, write_archive  # noqa: E402
from logos import Pkg, Stock, bc7_decode, fragments  # noqa: E402

PATH = "home_screen.tiles_solo_6x6"
TEXTURE = "ui_package/art/textures/classic_seasons/tiles/{}.tif"
SRC = ROOT / "art/tiles/src"
OUT = ROOT / "erp/tiles"
BC7_DXGI = (98, 99)


def season_tiles():
    data = json.loads((ROOT / "data/seasons.json").read_text())
    return [(f"cs_f1_{s['year']}", SRC / f"{s['year']}.dds") for s in data["seasons"]]


def dds_blocks(path, w, h):
    b = path.read_bytes()
    dh, dw = struct.unpack_from("<II", b, 12)
    mips = struct.unpack_from("<I", b, 28)[0]
    if b[:4] != b"DDS " or b[84:88] != b"DX10":
        sys.exit(f"{path}: not a DX10 DDS")
    dxgi, _, _, layers = struct.unpack_from("<IIII", b, 128)
    if dxgi not in BC7_DXGI or (dw, dh) != (w, h) or mips > 1 or layers != 1 or len(b) - 148 != w * h:
        sys.exit(f"{path}: DXGI {dxgi}, {dw}x{dh}, {mips} mips, {layers} layers; the array wants BC7 {w}x{h}, 1 mip")
    return b[148:]


def build(game, baseline, previews):
    stock = Stock(game, baseline)
    image, view = ARRAY.format(PATH) + ".image", ARRAY.format(PATH)
    where = carriers(game, stock, [image, view])
    rels = where[image]
    if not rels or rels != where[view]:
        sys.exit(f"{PATH}: carriers {rels} / {where[view]}")
    stocks = {tuple(fragments(stock.path(rel), image) + fragments(stock.path(rel), view)) for rel in rels}
    if len(stocks) != 1:
        sys.exit(f"{PATH}: carriers hold different stock arrays")
    head, data, view0 = stocks.pop()
    fmt, w, h = struct.unpack_from("<III", head, 8)
    mips, count = struct.unpack_from("<II", head, 24)
    listed = overrides_list(Pkg(fragments(stock.path(DATABASES), OVERRIDES)[0]).root, PATH)
    if fmt != BC7 or mips != 1 or len(data) != count * w * h or len(listed) != count \
            or struct.unpack_from("<I", view0, 0x14)[0] != count:
        sys.exit(f"{PATH}: unexpected stock array (format {fmt}, {w}x{h}, {mips} mips, {count} slices)")
    tiles = season_tiles()
    taken = [k for k, _ in tiles if k in listed]
    if taken:
        sys.exit(f"{PATH}: already lists {taken}")

    added = [dds_blocks(src, w, h) for _, src in tiles]
    head2 = bytearray(head)
    struct.pack_into("<I", head2, 28, count + len(added))
    view2 = bytearray(view0)
    struct.pack_into("<I", view2, 0x14, count + len(added))
    if OUT.exists():
        for p in OUT.iterdir():
            p.unlink()
    OUT.mkdir(parents=True, exist_ok=True)
    write_archive(stock, {PATH: (rels,)}, {PATH: (bytes(head2), data + b"".join(added), bytes(view2))},
                  OUT / "tiles.erpk")

    ops = [{"op": "put-resource", "erp": rel, "resource": res, "source": "erp/tiles/tiles.erpk"}
           for rel in rels for res in (image, view)]
    at = "overrides.Contents." + ".Contents.".join(PATH.split(".")) + ".Contents"
    edits = [{"op": "clone-entry", "at": at, "src": f"[Name={listed[-1]}]", "as": key,
              "set": {"Value": TEXTURE.format(key)}} for key, _ in tiles]
    ops.append({"op": "patch-pkg", "erp": DATABASES, "resource": OVERRIDES, "fragment": 0, "edits": edits})
    (ROOT / "erp/tiles.json").write_text(json.dumps({"schema_version": 1, "ops": ops}, indent=1) + "\n")
    if previews:
        write_previews(tiles, count, w, h, previews)
    return [f"{PATH}: {w}x{h}, {count} + {len(added)} slices, {len(rels)} archive(s): {', '.join(rels)}"]


def write_previews(tiles, count, w, h, out):
    image = ARRAY.format(PATH) + ".image"
    head, data = fragments(OUT / "tiles.erpk", image)
    if struct.unpack_from("<I", head, 28)[0] != count + len(tiles) or len(data) != (count + len(tiles)) * w * h:
        sys.exit(f"{OUT / 'tiles.erpk'}: written array does not hold {count + len(tiles)} slices")
    out.mkdir(parents=True, exist_ok=True)
    for i, (key, _) in enumerate(tiles, count):
        px = bc7_decode(data[i * w * h:(i + 1) * w * h], w, h)
        Image.frombytes("RGBA", (w, h), px).convert("RGB").save(out / f"{key}.png")


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", required=True, type=Path)
    ap.add_argument("--baseline", type=Path, help="launcher baseline folder holding baseline.json (stock copies)")
    ap.add_argument("--previews", type=Path, help="decode the added slices from the written archive into PNGs here")
    a = ap.parse_args(argv[1:])
    for line in build(a.game, a.baseline, a.previews):
        print(line)


if __name__ == "__main__":
    main(sys.argv)
