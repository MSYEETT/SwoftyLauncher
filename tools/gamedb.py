#!/usr/bin/env python3
import argparse
import json
import os
import struct
import sys

MASK = b"tG5MYmBZEZZnrH7yg2qTC4qj42Jcq9"
MASK_START = 10
FNV_BASIS = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3
FNV_PRIME_INV = pow(FNV_PRIME, -1, 1 << 64)
U64 = (1 << 64) - 1

TYPE_NAMES = {
    0: "int",
    1: "float",
    2: "string",
    3: "enum",
    4: "id",
    5: "bool",
    6: "time",
    7: "t7",
    8: "t8",
    9: "curve",
    11: "t11",
    12: "t12",
    13: "list",
    14: "vector",
    17: "t17",
    18: "t18",
}


def fnv1a64(data, h=FNV_BASIS):
    if isinstance(data, str):
        data = data.encode("utf-8")
    for c in data:
        h = ((h ^ c) * FNV_PRIME) & U64
    return h


def fnv1a64_unwind(h, suffix):
    if isinstance(suffix, str):
        suffix = suffix.encode("utf-8")
    for c in reversed(suffix):
        h = ((h * FNV_PRIME_INV) & U64) ^ c
    return h


def _mask_stream(length, pos):
    n = len(MASK)
    rep = MASK[pos % n:] + MASK * ((length // n) + 2)
    return rep[:length]


def unmask(enc, pos):
    m = _mask_stream(len(enc), MASK_START + pos)
    return bytes((a - b) & 0xFF for a, b in zip(enc, m))


def mask(plain, pos):
    m = _mask_stream(len(plain), MASK_START + pos)
    return bytes((a + b) & 0xFF for a, b in zip(plain, m))


class Record:
    __slots__ = ("key", "type", "value")

    def __init__(self, key, typ, value):
        self.key = key
        self.type = typ
        self.value = value

    @property
    def key_hi(self):
        return self.key >> 32

    @property
    def key_lo(self):
        return self.key & 0xFFFFFFFF


def read(data):
    count = struct.unpack_from("<I", data, 0)[0]
    p = 4
    stream = 0
    recs = []
    for _ in range(count):
        lo, hi, typ, n = struct.unpack_from("<IIBI", data, p)
        enc = data[p + 13:p + 13 + n]
        if data[p + 13 + n] != 0:
            raise ValueError("missing terminator at 0x%x" % (p + 13 + n))
        recs.append(Record((hi << 32) | lo, typ, unmask(enc, stream)))
        stream += n
        p += 14 + n
    if p != len(data):
        raise ValueError("trailing bytes: parsed %d of %d" % (p, len(data)))
    return recs


def write(recs, sort=True):
    if sort:
        recs = sorted(recs, key=lambda r: r.key >> 32)
    out = bytearray(struct.pack("<I", len(recs)))
    stream = 0
    for r in recs:
        enc = mask(r.value, stream)
        out += struct.pack("<IIBI", r.key & 0xFFFFFFFF, r.key >> 32, r.type, len(enc))
        out += enc
        out += b"\x00"
        stream += len(enc)
    return bytes(out)


def load_names(path):
    names = {}
    if not path or not os.path.exists(path):
        return names
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            h, name = line.split("\t", 1)
            names[int(h, 16)] = name
    return names


def default_names_path():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.join(here, "..", "work", "gamedb", "names.tsv")


def cmd_dump(args):
    data = open(args.input, "rb").read()
    recs = read(data)
    names = load_names(args.names)
    rows = []
    for r in recs:
        try:
            v = r.value.decode("utf-8")
        except UnicodeDecodeError:
            v = r.value.decode("latin-1")
        rows.append({
            "key": "%016x" % r.key,
            "name": names.get(r.key),
            "type": r.type,
            "type_name": TYPE_NAMES.get(r.type, "t%d" % r.type),
            "value": v,
        })
    if args.tree:
        tree = {}
        unnamed = []
        for row in rows:
            if row["name"] is None:
                unnamed.append(row)
                continue
            node = tree
            parts = row["name"].split(".")
            for part in parts[:-1]:
                nxt = node.get(part)
                if not isinstance(nxt, dict):
                    nxt = {} if nxt is None else {"": nxt}
                    node[part] = nxt
                node = nxt
            leaf = parts[-1]
            if isinstance(node.get(leaf), dict):
                node[leaf][""] = row["value"]
            else:
                node[leaf] = row["value"]
        doc = {"named": tree, "unnamed": unnamed}
    else:
        doc = {"count": len(rows), "records": rows}
    out = open(args.output, "w", encoding="utf-8") if args.output else sys.stdout
    json.dump(doc, out, indent=1, ensure_ascii=False)
    if args.output:
        out.close()
        named = sum(1 for r in rows if r["name"])
        print("%d records, %d named -> %s" % (len(rows), named, args.output), file=sys.stderr)


def cmd_roundtrip(args):
    data = open(args.input, "rb").read()
    recs = read(data)
    again = write(recs)
    ok = again == data
    print("records=%d bytes=%d roundtrip=%s" % (len(recs), len(data), "IDENTICAL" if ok else "DIFFERENT"))
    return 0 if ok else 1


def cmd_get(args):
    data = open(args.input, "rb").read()
    recs = read(data)
    want = fnv1a64(args.path)
    for r in recs:
        if r.key == want:
            print("%016x\t%s\t%s" % (r.key, TYPE_NAMES.get(r.type, r.type), r.value.decode("utf-8", "replace")))
            return 0
    print("not found: %s (%016x)" % (args.path, want))
    return 1


def cmd_set(args):
    data = open(args.input, "rb").read()
    recs = read(data)
    want = fnv1a64(args.path)
    hit = [r for r in recs if r.key == want]
    if hit:
        hit[0].value = args.value.encode("utf-8")
        if args.type is not None:
            hit[0].type = args.type
    else:
        if args.type is None:
            print("new key needs --type", file=sys.stderr)
            return 1
        recs.append(Record(want, args.type, args.value.encode("utf-8")))
    open(args.output, "wb").write(write(recs))
    print("%s %s -> %s" % ("updated" if hit else "added", args.path, args.output))
    return 0


def main():
    ap = argparse.ArgumentParser(description="F1 25 gamedatabases.2025.bin reader/writer")
    sub = ap.add_subparsers(dest="cmd", required=True)
    d = sub.add_parser("dump")
    d.add_argument("input")
    d.add_argument("-o", "--output")
    d.add_argument("--names", default=default_names_path())
    d.add_argument("--tree", action="store_true")
    d.set_defaults(fn=cmd_dump)
    r = sub.add_parser("roundtrip")
    r.add_argument("input")
    r.set_defaults(fn=cmd_roundtrip)
    g = sub.add_parser("get")
    g.add_argument("input")
    g.add_argument("path")
    g.set_defaults(fn=cmd_get)
    s = sub.add_parser("set")
    s.add_argument("input")
    s.add_argument("path")
    s.add_argument("value")
    s.add_argument("-o", "--output", required=True)
    s.add_argument("--type", type=int)
    s.set_defaults(fn=cmd_set)
    args = ap.parse_args()
    rc = args.fn(args)
    sys.exit(rc or 0)


if __name__ == "__main__":
    main()
