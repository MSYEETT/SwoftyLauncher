import math

import bmesh
import bpy
from mathutils import Matrix, Vector

TAU = math.tau


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.lights, bpy.data.cameras):
        for block in list(coll):
            coll.remove(block)


def _bsdf(name):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new('ShaderNodeOutputMaterial')
    b = nt.nodes.new('ShaderNodeBsdfPrincipled')
    nt.links.new(b.outputs['BSDF'], out.inputs['Surface'])
    return m, nt, b


def _set(b, **kw):
    names = {
        'base': 'Base Color', 'metal': 'Metallic', 'rough': 'Roughness', 'coat': 'Coat Weight',
        'coat_rough': 'Coat Roughness', 'spec': 'Specular IOR Level', 'aniso': 'Anisotropic',
    }
    for k, v in kw.items():
        sock = b.inputs[names[k]]
        sock.default_value = (*v, 1.0) if k == 'base' else v


def _coords(nt, scale):
    tc = nt.nodes.new('ShaderNodeTexCoord')
    mp = nt.nodes.new('ShaderNodeMapping')
    mp.inputs['Scale'].default_value = (scale, scale, scale)
    nt.links.new(tc.outputs['Object'], mp.inputs['Vector'])
    return mp.outputs['Vector']


def _bump(nt, b, height_socket, strength, distance=0.002):
    bump = nt.nodes.new('ShaderNodeBump')
    bump.inputs['Strength'].default_value = strength
    bump.inputs['Distance'].default_value = distance
    nt.links.new(height_socket, bump.inputs['Height'])
    nt.links.new(bump.outputs['Normal'], b.inputs['Normal'])


def _noise_rough(nt, b, lo, hi, scale=60.0):
    nz = nt.nodes.new('ShaderNodeTexNoise')
    nz.inputs['Scale'].default_value = scale
    nz.inputs['Detail'].default_value = 6.0
    ramp = nt.nodes.new('ShaderNodeMapRange')
    ramp.inputs['From Min'].default_value = 0.3
    ramp.inputs['From Max'].default_value = 0.7
    ramp.inputs['To Min'].default_value = lo
    ramp.inputs['To Max'].default_value = hi
    nt.links.new(nz.outputs['Fac'], ramp.inputs['Value'])
    nt.links.new(ramp.outputs['Result'], b.inputs['Roughness'])
    return nz


MATS = {}


