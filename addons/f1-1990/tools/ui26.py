#!/usr/bin/env python3
"""Writes erp/ui26.json and ui/: the laptop lists grown for 13 teams. Constructor standings get a 13th row and the
Vehicle Performance Comparison email 13 pills instead of 11. Each change adds rows below the stock ones and leaves the
stock rows where they are. The R&D screen's comparison charts share the grown pill stack, which gives them a 12th row
(the stock screen already binds entry_11 to pill_11); their screen, laptop_r_and_d.ui, is left to rnd-tyres-thermal,
which replaces it.

usage: ui26.py [--game <F1 25>] [--baseline <launcher baseline>/<build>]

The stock resources come from the baseline when it lists the archive, else from the game folder.
"""
import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
REPO = ROOT.parents[1]
sys.path[:0] = [str(REPO / "tools"), str(REPO / "tools/swofty-ui")]

import idf as I  # noqa: E402
import scene as S  # noqa: E402
from erp import parse as erp_parse  # noqa: E402
from pkg import Arr, Obj, Pkg, Val  # noqa: E402
from uinode import CHILDREN, LAYOUT, PROPS, children, clone_node, fix_ids, is_node, name  # noqa: E402

UI = "2025_asset_groups/ui_package/"
HUB, HQ = UI + "flow_career_hub.erp", UI + "flow_team_hq.erp"
ART = "eaid://ui_package/art/components/career/laptop/"
SCREENS = "eaid://ui_package/screens/"
OUT = ROOT / "ui"
OPS = ROOT / "erp/ui26.json"
WHEN = {"cars": 26}

# scene: (resource, archives, [(source element, new element, y step)])
SCENES = {
    "sl_constructor_stack.idf": ((HUB, HQ), [("11", "12", -0.4)]),
    "r_and_d_performance_compare_stack_attach.idf": ((HUB, HQ), [("pill_10", "pill_11", -0.45),
                                                                  ("pill_11", "pill_12", -0.45)]),
    "r_and_d_performance_compare_stack.idf": ((HUB,), [("pill_10", "pill_11", -0.46), ("pill_11", "pill_12", -0.46)]),
}
# screen: (archives, [(list node path, source entry, new entry, scene element, view height or None)])
LISTS = {
    "laptop_standings.ui": ((HUB, HQ), [
        ("tab_content_championship_standings.constructor_championship.constructor_list", "11", "12", "12", None),
    ]),
    "laptop_overview.ui": ((HUB, HQ), [
        ("tab_content_emails_panel.reading_pane.attachments.performance_comparison.performance_list",
         "entry_11", "entry_12", "pill_12", 13),
    ]),
}


class Stock:
    def __init__(self, game, baseline):
        self.game, self.files, self.objects = game, {}, None
        if baseline:
            self.files = json.loads((baseline / "baseline.json").read_text())["files"]
            self.objects = baseline.parent / "objects"

    def path(self, rel):
        entry = self.files.get(rel)
        if entry:
            obj = self.objects / entry["sha256"][:2] / entry["sha256"]
            if obj.exists():
                return obj
        if not self.game:
            sys.exit(f"{rel}: not in the baseline and no --game")
        p = self.game / rel
        if entry and hashlib.sha256(p.read_bytes()).hexdigest() != entry["sha256"]:
            sys.exit(f"{rel}: changed in the game folder and no stock copy in the baseline")
        return p

    def fragment(self, rel, resource):
        path = self.path(rel)
        meta, res = erp_parse(str(path))
        frags = {r[0]: r for r in res}[resource][3]
        _, off, _, _, comp, packed = frags[0]
        with open(path, "rb") as f:
            f.seek(meta["data_off"] + off)
            raw = f.read(packed)
        if comp in (0x11, 0x91):
            raw = subprocess.run(["zstd", "-dc"], input=raw, capture_output=True, check=True).stdout
        return raw


def element_key(doc, element):
    for si, sc in enumerate(doc.scenes):
        def walk(obj, key):
            for i, c in enumerate(S.child_objs(obj)):
                k = f"{key}.{i}"
                if I._val(c.get("name")) == element:
                    return k, c
                found = walk(c, k)
                if found:
                    return found
            return None
        found = walk(sc.entry.get("scene"), str(si))
        if found:
            return found
    raise KeyError(element)


def grow_scene(data, clones):
    doc = I.Idf(data)
    for src, new, step in clones:
        key, obj = element_key(doc, src)
        pos = list(obj.get("position").value)
        S.apply_ops(doc, [{"op": "clone", "key": key, "name": new, "position": [pos[0], round(pos[1] + step, 6), pos[2]]}])
    out = doc.to_bytes()
    checks = S.verify_bytes(out, component_ids=[sc.name for sc in I.Idf(data).scenes])
    if not checks["ok"]:
        sys.exit(f"scene check failed: {checks}")
    return out


def screen_roots(o):
    if is_node(o):
        yield o
        return
    for x in (v for _, v in o.items) if isinstance(o, Obj) else (o.items if isinstance(o, Arr) else []):
        yield from screen_roots(x)


