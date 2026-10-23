"""Cockpit and TV pod cameras of the season cars.

fanfreluche's archives keep the stock 2025 camera layers (<team>-camscargame.layer and its split-screen _ss copy),
whose cockpit camera hangs off the CameraAttachmentPointCockpit (_ss) instance: AttachmentPoint.Offset and the
WorldObject transform's translation, x right, y up, z forward, in metres. `move_cockpit` shifts both in both of a package's layers.
The TV pod cameras (CamPod, CamPodOffset and their _ss copies) leave out what lies nearer than their Camera.Near
clip (0.23 m on the stock CamPod), which hides the top of a 2025 car; `pod_near_clip` sets it on all four for a
taller car.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "tools"))

from pkg import Obj, Pkg, Val

LAYERS = ("-camscargame.layer", "-camscargame_ss.layer")
POINTS = ("$CameraAttachmentPointCockpit", "$CameraAttachmentPointCockpit_ss")
PODS = ("$CamPod", "$CamPodOffset", "$CamPod_ss", "$CamPodOffset_ss")


def name_of(inst):
    v = inst.get("name")
    return v.value if isinstance(v, Val) else v


def move_cockpit(e, pkg, up, forward):
    moved = 0
    for r in e.res:
        if not r.name.startswith(f"eaid://{pkg}/") or not r.name.endswith(LAYERS):
            continue
        p = Pkg(e.frag_data(r, 0))
        for inst in p.root["instances"].items:
            if not isinstance(inst, Obj) or not str(name_of(inst)).endswith(POINTS):
                continue
            over = inst["overrides"]
            off = over["AttachmentPoint"]["Offset"]
            off.value = [off.value[0], off.value[1] + up, off.value[2] + forward, off.value[3]]
            tr = over["WorldObject"]["transform"]
            m = list(tr.value)
            m[13], m[14] = m[13] + up, m[14] + forward
            tr.value = m
            moved += 1
        e.set_frag(r, 0, p.to_bytes())
    if moved != len(LAYERS):
        raise ValueError(f"{moved} cockpit camera attachment point(s), expected {len(LAYERS)}")
    return moved


def pod_near_clip(e, pkg, near):
    done = 0
    for r in e.res:
        if not r.name.startswith(f"eaid://{pkg}/") or not r.name.endswith(LAYERS):
            continue
        p = Pkg(e.frag_data(r, 0))
        for inst in p.root["instances"].items:
            if not isinstance(inst, Obj) or not str(name_of(inst)).endswith(PODS):
                continue
            over = inst["overrides"]
            cam = over.get("Camera")
            if not isinstance(cam, Obj):
                cam = Obj()
                over.set("Camera", cam)
            cam.set("Near clip", Val("fp32", float(near)))
            done += 1
        e.set_frag(r, 0, p.to_bytes())
    if done != len(PODS):
        raise ValueError(f"{done} TV pod camera(s), expected {len(PODS)}")
    return done