def mat(kind):
    if kind in MATS:
        return MATS[kind]
    m, nt, b = _bsdf(kind)
    if kind == 'carbon':
        _set(b, base=(0.018, 0.019, 0.021), metal=0.0, rough=0.42, coat=1.0, coat_rough=0.06, spec=0.5)
        v = _coords(nt, 1.0)
        chk = nt.nodes.new('ShaderNodeTexChecker')
        chk.inputs['Scale'].default_value = 420.0
        chk.inputs['Color1'].default_value = (0.034, 0.035, 0.038, 1)
        chk.inputs['Color2'].default_value = (0.010, 0.010, 0.012, 1)
        nt.links.new(v, chk.inputs['Vector'])
        nt.links.new(chk.outputs['Color'], b.inputs['Base Color'])
        _bump(nt, b, chk.outputs['Fac'], 0.25, 0.0006)
    elif kind == 'carbon_matte':
        _set(b, base=(0.035, 0.034, 0.033), metal=0.0, rough=0.75, spec=0.35)
        nz = _noise_rough(nt, b, 0.6, 0.85, 140.0)
        _bump(nt, b, nz.outputs['Fac'], 0.25, 0.0008)
    elif kind == 'cc_disc':
        _set(b, base=(0.016, 0.0155, 0.015), metal=0.0, rough=0.62, spec=0.45)
        nz = _noise_rough(nt, b, 0.55, 0.85, 220.0)
        _bump(nt, b, nz.outputs['Fac'], 0.35, 0.0006)
    elif kind == 'hole':
        _set(b, base=(0.004, 0.004, 0.004), metal=0.0, rough=0.9, spec=0.1)
    elif kind == 'alu':
        _set(b, base=(0.62, 0.62, 0.63), metal=1.0, rough=0.28, aniso=0.3)
        _noise_rough(nt, b, 0.22, 0.36, 90.0)
    elif kind == 'alu_dark':
        _set(b, base=(0.22, 0.22, 0.235), metal=1.0, rough=0.32)
        _noise_rough(nt, b, 0.26, 0.4, 90.0)
    elif kind == 'titanium':
        _set(b, base=(0.42, 0.40, 0.38), metal=1.0, rough=0.3)
        _noise_rough(nt, b, 0.24, 0.38, 120.0)
    elif kind == 'steel':
        _set(b, base=(0.55, 0.56, 0.58), metal=1.0, rough=0.18)
    elif kind == 'rubber':
        _set(b, base=(0.016, 0.016, 0.017), metal=0.0, rough=0.62, spec=0.45)
        nz = _noise_rough(nt, b, 0.5, 0.72, 300.0)
        _bump(nt, b, nz.outputs['Fac'], 0.08, 0.0005)
    elif kind == 'rubber_worn':
        _set(b, base=(0.03, 0.029, 0.028), metal=0.0, rough=0.85, spec=0.3)
        nz = _noise_rough(nt, b, 0.75, 0.95, 400.0)
        _bump(nt, b, nz.outputs['Fac'], 0.4, 0.0005)
    elif kind == 'rim':
        _set(b, base=(0.025, 0.025, 0.027), metal=0.6, rough=0.35, spec=0.5)
    elif kind == 'gold':
        _set(b, base=(1.0, 0.72, 0.32), metal=1.0, rough=0.22)
        nz = nt.nodes.new('ShaderNodeTexVoronoi')
        nz.inputs['Scale'].default_value = 90.0
        nt.links.new(_coords(nt, 1.0), nz.inputs['Vector'])
        _bump(nt, b, nz.outputs['Distance'], 0.35, 0.0015)
    elif kind == 'copper':
        _set(b, base=(0.95, 0.55, 0.35), metal=1.0, rough=0.3)
    elif kind == 'red':
        _set(b, base=(0.55, 0.015, 0.012), metal=0.0, rough=0.35, coat=0.6, coat_rough=0.1)
    elif kind == 'red_anod':
        _set(b, base=(0.65, 0.04, 0.03), metal=1.0, rough=0.3)
    elif kind == 'paint_red':
        _set(b, base=(0.75, 0.02, 0.02), metal=0.0, rough=0.5)
    elif kind == 'paint_yellow':
        _set(b, base=(0.95, 0.62, 0.02), metal=0.0, rough=0.5)
    elif kind == 'paint_white':
        _set(b, base=(0.85, 0.85, 0.85), metal=0.0, rough=0.5)
    elif kind == 'paint_green':
        _set(b, base=(0.05, 0.55, 0.12), metal=0.0, rough=0.5)
    elif kind == 'paint_blue':
        _set(b, base=(0.03, 0.22, 0.75), metal=0.0, rough=0.5)
    elif kind == 'black_plastic':
        _set(b, base=(0.02, 0.02, 0.022), metal=0.0, rough=0.45, spec=0.5)
    elif kind == 'pcb':
        _set(b, base=(0.02, 0.12, 0.05), metal=0.0, rough=0.4)
    elif kind == 'fin':
        _set(b, base=(0.58, 0.58, 0.6), metal=1.0, rough=0.38)
    elif kind == 'fin_core':
        _set(b, base=(0.30, 0.30, 0.31), metal=1.0, rough=0.42)
        wv = nt.nodes.new('ShaderNodeTexWave')
        wv.inputs['Scale'].default_value = 260.0
        wv.inputs['Distortion'].default_value = 0.0
        nt.links.new(_coords(nt, 1.0), wv.inputs['Vector'])
        _bump(nt, b, wv.outputs['Fac'], 0.9, 0.002)
    elif kind == 'ceramic':
        _set(b, base=(0.78, 0.76, 0.72), metal=0.0, rough=0.55)
    elif kind == 'carcass':
        _set(b, base=(0.32, 0.30, 0.26), metal=0.0, rough=0.7)
    elif kind == 'belt':
        _set(b, base=(0.42, 0.42, 0.44), metal=0.9, rough=0.45)
    elif kind == 'liner':
        _set(b, base=(0.06, 0.06, 0.065), metal=0.0, rough=0.8)
    else:
        raise KeyError(kind)
    MATS[kind] = m
    return m


def obj_from_bm(name, bm, material, smooth=35.0):
    me = bpy.data.meshes.new(name)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    if isinstance(material, (list, tuple)):
        for m in material:
            me.materials.append(mat(m))
    else:
        me.materials.append(mat(material))
    finish(ob, smooth)
    return ob


def finish(ob, smooth=35.0):
    me = ob.data
    if smooth is None:
        me.shade_flat()
    else:
        me.shade_smooth()
        try:
            me.set_sharp_from_angle(angle=math.radians(smooth))
        except AttributeError:
            pass


