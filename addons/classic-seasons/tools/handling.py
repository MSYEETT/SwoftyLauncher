"""Suspension geometry of the season cars.

The stock handling files set SuspensionSystem/Suspension/UseModelPositions, so the wheels sit where the car model's
skeleton puts them. The zips keep the stock handling files under models with each era's geometry, which the 2025
AI was not tuned for. `use_handling_positions` turns it off in every handling file of an archive, and the wheels take
the handling file's own LocalRestPosition values.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "tools"))

import vtf


def use_handling_positions(e):
    done = 0
    for r in e.res:
        if not r.name.endswith(".vtf"):
            continue
        root = vtf.parse(e.frag_data(r, 0))
        flags = []

        def walk(node):
            if node.kind != 0:
                return
            if node.name == b"UseModelPositions" and node.attr(b"value") == b"true":
                flags.append(node)
            for c in node.children:
                walk(c)

        walk(root)
        for node in flags:
            node.set_attr(b"value", b"false")
        if flags:
            e.set_frag(r, 0, vtf.write(root))
            done += len(flags)
    return done