def find(pkg, path):
    first, *rest = path.split(".")
    for r in screen_roots(pkg.root):
        if name(r) != first:
            continue
        n = r
        for part in rest:
            n = next(c for c in children(n) if is_node(c) and name(c) == part)
        return n
    raise KeyError(path)


def bind(node, element):
    for grp in node.items[LAYOUT].items:
        for b in grp.items if isinstance(grp, Arr) else []:
            if isinstance(b, Obj) and b.get("debugName") is not None:
                b["debugName"].value = element
                return
    raise KeyError(f"{name(node)} has no scene binding")


def grow_screen(data, lists):
    pkg = Pkg(data)
    for path, src, new, element, view in lists:
        lst = find(pkg, path)
        kids = lst.items[CHILDREN].items
        at = next(i for i, c in enumerate(kids) if is_node(c) and name(c) == src)
        if any(is_node(c) and name(c) == new for c in kids):
            sys.exit(f"{path}.{new} already exists")
        clone = clone_node(kids[at], path, new)
        bind(clone, element)
        fix_ids(clone, path)
        kids.insert(at + 1, clone)
        if view is not None:
            vh = lst.items[PROPS].get("viewHeight")
            vh["value"].value = view
    out = pkg.to_bytes()
    if Pkg(out).to_bytes() != out:
        sys.exit("edited screen does not round-trip")
    return out


def tree(pkg):
    out = {}

    def walk(n, path):
        p = f"{path}.{name(n)}" if path else name(n)
        out[p] = n
        for c in children(n):
            if is_node(c):
                walk(c, p)
    for r in screen_roots(pkg.root):
        walk(r, "")
    return out


def check_screen(stock, edited, lists):
    a, b = tree(Pkg(stock)), tree(Pkg(edited))
    added = sorted(set(b) - set(a))
    expect = {f"{p}.{new}" for p, _, new, _, _ in lists}
    if set(a) - set(b) or not all(any(x == e or x.startswith(e + ".") for e in expect) for x in added):
        sys.exit(f"unexpected node changes: removed {sorted(set(a) - set(b))[:5]}, added {added[:5]}")
    changed = [p for p in a if _node_bytes(a[p]) != _node_bytes(b[p])]
    lists_paths = {p for p, *_ in lists}
    if any(p not in lists_paths for p in changed):
        sys.exit(f"stock nodes changed: {[p for p in changed if p not in lists_paths][:5]}")
    return len(added), changed


def _node_bytes(n):
    props = n.items[PROPS]
    own = [(k.value, repr(v.value) if isinstance(v, Val) else _flat(v)) for k, v in props.items]
    return repr(own) + repr([_flat(n.items[i]) for i in range(len(n.items)) if i not in (PROPS, CHILDREN)])


def _flat(o):
    if isinstance(o, Val):
        return repr(o.value)
    if isinstance(o, Obj):
        return "{" + ",".join(f"{k.value}:{_flat(v)}" for k, v in o.items) + "}"
    if isinstance(o, Arr):
        return "[" + ",".join(_flat(x) for x in o.items) + "]"
    return "?"


def build(stock):
    OUT.mkdir(exist_ok=True)
    ops, lines = [], []
    for res, (archives, clones) in SCENES.items():
        data = stock.fragment(archives[0], ART + res)
        for rel in archives[1:]:
            if stock.fragment(rel, ART + res) != data:
                sys.exit(f"{res}: the stock copies in {archives} differ")
        out = grow_scene(data, clones)
        (OUT / res).write_bytes(out)
        ops += [{"op": "replace-resource", "when": WHEN, "erp": rel, "resource": ART + res, "source": f"ui/{res}"}
                for rel in archives]
        lines.append(f"ui/{res}: {len(data)} -> {len(out)} bytes, +{', '.join(n for _, n, _ in clones)}")
    for res, (archives, lists) in LISTS.items():
        data = stock.fragment(archives[0], SCREENS + res)
        for rel in archives[1:]:
            if stock.fragment(rel, SCREENS + res) != data:
                sys.exit(f"{res}: the stock copies in {archives} differ")
        out = grow_screen(data, lists)
        added, changed = check_screen(data, out, lists)
        (OUT / res).write_bytes(out)
        ops += [{"op": "replace-resource", "when": WHEN, "erp": rel, "resource": SCREENS + res, "source": f"ui/{res}"}
                for rel in archives]
        lines.append(f"ui/{res}: {len(data)} -> {len(out)} bytes, {added} node(s) added, changed {', '.join(changed)}")
    OPS.write_text(json.dumps({"schema_version": 1, "ops": ops}, indent=1) + "\n")
    lines.append(f"erp/ui26.json: {len(ops)} ops")
    return lines


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", type=Path)
    ap.add_argument("--baseline", type=Path)
    a = ap.parse_args(argv[1:])
    if not a.game and not a.baseline:
        sys.exit(__doc__)
    for line in build(Stock(a.game, a.baseline)):
        print(line)


if __name__ == "__main__":
    main(sys.argv)