def lathe_bm(bm, prof, segs=64, angle=TAU, start=0.0, closed=True, caps=True, matrix=None, mat_index=0):
    full = abs(angle - TAU) < 1e-6
    rings = segs if full else segs + 1
    grid = []
    for i in range(rings):
        a = start + angle * i / segs
        ca, sa = math.cos(a), math.sin(a)
        ring = []
        for r, z in prof:
            p = Vector((r * ca, r * sa, z))
            if matrix is not None:
                p = matrix @ p
            ring.append(bm.verts.new(p))
        grid.append(ring)
    n = len(prof)
    faces = []
    for i in range(segs):
        a, b = grid[i], grid[(i + 1) % rings]
        for j in range(n if closed else n - 1):
            k = (j + 1) % n
            try:
                f = bm.faces.new((a[j], b[j], b[k], a[k]))
                f.material_index = mat_index
                faces.append(f)
            except ValueError:
                pass
    if caps and closed and not full:
        for ring in (grid[0], grid[-1]):
            f = bm.faces.new(ring)
            f.material_index = mat_index
            faces.append(f)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-7)
    return faces


def lathe(name, prof, material, segs=64, angle=TAU, start=0.0, closed=True, caps=True, matrix=None, smooth=35.0):
    bm = bmesh.new()
    lathe_bm(bm, prof, segs, angle, start, closed, caps, matrix)
    return obj_from_bm(name, bm, material, smooth)


def loft_bm(bm, sections, closed_loop=True, cap_ends=False, mat_index=0):
    grid = [[bm.verts.new(Vector(p)) for p in sec] for sec in sections]
    n = len(sections[0])
    faces = []
    for i in range(len(grid) - 1):
        a, b = grid[i], grid[i + 1]
        for j in range(n if closed_loop else n - 1):
            k = (j + 1) % n
            f = bm.faces.new((a[j], b[j], b[k], a[k]))
            f.material_index = mat_index
            faces.append(f)
    if cap_ends:
        for ring in (grid[0], grid[-1]):
            f = bm.faces.new(ring)
            f.material_index = mat_index
    return grid


def rounded_rect(w, h, r, n_corner=5):
    pts = []
    corners = [(w / 2 - r, h / 2 - r, 0), (-w / 2 + r, h / 2 - r, 90), (-w / 2 + r, -h / 2 + r, 180), (w / 2 - r, -h / 2 + r, 270)]
    for cx, cy, a0 in corners:
        for i in range(n_corner + 1):
            a = math.radians(a0 + 90 * i / n_corner)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def box_bm(bm, size, matrix=Matrix(), mat_index=0, bevel=0.0):
    res = bmesh.ops.create_cube(bm, size=1.0)
    verts = res['verts']
    sx, sy, sz = size
    for v in verts:
        v.co = matrix @ Vector((v.co.x * sx, v.co.y * sy, v.co.z * sz))
    faces = list({f for v in verts for f in v.link_faces})
    for f in faces:
        f.material_index = mat_index
    if bevel > 0:
        edges = list({e for v in verts for e in v.link_edges})
        bmesh.ops.bevel(bm, geom=edges, offset=bevel, segments=1, affect='EDGES', profile=0.5)
    return verts


def cyl_bm(bm, r, depth, segs=24, matrix=Matrix(), mat_index=0, r2=None):
    res = bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=segs, radius1=r,
                                radius2=r if r2 is None else r2, depth=depth, matrix=matrix)
    for v in res['verts']:
        for f in v.link_faces:
            f.material_index = mat_index
    return res['verts']


def solidify_bm(bm, faces, thickness):
    res = bmesh.ops.solidify(bm, geom=faces, thickness=thickness)
    return res


def bevel_obj(ob, width, segments=1, angle=40.0):
    mod = ob.modifiers.new('bevel', 'BEVEL')
    mod.width = width
    mod.segments = segments
    mod.limit_method = 'ANGLE'
    mod.angle_limit = math.radians(angle)
    mod.harden_normals = False
    apply_mods(ob)


def apply_mods(ob):
    dg = bpy.context.evaluated_depsgraph_get()
    ev = ob.evaluated_get(dg)
    me = bpy.data.meshes.new_from_object(ev)
    old = ob.data
    ob.modifiers.clear()
    ob.data = me
    bpy.data.meshes.remove(old)


def transform(objs, matrix):
    for ob in objs:
        ob.matrix_world = matrix @ ob.matrix_world


def rot(axis, deg):
    return Matrix.Rotation(math.radians(deg), 4, axis)


def tr(x, y, z):
    return Matrix.Translation((x, y, z))


