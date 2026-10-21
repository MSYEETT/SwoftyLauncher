#!/usr/bin/env python3
"""Writes erp/wizard.json: the route the 1990 Career tile fires to open the career wizard on its settings step.

usage: wizard.py --game <F1 25 folder> [--baseline <launcher baseline>/<build>]

The Career content select (grouping fe_career_dlc_select in routes_frontend_general.uir) opens a new driver career on
the season length step with career_wizard_driver_career_to_f1. The op clones that route as
career_wizard_driver_career_1990 and points its close and open steps at career_wizard_settings_select, as fe_home's
career_wizard_driver_career does. Only f1_1990_wizard.dll fires the new route (for a press of the 1990 tile); no stock
route changes.
"""
import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
REPO = ROOT.parents[1]
sys.path[:0] = [str(HERE), str(REPO / "tools")]

from pkg import Arr, Obj, Pkg, Val  # noqa: E402
from stock import Stock, fragments  # noqa: E402

ERP = "2025_asset_groups/ui_package/persistent.erp"
RESOURCE = "eaid://ui_package/routes/routes_frontend_general.uir"
GROUPING = "fe_career_dlc_select"
SOURCE = "career_wizard_driver_career_to_f1"
ROUTE = "career_wizard_driver_career_1990"
FROM_STEP = "career_wizard_season_length_select"
TO_STEP = "career_wizard_settings_select"
RETARGET = [("3", "To"), ("6", "Target")]


def flat(o):
    if isinstance(o, Val):
        return o.value
    if isinstance(o, Obj):
        return {k.value: flat(v) for k, v in o.items}
    if isinstance(o, Arr):
        return [flat(x) for x in o.items]
    raise SystemExit(f"{RESOURCE}: unexpected node {o!r}")


def source_route(stock):
    doc = flat(Pkg(fragments(stock.path(ERP), RESOURCE)[0]).root)
    groups = [g for g in doc["RouteGroupings"] if g["Id"] == GROUPING]
    if len(groups) != 1:
        raise SystemExit(f"{RESOURCE}: {len(groups)} groupings named {GROUPING}")
    routes = {r["Id"]: r for r in groups[0]["Routes"]}
    if ROUTE in routes:
        raise SystemExit(f"{GROUPING}: the stock grouping already has {ROUTE}")
    if SOURCE not in routes:
        raise SystemExit(f"{GROUPING}: no route {SOURCE}")
    return routes[SOURCE]


def check(route):
    for dep, attr in RETARGET:
        hits = [c for c in route["Commands"] if c.get("Dependencies") == dep]
        if len(hits) != 1 or hits[0]["Type"] != "TriggerSubRoute":
            raise SystemExit(f"{SOURCE}: {len(hits)} command(s) with Dependencies={dep}, want one TriggerSubRoute")
        values = [a["Value"] for a in hits[0]["Attributes"] if a["Id"] == attr]
        if values != [FROM_STEP]:
            raise SystemExit(f"{SOURCE}: Dependencies={dep} {attr} is {values}, want [{FROM_STEP!r}]")


def build(stock):
    check(source_route(stock))
    return {"schema_version": 1, "ops": [{
        "op": "patch-pkg", "erp": ERP, "resource": RESOURCE, "fragment": 0,
        "edits": [{
            "op": "clone-entry", "at": f"RouteGroupings[Id={GROUPING}].Routes", "src": f"[Id={SOURCE}]", "as": ROUTE,
            "set": {f"Commands[Dependencies={dep}].Attributes[Id={attr}].Value": TO_STEP for dep, attr in RETARGET},
        }],
    }]}


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", required=True, type=Path)
    ap.add_argument("--baseline", type=Path)
    a = ap.parse_args(argv[1:])
    out = ROOT / "erp/wizard.json"
    with open(out, "w", encoding="utf-8") as f:
        json.dump(build(Stock(a.game, a.baseline)), f, indent=1)
        f.write("\n")
    print(f"wrote {out}")


if __name__ == "__main__":
    main(sys.argv)
