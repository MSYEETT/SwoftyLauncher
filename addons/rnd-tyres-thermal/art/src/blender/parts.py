import math

import bmesh
from mathutils import Matrix, Vector

from lib import (TAU, apply_mods, bevel_obj, bezier, box_bm, cyl_bm, lathe, lathe_bm, loft_bm, obj_from_bm, orient,
                 rot, tr, transform, tube_bm)

PARTS = {}


def part(name, **opts):
    def wrap(fn):
        PARTS[name] = (fn, opts)
        return fn
    return wrap


def disc(ro=0.164, ri=0.106, t=0.016, segs=160, rows=5, bell=True, bolts=12):
    c = 0.0018
    zs = [-t + c + (2 * (t - c)) * i / rows for i in range(rows + 1)]
    band = ro - 0.010
    prof = [(band, -t), (ro - c, -t)] + [(ro, z) for z in zs] + [(ro - c, t), (band, t)]
    bm = bmesh.new()
    lathe_bm(bm, prof, segs=segs)
    lathe_bm(bm, [(ri, -t), (band, -t), (band, t), (ri, t)], segs=segs // 2)
    outer = [f for f in bm.faces if all(abs(v.co.xy.length - ro) < 1e-5 for v in f.verts)]
    bmesh.ops.inset_individual(bm, faces=outer, thickness=0.0011, depth=-0.0035, use_even_offset=True)
    for f in outer:
        f.material_index = 1
    objs = [obj_from_bm('disc', bm, ['cc_disc', 'hole'], smooth=30)]
    if bell:
        hp = [(0.050, -0.074), (0.086, -0.074), (0.086, -0.066), (0.080, -0.062), (0.080, -0.012),
              (0.112, -0.012), (0.112, 0.004), (0.074, 0.004), (0.074, -0.058), (0.050, -0.058)]
        bm = bmesh.new()
        lathe_bm(bm, hp, segs=72)
        for i in range(bolts):
            a = TAU * i / bolts
            m = Matrix.Rotation(a, 4, 'Z') @ tr(0.103, 0, -0.004) @ rot('Y', 0)
            cyl_bm(bm, 0.0055, 0.012, segs=12, matrix=m)
        for i in range(8):
            a = TAU * (i + 0.5) / 8
            m = Matrix.Rotation(a, 4, 'Z') @ tr(0.062, 0, -0.074)
            cyl_bm(bm, 0.005, 0.006, segs=10, matrix=m)
        ob = obj_from_bm('bell', bm, 'alu_dark', smooth=40)
        objs.append(ob)
        bm = bmesh.new()
        for i in range(bolts):
            a = TAU * i / bolts
            m = Matrix.Rotation(a, 4, 'Z') @ tr(0.103, 0, t + 0.0015)
            cyl_bm(bm, 0.0042, 0.003, segs=12, matrix=m)
        objs.append(obj_from_bm('pegs', bm, 'titanium', smooth=40))
    return objs


def caliper(fins=0, span=70.0, mat='alu', r_in=0.118, r_out=0.184, half=0.058, gap=0.022):
    prof = [(r_in, gap), (r_in, half - 0.004), (r_in + 0.004, half), (r_out - 0.008, half), (r_out, half - 0.008),
            (r_out, -half + 0.008), (r_out - 0.008, -half), (r_in + 0.004, -half), (r_in, -half + 0.004),
            (r_in, -gap), (r_out - 0.016, -gap), (r_out - 0.016, gap)]
    a0 = math.radians(-span / 2)
    objs = []
    bm = bmesh.new()
    lathe_bm(bm, prof, segs=28, angle=math.radians(span), start=a0)
    for ang in (-span / 2 - 4, span / 2 + 4):
        m = Matrix.Rotation(math.radians(ang), 4, 'Z') @ tr(0.150, 0, -half + 0.012)
        cyl_bm(bm, 0.015, 0.024, segs=20, matrix=m)
        m = Matrix.Rotation(math.radians(ang), 4, 'Z') @ tr(0.150, 0, -half - 0.001)
        cyl_bm(bm, 0.007, 0.004, segs=14, matrix=m)
    body = obj_from_bm('caliper', bm, mat, smooth=40)
    bevel_obj(body, 0.0016, segments=2, angle=40)
    objs.append(body)
    bm = bmesh.new()
    for side in (1, -1):
        for k, ang in enumerate((-21, 0, 21)):
            rr = 0.0175 if k == 1 else 0.016
            m = Matrix.Rotation(math.radians(ang), 4, 'Z') @ tr(0.146, 0, side * (half + 0.0035))
            cyl_bm(bm, rr, 0.007, segs=28, matrix=m)
            m = Matrix.Rotation(math.radians(ang), 4, 'Z') @ tr(0.146, 0, side * (half + 0.0075))
            cyl_bm(bm, rr * 0.55, 0.002, segs=20, matrix=m)
    objs.append(obj_from_bm('pistons', bm, 'alu_dark', smooth=40))
    bm = bmesh.new()
    m = Matrix.Rotation(math.radians(span / 2 - 8), 4, 'Z') @ tr(r_out + 0.006, 0, half - 0.016) @ rot('Y', 90)
    cyl_bm(bm, 0.0045, 0.014, segs=14, matrix=m)
    objs.append(obj_from_bm('bleed', bm, 'red_anod', smooth=40))
    if fins:
        bm = bmesh.new()
        fs = math.radians(span - 12)
        for i in range(fins):
            zc = -half + 0.010 + (2 * half - 0.020) * i / (fins - 1)
            fp = [(r_out - 0.004, zc - 0.0019), (r_out + 0.021, zc - 0.0011), (r_out + 0.022, zc),
                  (r_out + 0.021, zc + 0.0011), (r_out - 0.004, zc + 0.0019)]
            lathe_bm(bm, fp, segs=22, angle=fs, start=-fs / 2)
        for side in (1, -1):
            for i in range(5):
                rr = r_in + 0.010 + i * 0.011
                for a0, a1 in ((-span / 2 + 4, -31), (31, span / 2 - 4)):
                    fp = [(rr - 0.0016, side * half - side * 0.002), (rr - 0.0010, side * (half + 0.010)),
                          (rr + 0.0010, side * (half + 0.010)), (rr + 0.0016, side * half - side * 0.002)]
                    lathe_bm(bm, fp, segs=6, angle=math.radians(a1 - a0), start=math.radians(a0))
        objs.append(obj_from_bm('fins', bm, 'fin', smooth=30))
    return objs


def axis(z_to, x_hint=(1, 0, 0)):
    z = Vector(z_to).normalized()
    x = Vector(x_hint)
    x = (x - z * x.dot(z)).normalized()
    y = z.cross(x)
    return Matrix((x, y, z)).transposed().to_4x4()


def superellipse(a, b, p, n=36):
    pts = []
    for i in range(n):
        t = TAU * (i + 0.5) / n
        c, s = math.cos(t), math.sin(t)
        x = a * math.copysign(abs(c) ** (2 / p), c)
        y = b * math.copysign(abs(s) ** (2 / p), s)
        pts.append((x, y))
    return pts


def duct(length=0.30, mouth=(0.085, 0.046), throat=0.046, bend=(0.12, 0.05), wall=0.0032, flare=1.18,
         sections=16, vanes=1, mat='carbon'):
    p0 = Vector((0, 0, 0))
    p1 = Vector((0, length * 0.6, 0))
    p2 = Vector((bend[0], length, bend[1]))

    def at(s):
        return (1 - s) ** 2 * p0 + 2 * (1 - s) * s * p1 + s * s * p2

    def tangent(s):
        return (2 * (1 - s) * (p1 - p0) + 2 * s * (p2 - p1)).normalized()

    outer, inner = [], []
    for i in range(sections + 1):
        s = i / sections
        e = s ** 0.8
        a = mouth[0] + (throat - mouth[0]) * e
        b = mouth[1] + (throat - mouth[1]) * e
        p = 5.0 + (2.0 - 5.0) * e
        if i == 0:
            a, b = a * flare, b * flare
        elif i == 1:
            a, b = a * (1 + (flare - 1) * 0.35), b * (1 + (flare - 1) * 0.35)
        T = tangent(s)
        N = T.cross(Vector((0, 0, 1))).normalized()
        B = N.cross(T).normalized()
        P = at(s)
        outer.append([P + N * x + B * y for x, y in superellipse(a, b, p)])
        inner.append([P + N * x + B * y for x, y in superellipse(a - wall, b - wall, p)])
    bm = bmesh.new()
    go = loft_bm(bm, outer)
    gi = loft_bm(bm, inner)
    n = len(go[0])
    for end in (0, -1):
        for j in range(n):
            k = (j + 1) % n
            bm.faces.new((go[end][j], go[end][k], gi[end][k], gi[end][j]))
    objs = [obj_from_bm('duct', bm, mat, smooth=50)]
    T, P = tangent(1.0), at(1.0)
    frame = Vector((0, 0, 1)).rotation_difference(T).to_matrix().to_4x4()
    bm = bmesh.new()
    lathe_bm(bm, [(throat - wall, -0.004), (throat + 0.022, -0.004), (throat + 0.022, 0.004), (throat - wall, 0.004)],
             segs=48, matrix=Matrix.Translation(P) @ frame)
    for i in range(6):
        m = Matrix.Translation(P) @ frame @ Matrix.Rotation(TAU * i / 6 + 0.3, 4, 'Z') @ tr(throat + 0.013, 0, 0.0055)
        cyl_bm(bm, 0.0045, 0.004, segs=6, matrix=m)
    objs.append(obj_from_bm('flange', bm, 'titanium', smooth=35))
    if vanes:
        bm = bmesh.new()
        for v in range(vanes):
            off = 0.0 if vanes == 1 else (v / (vanes - 1) - 0.5) * mouth[0] * 1.1
            secs = []
            for i in range(0, 7):
                s = i / sections
                T = tangent(s)
                N = T.cross(Vector((0, 0, 1))).normalized()
                B = N.cross(T).normalized()
                e = s ** 0.8
                bb = (mouth[1] + (throat - mouth[1]) * e) * (flare if i == 0 else 1.0) - wall
                P = at(s) + N * off * (1 - e)
                secs.append([P + B * bb + N * 0.0012, P - B * bb + N * 0.0012, P - B * bb - N * 0.0012,
                             P + B * bb - N * 0.0012])
            loft_bm(bm, secs, cap_ends=True)
        objs.append(obj_from_bm('vanes', bm, mat, smooth=30))
    return objs


@part('bc_duct_inlet')
def bc_duct_inlet():
    objs = duct(vanes=2)
    bm = bmesh.new()
    box_bm(bm, (0.012, 0.07, 0.05), tr(0.0, 0.11, -0.068), bevel=0.002)
    cyl_bm(bm, 0.008, 0.016, segs=16, matrix=tr(0.0, 0.125, -0.085) @ rot('Y', 90))
    objs.append(obj_from_bm('bracket', bm, 'alu_dark', smooth=40))
    transform(objs, rot('Z', 18) @ rot('X', -8))
    return objs


@part('bc_disc_ventilation')
def bc_disc_ventilation():
    objs = disc(segs=168, rows=6)
    transform(objs, axis((-0.05, -0.55, 1.0), (1, -0.2, 0.3)))
    return objs


@part('bc_caliper_fins')
def bc_caliper_fins():
    objs = caliper(fins=9)
    transform(objs, tr(-0.151, 0, 0))
    transform(objs, orient((0.05, -0.45, 1.0), (-0.55, -1.0, -0.2)))
    return objs


@part('bc_drum_package')
def bc_drum_package():
    objs = []
    objs += disc(segs=132, rows=4, bell=True)
    cal = caliper(fins=0, span=62, r_in=0.120, r_out=0.182, half=0.050, gap=0.019)
    transform(cal, rot('Z', 128))
    objs += cal
    cut0, cut = math.radians(-128), math.radians(258)
    prof = [(0.122, 0.074), (0.207, 0.074), (0.212, 0.068), (0.212, -0.062), (0.207, -0.062), (0.207, 0.070),
            (0.122, 0.070)]
    bm = bmesh.new()
    lathe_bm(bm, prof, segs=86, angle=cut, start=cut0)
    for i in range(30):
        a = cut0 + cut * (i + 0.5) / 30
        m = Matrix.Rotation(a, 4, 'Z') @ tr(0.168, 0, 0.0765) @ rot('Z', 28)
        box_bm(bm, (0.075, 0.0024, 0.006), m)
    drum = obj_from_bm('drum', bm, 'carbon', smooth=40)
    objs.append(drum)
    bm = bmesh.new()
    lathe_bm(bm, [(0.212, -0.062), (0.226, -0.066), (0.226, -0.074), (0.040, -0.074), (0.040, -0.068),
                  (0.207, -0.062)], segs=86, angle=cut, start=cut0)
    objs.append(obj_from_bm('backplate', bm, 'carbon_matte', smooth=40))
    d = duct(length=0.24, mouth=(0.070, 0.040), throat=0.038, bend=(-0.07, -0.11), vanes=1)
    transform(d, tr(0.262, -0.20, 0.01))
    objs += d
    bm = bmesh.new()
    lathe_bm(bm, [(0.0, -0.20), (0.036, -0.20), (0.036, -0.074), (0.0, -0.074)], segs=40)
    objs.append(obj_from_bm('stub', bm, 'alu_dark', smooth=40))
    transform(objs, axis((0.12, -0.85, 0.75), (0.75, 0.1, 0.45)))
    return objs


TYRE_HALF = [(0.2286, 0.128), (0.236, 0.139), (0.248, 0.147), (0.266, 0.1525), (0.286, 0.1550), (0.306, 0.1535),
             (0.322, 0.1490), (0.336, 0.1410), (0.3465, 0.1300), (0.3540, 0.1150), (0.3585, 0.0960), (0.3600, 0.0720),
             (0.3600, 0.0480), (0.3600, 0.0240), (0.3600, 0.0)]


def tyre_outer(width=1.0):
    half = [(r, z * width) for r, z in TYRE_HALF]
    return [(r, -z) for r, z in half] + [(r, z) for r, z in reversed(half[:-1])]


def offset(curve, d):
    out = []
    n = len(curve)
    for i, (r, z) in enumerate(curve):
        a = curve[max(i - 1, 0)]
        b = curve[min(i + 1, n - 1)]
        tr_, tz = b[0] - a[0], b[1] - a[1]
        ln = math.hypot(tr_, tz) or 1.0
        nr, nz = tz / ln, -tr_ / ln
        if (r - 0.29) * nr + z * nz < 0:
            nr, nz = -nr, -nz
        dd = d(r, z) if callable(d) else d
        out.append((r - nr * dd, z - nz * dd))
    return out


def rubber_depth(r, z):
    return 0.0055 + 0.0055 * max(0.0, min(1.0, (r - 0.335) / 0.02))


def shell(outer, inner):
    return list(outer) + list(reversed(inner))


def z_at(curve, r_target, positive=True):
    pts = [(r, z) for r, z in curve if (z >= 0) == positive]
    pts.sort()
    for (r0, z0), (r1, z1) in zip(pts, pts[1:]):
        if r0 <= r_target <= r1:
            t = (r_target - r0) / (r1 - r0)
            return z0 + (z1 - z0) * t
    return pts[-1][1]


def tyre(stripe='paint_red', angle=TAU, start=0.0, layers=False, width=1.0, segs=72, letters=True):
    out = tyre_outer(width)
    full = abs(angle - TAU) < 1e-6
    objs = []
    if layers:
        l1 = offset(out, rubber_depth)
        l2 = offset(out, lambda r, z: rubber_depth(r, z) + 0.0035)
        l3 = offset(out, lambda r, z: rubber_depth(r, z) + 0.0060)
        objs.append(lathe('tyre', shell(out, l1), 'rubber', segs=segs, angle=angle, start=start))
        objs.append(lathe('carcass', shell(l1, l2), 'carcass', segs=segs, angle=angle, start=start))
        objs.append(lathe('liner', shell(l2, l3), 'liner', segs=segs, angle=angle, start=start))
        tread = [p for p in l1 if p[0] > 0.338]
        belt = shell(tread, offset(tread, 0.0018))
        objs.append(lathe('belt', belt, 'belt', segs=segs, angle=angle, start=start))
    else:
        inner = offset(out, 0.011)
        objs.append(lathe('tyre', shell(out, inner), 'rubber', segs=segs, angle=angle, start=start))
    if stripe:
        bm = bmesh.new()
        for side in (1, -1):
            prof = []
            for r in (0.300, 0.304, 0.308, 0.312):
                prof.append((r, side * (z_at(out, r) + 0.0007)))
            prof += [(r, side * (z_at(out, r) - 0.0004)) for r in (0.312, 0.300)]
            lathe_bm(bm, prof, segs=segs, angle=angle, start=start)
        objs.append(obj_from_bm('stripe', bm, stripe, smooth=40))
    if letters:
        bm = bmesh.new()
        import random
        rnd = random.Random(7)
        for side in (1,):
            for a0, a1 in ((0.35, 1.25), (3.5, 4.4)):
                a = a0
                while a < a1:
                    w = rnd.choice((0.004, 0.005, 0.006, 0.008))
                    ang = a + w / 0.33 / 2
                    if full or (start <= ang <= start + angle):
                        zc = z_at(out, 0.326) + 0.0006
                        m = Matrix.Rotation(ang, 4, 'Z') @ tr(0.326, 0, side * zc) @ rot('Z', 90)
                        box_bm(bm, (w, 0.011, 0.0012), m)
                    a += (w + 0.0028) / 0.326
        objs.append(obj_from_bm('letters', bm, 'paint_white', smooth=None))
    return objs


RIM_PROF = [(0.230, -0.150), (0.238, -0.146), (0.238, -0.140), (0.229, -0.135), (0.229, -0.100), (0.214, -0.085),
            (0.214, 0.060), (0.229, 0.080), (0.229, 0.135), (0.238, 0.140), (0.238, 0.146), (0.230, 0.150),
            (0.222, 0.150), (0.222, 0.082), (0.206, 0.066), (0.206, -0.086), (0.222, -0.102), (0.222, -0.150)]


def rim(spokes=10, nut='red_anod', fins=0, holes=0, segs=72):
    objs = [lathe('rim', RIM_PROF, 'rim', segs=segs)]
    bm = bmesh.new()
    for i in range(spokes):
        a = TAU * i / spokes
        m = Matrix.Rotation(a, 4, 'Z') @ tr(0.140, 0, 0.118) @ rot('X', 8)
        v = box_bm(bm, (0.142, 0.020, 0.014), m)
        for vv in v:
            loc = Matrix.Rotation(-a, 4, 'Z') @ vv.co
            k = (loc.x - 0.069) / 0.142
            scale = 1.0 - 0.35 * k
            loc.y *= scale
            vv.co = Matrix.Rotation(a, 4, 'Z') @ loc
    lathe_bm(bm, [(0.040, 0.095), (0.078, 0.095), (0.078, 0.112), (0.070, 0.130), (0.040, 0.130)], segs=48)
    lathe_bm(bm, [(0.198, 0.104), (0.214, 0.104), (0.214, 0.130), (0.198, 0.130)], segs=segs)
    spk = obj_from_bm('spokes', bm, 'rim', smooth=35)
    bevel_obj(spk, 0.0015, 1, 40)
    objs.append(spk)
    bm = bmesh.new()
    lathe_bm(bm, [(0.020, 0.128), (0.043, 0.128), (0.043, 0.158), (0.020, 0.158)], segs=6)
    lathe_bm(bm, [(0.012, 0.150), (0.024, 0.150), (0.024, 0.168), (0.012, 0.168)], segs=24)
    objs.append(obj_from_bm('nut', bm, nut, smooth=20))
    if fins:
        bm = bmesh.new()
        for i in range(fins):
            a = TAU * i / fins
            m = Matrix.Rotation(a, 4, 'Z') @ tr(0.2005, 0, -0.012)
            box_bm(bm, (0.012, 0.0028, 0.140), m)
        objs.append(obj_from_bm('rimfins', bm, 'alu', smooth=30))
    if holes:
        bm = bmesh.new()
        for i in range(holes):
            a = TAU * (i + 0.5) / holes
            m = Matrix.Rotation(a, 4, 'Z') @ tr(0.2245, 0, 0.150)
            cyl_bm(bm, 0.0032, 0.0016, segs=10, matrix=m)
        objs.append(obj_from_bm('rimholes', bm, 'hole', smooth=None))
    return objs


def heat_shield(z0=-0.128, z1=0.052, r=0.1975):
    prof = [(r - 0.004, z0), (r, z0 - 0.003), (r, z1 + 0.003), (r - 0.004, z1), (r - 0.0055, z1),
            (r - 0.0015, z1 - 0.003), (r - 0.0015, z0 + 0.003), (r - 0.0055, z0)]
    objs = [lathe('shield', prof, 'gold', segs=72)]
    bm = bmesh.new()
    for i in range(6):
        m = Matrix.Rotation(TAU * i / 6, 4, 'Z') @ tr(r - 0.004, 0, z1 - 0.012)
        box_bm(bm, (0.006, 0.014, 0.018), m, bevel=0.001)
    objs.append(obj_from_bm('clips', bm, 'titanium', smooth=40))
    return objs


def wheel(stripe='paint_red', angle=TAU, start=0.0, layers=False, rim_opts=None):
    return tyre(stripe, angle, start, layers) + rim(**(rim_opts or {}))


@part('tt_thermal_model')
def tt_thermal_model():
    span = math.radians(64)
    objs = tyre('paint_yellow', angle=span, start=-span / 2, layers=True, segs=24, letters=False)
    bm = bmesh.new()
    for k, z in enumerate((-0.10, -0.035, 0.035, 0.10)):
        a = math.radians(-18 + 12 * k)
        m = Matrix.Rotation(a, 4, 'Z') @ tr(0.352, 0, z) @ rot('Y', 90)
        cyl_bm(bm, 0.0022, 0.030, segs=8, matrix=m)
    objs.append(obj_from_bm('probes', bm, 'steel', smooth=40))
    bm = bmesh.new()
    for k, z in enumerate((-0.10, -0.035, 0.035, 0.10)):
        a = math.radians(-18 + 12 * k)
        m = Matrix.Rotation(a, 4, 'Z') @ tr(0.370, 0, z) @ rot('Y', 90)
        cyl_bm(bm, 0.0065, 0.012, segs=14, matrix=m)
    objs.append(obj_from_bm('probe_caps', bm, 'red_anod', smooth=40))
    bm = bmesh.new()
    for k, z in enumerate((-0.10, -0.035, 0.035, 0.10)):
        a = math.radians(-18 + 12 * k)
        p0 = Matrix.Rotation(a, 4, 'Z') @ Vector((0.376, 0, z))
        p3 = Vector((0.43, -0.05 + 0.02 * k, 0.16))
        tube_bm(bm, bezier(p0, p0 + Vector((0.04, 0, 0)), p3 - Vector((0.02, 0, 0.05)), p3, 10), 0.0024, segs=8)
    objs.append(obj_from_bm('wires', bm, 'black_plastic', smooth=40))
    transform(objs, tr(-0.30, 0, 0))
    transform(objs, axis((-0.75, -0.55, 0.35), (0.3, -0.5, 1.0)))
    return objs


@part('tt_rim_heat_shield')
def tt_rim_heat_shield():
    objs = rim()
    sh = heat_shield()
    transform(sh, tr(0, 0, -0.165))
    objs += sh
    transform(objs, axis((0.35, 0.9, -0.25), (0.3, 0.0, 1.0)))
    return objs


@part('tt_rim_cooling')
def tt_rim_cooling():
    objs = rim(fins=48, holes=36)
    transform(objs, axis((0.40, 0.85, -0.10), (0.2, 0.0, 1.0)))
    return objs


@part('tt_carcass_heat')
def tt_carcass_heat():
    objs = tyre('paint_red', angle=math.radians(250), start=math.radians(-20), layers=True)
    objs += rim(fins=48, holes=36)
    objs += heat_shield()
    transform(objs, axis((-0.25, -0.95, 0.55), (0.2, -0.3, 1.0)))
    return objs


def squash(objs, h0=0.334, bulge=0.07):
    for ob in objs:
        for v in ob.data.vertices:
            depth = -v.co.y - (h0 - 0.07)
            if depth > 0:
                f = min(1.0, depth / 0.07)
                v.co.z *= 1.0 + bulge * f * f
            if v.co.y < -h0:
                v.co.y = -h0


@part('tl_sidewall_load')
def tl_sidewall_load():
    ty = tyre('paint_white')
    squash(ty)
    objs = ty + rim()
    bm = bmesh.new()
    box_bm(bm, (0.62, 0.014, 0.42), tr(0, -0.334 - 0.007, 0.0), bevel=0.002)
    for x in (-0.33, 0.33):
        cyl_bm(bm, 0.035, 0.44, segs=32, matrix=tr(x, -0.334 - 0.050, 0.0))
    objs.append(obj_from_bm('belt_rig', bm, 'steel', smooth=40))
    bm = bmesh.new()
    for x in (-0.33, 0.33):
        for z in (-0.235, 0.235):
            box_bm(bm, (0.08, 0.11, 0.022), tr(x, -0.334 - 0.050, z), bevel=0.003)
    box_bm(bm, (0.78, 0.03, 0.022), tr(0, -0.334 - 0.095, -0.235), bevel=0.003)
    box_bm(bm, (0.78, 0.03, 0.022), tr(0, -0.334 - 0.095, 0.235), bevel=0.003)
    objs.append(obj_from_bm('rig_frame', bm, 'alu_dark', smooth=40))
    transform(objs, rot('X', 90))
    transform(objs, rot('Z', -32))
    return objs


@part('tl_pressure_telemetry')
def tl_pressure_telemetry():
    start = math.radians(-35)
    objs = tyre('paint_red', angle=math.radians(290), start=start) + rim()
    a = math.radians(-35 - 35)
    bm = bmesh.new()
    m = Matrix.Rotation(a, 4, 'Z') @ tr(0.254, 0, 0.045) @ rot('Z', 90)
    box_bm(bm, (0.100, 0.050, 0.060), m, bevel=0.007)
    objs.append(obj_from_bm('sensor', bm, 'black_plastic', smooth=40))
    bm = bmesh.new()
    m = Matrix.Rotation(a, 4, 'Z') @ tr(0.2795, 0, 0.045) @ rot('Z', 90)
    box_bm(bm, (0.070, 0.004, 0.040), m, bevel=0.002)
    objs.append(obj_from_bm('label', bm, 'paint_red', smooth=None))
    bm = bmesh.new()
    m = Matrix.Rotation(a + 0.24, 4, 'Z') @ tr(0.236, 0, 0.045) @ rot('Y', 90)
    cyl_bm(bm, 0.007, 0.05, segs=16, matrix=m)
    m = Matrix.Rotation(a + 0.24, 4, 'Z') @ tr(0.264, 0, 0.045) @ rot('Y', 90)
    cyl_bm(bm, 0.011, 0.012, segs=16, matrix=m)
    m = Matrix.Rotation(a - 0.17, 4, 'Z') @ tr(0.268, 0, 0.09)
    cyl_bm(bm, 0.0028, 0.05, segs=10, matrix=m)
    m = Matrix.Rotation(a - 0.17, 4, 'Z') @ tr(0.268, 0, 0.116)
    cyl_bm(bm, 0.0055, 0.006, segs=12, matrix=m)
    objs.append(obj_from_bm('valve', bm, 'alu', smooth=40))
    transform(objs, axis((-0.20, -0.95, 0.55), (0.3, -0.2, 1.0)))
    return objs


def wishbone(bm, outer, front, rear, r=0.011):
    for p in (front, rear):
        o, q = Vector(outer), Vector(p)
        pts = [o + (q - o) * (i / 8) for i in range(9)]
        tube_bm(bm, pts, lambda i: r * (1.0 - 0.15 * abs(i - 4) / 4), segs=12)


@part('tl_camber_wear')
def tl_camber_wear():
    wh = wheel('paint_yellow')
    transform(wh, rot('Y', 90))
    bm = bmesh.new()
    box_bm(bm, (0.05, 0.10, 0.27), tr(-0.175, 0, 0.0), bevel=0.006)
    cyl_bm(bm, 0.05, 0.05, segs=32, matrix=tr(-0.15, 0, 0) @ rot('Y', 90))
    up = obj_from_bm('upright', bm, 'alu', smooth=40)
    bm = bmesh.new()
    lathe_bm(bm, [(0.0, -0.08), (0.17, -0.08), (0.17, -0.06), (0.0, -0.06)], segs=48,
             matrix=tr(-0.07, 0, 0) @ rot('Y', 90))
    objs = wh + [up]
    transform(objs, rot('Y', -11))
    bm = bmesh.new()
    wishbone(bm, (-0.21, 0.0, 0.135), (-0.78, -0.26, 0.17), (-0.78, 0.30, 0.16), r=0.014)
    wishbone(bm, (-0.21, 0.0, -0.135), (-0.78, -0.30, -0.11), (-0.78, 0.32, -0.12), r=0.015)
    tube_bm(bm, [Vector((-0.24, 0.02, -0.11)) + (Vector((-0.70, 0.04, 0.33)) - Vector((-0.24, 0.02, -0.11))) * (i / 6)
                 for i in range(7)], 0.009, segs=12)
    tube_bm(bm, [Vector((-0.20, -0.07, -0.02)) + (Vector((-0.78, -0.12, 0.0)) - Vector((-0.20, -0.07, -0.02))) * (i / 6)
                 for i in range(7)], 0.007, segs=12)
    objs.append(obj_from_bm('arms', bm, 'carbon', smooth=40))
    bm = bmesh.new()
    for p in ((-0.78, -0.26, 0.17), (-0.78, 0.30, 0.16), (-0.78, -0.30, -0.11), (-0.78, 0.32, -0.12),
              (-0.70, 0.04, 0.33), (-0.78, -0.12, 0.0)):
        cyl_bm(bm, 0.016, 0.03, segs=16, matrix=tr(*p) @ rot('X', 90))
    objs.append(obj_from_bm('pickups', bm, 'titanium', smooth=40))
    transform(objs, rot('Z', 128) @ rot('X', -6))
    return objs


@part('tl_low_deg_package')
def tl_low_deg_package():
    objs = []
    for stripe, pos, ang in (('paint_red', (-0.40, 0.30, 0.02), 18), ('paint_yellow', (0.42, 0.34, 0.0), -12),
                             ('paint_white', (0.0, -0.32, -0.05), 4)):
        w = wheel(stripe)
        transform(w, tr(*pos) @ rot('Z', ang) @ rot('X', 90))
        objs += w
    transform(objs, rot('Z', -18))
    return objs


def radiator(w=0.42, h=0.30, d=0.05, tubes=24, pipes=True):
    objs = []
    bm = bmesh.new()
    box_bm(bm, (w - 0.02, d * 0.8, h - 0.02), Matrix())
    objs.append(obj_from_bm('core', bm, 'fin_core', smooth=None))
    bm = bmesh.new()
    pitch = (h - 0.03) / tubes
    for i in range(tubes):
        z = -h / 2 + 0.015 + pitch * (i + 0.5)
        box_bm(bm, (w - 0.02, d * 0.86, pitch * 0.28), tr(0, 0, z))
    objs.append(obj_from_bm('tubes', bm, 'alu', smooth=None))
    bm = bmesh.new()
    for x in (-w / 2, w / 2):
        box_bm(bm, (0.032, d * 1.25, h + 0.02), tr(x, 0, 0), bevel=0.007)
    for z in (-h / 2 - 0.004, h / 2 + 0.004):
        box_bm(bm, (w, d * 0.9, 0.008), tr(0, 0, z), bevel=0.001)
    for x in (-w / 2, w / 2):
        for z in (-h / 2 - 0.02, h / 2 + 0.02):
            cyl_bm(bm, 0.006, 0.03, segs=12, matrix=tr(x, 0, z))
    objs.append(obj_from_bm('tanks', bm, 'alu_dark', smooth=40))
    if pipes:
        bm = bmesh.new()
        tube_bm(bm, bezier((-w / 2, 0, h / 2 - 0.04), (-w / 2 - 0.10, 0, h / 2 - 0.04), (-w / 2 - 0.10, -0.06, h / 2 + 0.06),
                           (-w / 2 - 0.10, -0.12, h / 2 + 0.10), 12), 0.016, segs=16)
        tube_bm(bm, bezier((w / 2, 0, -h / 2 + 0.04), (w / 2 + 0.10, 0, -h / 2 + 0.04), (w / 2 + 0.10, -0.06, -h / 2 - 0.04),
                           (w / 2 + 0.12, -0.14, -h / 2 - 0.06), 12), 0.016, segs=16)
        objs.append(obj_from_bm('hoses', bm, 'black_plastic', smooth=40))
        bm = bmesh.new()
        for p, ax in (((-w / 2 - 0.035, 0, h / 2 - 0.04), 'Y'), ((w / 2 + 0.035, 0, -h / 2 + 0.04), 'Y')):
            cyl_bm(bm, 0.020, 0.014, segs=20, matrix=tr(*p) @ rot(ax, 90))
        objs.append(obj_from_bm('clamps', bm, 'steel', smooth=40))
    return objs


@part('pc_radiator_core')
def pc_radiator_core():
    objs = radiator(tubes=18)
    transform(objs, rot('Z', -28) @ rot('X', 8))
    return objs


def louvre_panel(nu=28, nv=18, su=0.62, sv=0.42, slots=7, bend=1.1, thick=0.005):
    def surf(u, v):
        x = (u / nu - 0.5) * su
        y = (v / nv - 0.5) * sv
        return Vector((x, y, -bend * y * y + 0.12 * x * x))

    slot_cols = [4 + 3 * k for k in range(slots)]
    v0, v1 = 4, nv - 4
    bm = bmesh.new()
    verts = [[bm.verts.new(surf(i, j)) for j in range(nv + 1)] for i in range(nu + 1)]
    faces = []
    for i in range(nu):
        for j in range(nv):
            if i in slot_cols and v0 <= j < v1:
                continue
            faces.append(bm.faces.new((verts[i][j], verts[i + 1][j], verts[i + 1][j + 1], verts[i][j + 1])))
    bmesh.ops.solidify(bm, geom=faces, thickness=thick)
    objs = [obj_from_bm('panel', bm, 'carbon', smooth=40)]
    bm = bmesh.new()
    for i in slot_cols:
        secs = []
        for j in range(v0 - 1, v1 + 2):
            p = surf(i + 0.15, j)
            q = surf(i + 1.25, j)
            lift = Vector((0, 0, 0.022))
            a = p + Vector((0, 0, 0.002))
            b = q + lift
            n = Vector((0, 0, 0.0025))
            secs.append([a, b, b + n, a + n])
        loft_bm(bm, secs, cap_ends=True)
    objs.append(obj_from_bm('blades', bm, 'carbon', smooth=40))
    return objs


@part('pc_cooling_louvres')
def pc_cooling_louvres():
    objs = louvre_panel()
    transform(objs, rot('Z', -24) @ rot('X', 28))
    return objs


@part('pc_integrated_thermal')
def pc_integrated_thermal():
    objs = []
    for side in (-1, 1):
        r = radiator(w=0.30, h=0.26, d=0.045, tubes=20, pipes=False)
        transform(r, tr(side * 0.23, 0.10, 0.0) @ rot('Z', side * 28))
        objs += r
    bm = bmesh.new()
    tube_bm(bm, bezier((-0.36, 0.17, 0.10), (-0.40, -0.10, 0.20), (0.40, -0.10, 0.20), (0.36, 0.17, 0.10), 24), 0.017,
            segs=16)
    tube_bm(bm, bezier((-0.36, 0.17, -0.10), (-0.30, -0.18, -0.20), (0.0, -0.20, -0.22), (0.0, -0.20, -0.10), 18), 0.017,
            segs=16)
    tube_bm(bm, bezier((0.36, 0.17, -0.10), (0.30, -0.18, -0.20), (0.06, -0.20, -0.22), (0.06, -0.20, -0.10), 18), 0.017,
            segs=16)
    objs.append(obj_from_bm('hoses', bm, 'black_plastic', smooth=40))
    bm = bmesh.new()
    lathe_bm(bm, [(0.0, -0.10), (0.06, -0.10), (0.065, -0.09), (0.065, 0.02), (0.05, 0.035), (0.0, 0.035)], segs=40,
             matrix=tr(0.03, -0.20, -0.06))
    for i in range(6):
        cyl_bm(bm, 0.006, 0.012, segs=10, matrix=tr(0.03, -0.20, -0.06) @ Matrix.Rotation(TAU * i / 6, 4, 'Z') @
               tr(0.057, 0, 0.04))
    objs.append(obj_from_bm('pump', bm, 'alu', smooth=40))
    lp = louvre_panel(nu=20, nv=12, su=0.42, sv=0.26, slots=5, bend=1.4)
    transform(lp, tr(0.0, 0.16, 0.24) @ rot('X', 6))
    objs += lp
    transform(objs, rot('Z', -22) @ rot('X', 10))
    return objs