def mark_all_edges():
    for ob in bpy.context.scene.objects:
        if ob.type != 'MESH':
            continue
        me = ob.data
        attr = me.attributes.get('freestyle_edge')
        if attr is None:
            attr = me.attributes.new('freestyle_edge', 'BOOLEAN', 'EDGE')
        attr.data.foreach_set('value', [True] * len(me.edges))
        try:
            for e in me.edges:
                e.use_freestyle_mark = True
        except AttributeError:
            pass


def world():
    w = bpy.data.worlds.new('studio')
    bpy.context.scene.world = w
    w.use_nodes = True
    nt = w.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new('ShaderNodeOutputWorld')
    bg = nt.nodes.new('ShaderNodeBackground')
    tc = nt.nodes.new('ShaderNodeTexCoord')
    sep = nt.nodes.new('ShaderNodeSeparateXYZ')
    ramp = nt.nodes.new('ShaderNodeValToRGB')
    ramp.color_ramp.elements[0].position = 0.35
    ramp.color_ramp.elements[0].color = (0.0, 0.0, 0.0, 1)
    ramp.color_ramp.elements[1].position = 0.95
    ramp.color_ramp.elements[1].color = (0.9, 0.9, 0.92, 1)
    mr = nt.nodes.new('ShaderNodeMapRange')
    mr.inputs['From Min'].default_value = -1.0
    mr.inputs['From Max'].default_value = 1.0
    nt.links.new(tc.outputs['Generated'], sep.inputs['Vector'])
    nt.links.new(sep.outputs['Z'], mr.inputs['Value'])
    nt.links.new(mr.outputs['Result'], ramp.inputs['Fac'])
    nt.links.new(ramp.outputs['Color'], bg.inputs['Color'])
    bg.inputs['Strength'].default_value = 0.22
    nt.links.new(bg.outputs['Background'], out.inputs['Surface'])


def area_light(name, loc, target, energy, size, color=(1, 1, 1)):
    ld = bpy.data.lights.new(name, 'AREA')
    ld.energy = energy
    ld.size = size
    ld.color = color
    ob = bpy.data.objects.new(name, ld)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = loc
    d = Vector(target) - Vector(loc)
    ob.rotation_euler = d.to_track_quat('-Z', 'Y').to_euler()
    return ob


def lights(scale=1.0):
    s = scale
    area_light('key', (-2.2 * s, -2.6 * s, 3.2 * s), (0, 0, 0), 260 * s * s, 2.6 * s)
    area_light('rim', (2.6 * s, 2.2 * s, 2.0 * s), (0, 0, 0), 520 * s * s, 1.4 * s)
    area_light('kick', (3.0 * s, -1.6 * s, 0.4 * s), (0, 0, 0), 160 * s * s, 1.0 * s)
    area_light('fill', (-3.0 * s, 0.6 * s, -0.6 * s), (0, 0, 0), 25 * s * s, 3.0 * s)


VIEW_DIR = Vector((-0.62, -1.0, 0.62)).normalized()


def mesh_points():
    pts = []
    dg = bpy.context.evaluated_depsgraph_get()
    for ob in bpy.context.scene.objects:
        if ob.type != 'MESH':
            continue
        ev = ob.evaluated_get(dg)
        mw = ev.matrix_world
        for v in ev.data.vertices:
            pts.append(mw @ v.co)
    return pts


def camera(fill=0.80, lens=70.0, view=VIEW_DIR, offset=(0.0, 0.0)):
    from bpy_extras.object_utils import world_to_camera_view
    sc = bpy.context.scene
    cd = bpy.data.cameras.new('cam')
    cd.lens = lens
    cam = bpy.data.objects.new('cam', cd)
    sc.collection.objects.link(cam)
    sc.camera = cam
    pts = mesh_points()
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    centre = (lo + hi) / 2
    radius = max((p - centre).length for p in pts)
    dist = radius * lens / 18.0
    for _ in range(6):
        cam.location = centre + view * dist
        cam.rotation_euler = (-view).to_track_quat('-Z', 'Y').to_euler()
        bpy.context.view_layer.update()
        cd.shift_x = cd.shift_y = 0.0
        bpy.context.view_layer.update()
        uv = [world_to_camera_view(sc, cam, p) for p in pts]
        x0, x1 = min(u.x for u in uv), max(u.x for u in uv)
        y0, y1 = min(u.y for u in uv), max(u.y for u in uv)
        ext = max(x1 - x0, y1 - y0)
        dist *= ext / fill
    cam.location = centre + view * dist
    bpy.context.view_layer.update()
    uv = [world_to_camera_view(sc, cam, p) for p in pts]
    x0, x1 = min(u.x for u in uv), max(u.x for u in uv)
    y0, y1 = min(u.y for u in uv), max(u.y for u in uv)
    cd.shift_x = (x0 + x1) / 2 - 0.5 - offset[0]
    cd.shift_y = (y0 + y1) / 2 - 0.5 - offset[1]
    cd.clip_start = dist * 0.05
    cd.clip_end = dist * 10
    return cam, radius


