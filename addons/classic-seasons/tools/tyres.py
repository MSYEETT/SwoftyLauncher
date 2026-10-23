"""Tyre set overrides for the season cars.

The car templates (F1CarModern2025, F1ShowRoomCarModern2025 in eaid://wote/types) give every car the Tyres component
with f1_2025_vehicle_package's tyre set. A season's tyre set keeps its own names (cs_<year>_vehicle_package), and
`override_layers` adds a Tyres override naming it to every car and showroom model layer of the season's archive.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "tools"))

from pkg import Obj, Pkg, Val

LAYERS = ("-carmodel.layer", "-carshowroommodel.layer")


def tyre_paths(pkg):
    base = f"eaid://{pkg}/shared/tyres/idf/"
    return {
        "tyre model": base + "tyres.idf?context=render",
        "tyre wet model": base + "tyres_wet.idf?context=render",
        "material": base + "tyres.emb",
        "wet material": base + "tyres_wet.emb?context=wet",
    }


def override_layers(e, pkg):
    paths = tyre_paths(pkg)
    prefix = f"eaid://{pkg}/"
    done = 0
    for r in e.res:
        if not r.name.startswith(prefix) or not r.name.endswith(LAYERS):
            continue
        p = Pkg(e.frag_data(r, 0))
        for inst in p.root["instances"].items:
            if not isinstance(inst, Obj):
                continue
            over = inst["overrides"]
            tyres = over.get("Tyres")
            if not isinstance(tyres, Obj):
                tyres = Obj()
                over.set("Tyres", tyres)
            for k, v in paths.items():
                tyres.set(k, Val("stri", v))
            done += 1
        e.set_frag(r, 0, p.to_bytes())
    return done