def setup_render(size):
    sc = bpy.context.scene
    sc.render.engine = 'CYCLES'
    sc.cycles.device = 'CPU'
    sc.render.resolution_x = sc.render.resolution_y = size
    sc.render.resolution_percentage = 100
    sc.render.film_transparent = True
    sc.render.image_settings.file_format = 'PNG'
    sc.render.image_settings.color_mode = 'RGBA'
    sc.render.image_settings.color_depth = '8'
    sc.view_settings.view_transform = 'AgX'
    try:
        sc.view_settings.look = 'AgX - Medium High Contrast'
    except TypeError:
        pass
    sc.view_settings.exposure = 0.0
    sc.cycles.use_denoising = True
    sc.cycles.samples = 128
    sc.cycles.max_bounces = 6
    sc.render.threads_mode = 'FIXED'
    sc.render.threads = 24


def render_beauty(path):
    sc = bpy.context.scene
    sc.render.use_freestyle = False
    sc.view_layers[0].material_override = None
    sc.cycles.samples = 128
    sc.cycles.use_denoising = True
    sc.render.filepath = path
    bpy.ops.render.render(write_still=True)


def render_wire(path, thickness=1.5, outline=2.2):
    sc = bpy.context.scene
    mark_all_edges()
    hold = bpy.data.materials.new('holdout')
    hold.use_nodes = True
    nt = hold.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    o = nt.nodes.new('ShaderNodeOutputMaterial')
    h = nt.nodes.new('ShaderNodeHoldout')
    nt.links.new(h.outputs['Holdout'], o.inputs['Surface'])
    vl = sc.view_layers[0]
    vl.material_override = hold
    sc.cycles.samples = 1
    sc.cycles.use_denoising = False
    sc.view_settings.view_transform = 'Standard'
    sc.view_settings.look = 'None'
    sc.render.use_freestyle = True
    sc.render.line_thickness_mode = 'ABSOLUTE'
    sc.render.line_thickness = 1.0
    fs = vl.freestyle_settings
    fs.mode = 'EDITOR'
    fs.crease_angle = math.radians(134)
    for ls in list(fs.linesets):
        fs.linesets.remove(ls)
    for name, th, kinds in (('edges', thickness, ('edge_mark', 'crease')), ('outline', outline, ('silhouette', 'border'))):
        ls = fs.linesets.new(name)
        ls.select_by_visibility = True
        ls.visibility = 'VISIBLE'
        ls.select_by_edge_types = True
        for k in ('silhouette', 'border', 'crease', 'edge_mark', 'contour', 'external_contour', 'material_boundary'):
            setattr(ls, 'select_' + k, k in kinds)
        st = bpy.data.linestyles.new(name)
        st.color = (1, 1, 1)
        st.alpha = 1.0
        st.thickness = th
        st.thickness_position = 'CENTER'
        st.caps = 'ROUND'
        st.chaining = 'PLAIN'
        ls.linestyle = st
    sc.render.filepath = path
    bpy.ops.render.render(write_still=True)


def orient(x_to, z_to):
    x = Vector(x_to).normalized()
    z = Vector(z_to)
    z = (z - x * z.dot(x)).normalized()
    y = z.cross(x)
    m = Matrix((x, y, z)).transposed().to_4x4()
    return m


def tube_bm(bm, path, r, segs=16, mat_index=0, caps=True):
    secs = []
    up = Vector((0, 0, 1))
    for i, p in enumerate(path):
        p = Vector(p)
        t = (Vector(path[min(i + 1, len(path) - 1)]) - Vector(path[max(i - 1, 0)])).normalized()
        if abs(t.dot(up)) > 0.95:
            up = Vector((1, 0, 0))
        n = t.cross(up).normalized()
        b = n.cross(t).normalized()
        rr = r(i) if callable(r) else r
        secs.append([p + (n * math.cos(TAU * k / segs) + b * math.sin(TAU * k / segs)) * rr for k in range(segs)])
    loft_bm(bm, secs, cap_ends=caps, mat_index=mat_index)


def bezier(p0, p1, p2, p3, n=16):
    p0, p1, p2, p3 = map(Vector, (p0, p1, p2, p3))
    out = []
    for i in range(n + 1):
        t = i / n
        out.append((1 - t) ** 3 * p0 + 3 * (1 - t) ** 2 * t * p1 + 3 * (1 - t) * t * t * p2 + t ** 3 * p3)
    return out
