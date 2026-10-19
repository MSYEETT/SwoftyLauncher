import math
import os
import sys
import uuid

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import idf as I
from pkg import Arr, Obj, Val

IDENT = (1.0, 0.0, 0.0, 0.0,
         0.0, 1.0, 0.0, 0.0,
         0.0, 0.0, 1.0, 0.0,
         0.0, 0.0, 0.0, 1.0)

TRANSFORM_FIELDS = {'position': 'vec3', 'scale': 'vec3', 'rotation': 'quat'}
EDITABLE_PARAM_TYPES = {'fp32', 'si32', 'ui32', 'ui16', 'ui08', 'bool', 'vec3', 'vec4', 'quat'}
TEXT_HEIGHT_PER_PT = 0.0122
MAX_NAME_BYTES = 200


class OpError(Exception):
    def __init__(self, index, message):
        super().__init__(message)
        self.index = index
        self.message = message


def quat_unit(q):
    x, y, z, w = (list(q or []) + [0.0, 0.0, 0.0, 0.0])[:4]
    n = math.sqrt(x * x + y * y + z * z + w * w)
    if n < 1e-9:
        return 0.0, 0.0, 0.0, 1.0
    return x / n, y / n, z / n, w / n


def reveal_scale(s):
    return [1.0 if abs(v) < 1e-3 else v for v in (list(s or []) + [1.0, 1.0, 1.0])[:3]]


def mat_trs(p, q, s):
    px, py, pz = (list(p or []) + [0.0, 0.0, 0.0])[:3]
    sx, sy, sz = (list(s or []) + [1.0, 1.0, 1.0])[:3]
    x, y, z, w = quat_unit(q)
    r00 = 1 - 2 * (y * y + z * z); r01 = 2 * (x * y - z * w); r02 = 2 * (x * z + y * w)
    r10 = 2 * (x * y + z * w); r11 = 1 - 2 * (x * x + z * z); r12 = 2 * (y * z - x * w)
    r20 = 2 * (x * z - y * w); r21 = 2 * (y * z + x * w); r22 = 1 - 2 * (x * x + y * y)
    return (r00 * sx, r01 * sy, r02 * sz, px,
            r10 * sx, r11 * sy, r12 * sz, py,
            r20 * sx, r21 * sy, r22 * sz, pz,
            0.0, 0.0, 0.0, 1.0)


def mat_mul(a, b):
    out = [0.0] * 16
    for r in range(4):
        for c in range(4):
            out[r * 4 + c] = (a[r * 4] * b[c] + a[r * 4 + 1] * b[4 + c]
                              + a[r * 4 + 2] * b[8 + c] + a[r * 4 + 3] * b[12 + c])
    return tuple(out)


def mat_point(m, v):
    x, y, z = v
    return (m[0] * x + m[1] * y + m[2] * z + m[3],
            m[4] * x + m[5] * y + m[6] * z + m[7],
            m[8] * x + m[9] * y + m[10] * z + m[11])


def box_corners(mn, mx):
    return [(x, y, z) for x in (mn[0], mx[0]) for y in (mn[1], mx[1]) for z in (mn[2], mx[2])]


def jsonable(v):
    if isinstance(v, bool):
        return v
    if isinstance(v, int):
        return str(v) if abs(v) > 2 ** 53 else v
    if isinstance(v, float):
        return v if math.isfinite(v) else None
    if isinstance(v, (list, tuple)):
        return [jsonable(x) for x in v]
    if isinstance(v, dict):
        return {k: jsonable(x) for k, x in v.items()}
    return v


def _num(v):
    try:
        f = float(v)
        return f if math.isfinite(f) else None
    except (TypeError, ValueError):
        return None


def child_objs(obj):
    ca = obj.get('children')
    if not isinstance(ca, Arr):
        return []
    return [it for it in ca.items if isinstance(it, Obj)]


def text_box(params):
    h = _num(params.get('previewHeight')) or _num(params.get('height'))
    if not h:
        style = str(params.get('style') or '')
        pt = _num(style.split('_', 1)[0]) if style else None
        h = pt * TEXT_HEIGHT_PER_PT if pt else 0.25
    bw = _num(params.get('boxWidth')) or _num(params.get('previewWidth')) or max(h * 4, 1.0)
    align = str(params.get('halign') or 'left').lower()
    if align.startswith('right'):
        x0, x1 = -bw, 0.0
    elif align.startswith('cent'):
        x0, x1 = -bw / 2, bw / 2
    else:
        x0, x1 = 0.0, bw
    return [x0, -h / 2, 0.0], [x1, h / 2, 0.0]


def local_box(e):
    p = e.params
    mn, mx = p.get('minExtents'), p.get('maxExtents')
    if isinstance(mn, list) and isinstance(mx, list) and len(mn) >= 3 and len(mx) >= 3:
        return [float(x) for x in mn[:3]], [float(x) for x in mx[:3]], 'extents'
    boxes = [(g.get('extentsmin'), g.get('extentsmax')) for g in e.geoms]
    boxes = [(a, b) for a, b in boxes if isinstance(a, list) and isinstance(b, list)]
    if boxes:
        lo = [min(a[i] for a, _ in boxes) for i in range(3)]
        hi = [max(b[i] for _, b in boxes) for i in range(3)]
        return lo, hi, 'geom'
    if e.type == 'eText':
        lo, hi = text_box(p)
        return lo, hi, 'text'
    w = _num(p.get('width'))
    h = _num(p.get('length')) or _num(p.get('height'))
    if w and h:
        return [-w / 2, -h / 2, 0.0], [w / 2, h / 2, 0.0], 'size'
    return None


class XrefResolver:
    def __init__(self, catalog, max_depth=5, max_items=3000):
        self.catalog = catalog
        self.max_depth = max_depth
        self.max_items = max_items
        self.cache = {}

    def get(self, eaid, prefer=None, stack=(), reveal=False):
        ck = (eaid, reveal)
        if ck in self.cache:
            return self.cache[ck]
        if eaid in stack:
            return {'found': False, 'error': 'reference cycle'}
        if len(stack) >= self.max_depth:
            return {'found': False, 'error': 'nesting too deep'}
        if self.catalog is None:
            return {'found': False, 'error': 'no ERP catalog loaded'}
        try:
            rel, data = self.catalog.read_scene(eaid, prefer)
            doc = I.Idf(data)
        except KeyError:
            res = {'found': False, 'error': 'not in any UI ERP'}
            self.cache[ck] = res
            return res
        except Exception as ex:
            res = {'found': False, 'error': f'parse failed: {ex}'}
            self.cache[ck] = res
            return res
        items = []
        truncated = [False]

        def add(item, clip):
            if len(items) >= self.max_items:
                truncated[0] = True
                return
            if clip is not None:
                item = clip_item(item, clip)
                if item is None:
                    return
            items.append(item)

        def walk(e, parent_m, depth, clip, layer):
            m = mat_mul(parent_m, mat_trs(e.position, e.rotation, reveal_scale(e.scale) if reveal else e.scale))
            layer = effective_layer(e.params, layer)
            box = local_box(e)
            if e.type == 'eXRef' and e.params.get('fileRef'):
                sub = self.get(e.params['fileRef'], rel, stack + (eaid,), reveal)
                if sub.get('found'):
                    for it in sub['items']:
                        inner = dict(it, m=mat_mul(m, it['m']), d=depth + 1 + it['d'])
                        if it.get('L') is None:
                            inner['L'] = layer
                        add(inner, clip)
                    if sub.get('min') is not None:
                        box = (sub['min'], sub['max'], 'xref')
            if box:
                item = {'n': e.name, 't': e.type, 'm': m, 'min': box[0], 'max': box[1],
                        's': box[2], 'd': depth, 'L': layer, 'e': rel}
                if e.geoms:
                    item['g'] = [[g.get('resourceid'), g.get('materialid')] for g in e.geoms]
                if e.type == 'eText':
                    item['a'] = e.params.get('halign') or 'left'
                    item['c'] = e.params.get('textcase')
                if e.type != 'eXRef':
                    add(item, clip)
                else:
                    add(dict(item, g=None), clip)
                if e.type == 'eWindow':
                    lo, hi = bounds_of([{'m': m, 'min': box[0], 'max': box[1]}])
                    if hi[0] - lo[0] > 1e-4 and hi[1] - lo[1] > 1e-4:
                        clip = (lo[:2], hi[:2]) if clip is None else (
                            [max(lo[0], clip[0][0]), max(lo[1], clip[0][1])],
                            [min(hi[0], clip[1][0]), min(hi[1], clip[1][1])])
            for c in e.children:
                walk(c, m, depth + 1, clip, layer)

        for sc in doc.scenes:
            if sc.root:
                walk(sc.root, IDENT, 0, None, None)
        lo, hi = bounds_of(items)
        res = {'found': True, 'erp': rel, 'items': items, 'min': lo, 'max': hi,
               'truncated': truncated[0], 'count': len(items)}
        self.cache[ck] = res
        return res

    def summary(self, eaid, prefer=None):
        r = self.get(eaid, prefer)
        return {k: v for k, v in r.items() if k != 'items'}

    def ghosts(self, eaid, prefer=None, reveal=False):
        r = self.get(eaid, prefer, (), reveal)
        out = dict(r)
        out['items'] = [dict(it, m=[round(x, 6) for x in it['m']]) for it in r.get('items', [])]
        return out


def bounds_of(items):
    lo = [math.inf] * 3
    hi = [-math.inf] * 3
    for it in items:
        for c in box_corners(it['min'], it['max']):
            w = mat_point(it['m'], c)
            for i in range(3):
                lo[i] = min(lo[i], w[i])
                hi[i] = max(hi[i], w[i])
    if lo[0] == math.inf:
        return None, None
    return [round(x, 6) for x in lo], [round(x, 6) for x in hi]


def effective_layer(params, parent):
    v = params.get('eLayer.layer')
    if not isinstance(v, int):
        return parent
    if params.get('eLayer.layerIsOffset') and parent is not None:
        return parent + v
    return v


def clip_item(item, clip):
    lo, hi = bounds_of([item])
    (cx0, cy0), (cx1, cy1) = clip
    if hi[0] <= cx0 or lo[0] >= cx1 or hi[1] <= cy0 or lo[1] >= cy1:
        return None
    if lo[0] >= cx0 and hi[0] <= cx1 and lo[1] >= cy0 and hi[1] <= cy1:
        return item
    return dict(item, m=IDENT, clipped=True,
                min=[max(lo[0], cx0), max(lo[1], cy0), lo[2]],
                max=[min(hi[0], cx1), min(hi[1], cy1), hi[2]])


def element_names(scene_entry):
    out = []

    def walk(obj):
        out.append(I._val(obj.get('name')))
        for c in child_objs(obj):
            walk(c)

    root = scene_entry.get('scene')
    if isinstance(root, Obj):
        walk(root)
    return out


def expected_lookup(scene_entry):
    return sorted({I.djb2(n) for n in element_names(scene_entry) if n is not None})


def resolve_key(doc, key):
    parts = str(key).split('.')
    try:
        si = int(parts[0])
        entry = doc.scenes[si].entry
    except (ValueError, IndexError):
        raise KeyError(f'bad element key {key!r}')
    obj = entry.get('scene')
    parent = None
    path = None
    for idx in parts[1:]:
        kids = child_objs(obj)
        try:
            i = int(idx)
            child = kids[i]
        except (ValueError, IndexError):
            raise KeyError(f'bad element key {key!r}')
        parent, obj = obj, child
        name = I._val(obj.get('name'))
        path = name if path is None else f'{path}.{name}'
    return entry, parent, obj, path


def coerce(vtype, value, old):
    if vtype in ('vec3', 'vec4', 'quat'):
        vals = [float(x) for x in value]
        if len(vals) < 4:
            pad = list(old)[len(vals):4] if isinstance(old, list) else []
            vals += [float(x) for x in pad] + [0.0] * (4 - len(vals) - len(pad))
        if not all(math.isfinite(x) for x in vals):
            raise ValueError('non-finite value')
        return vals[:4]
    if vtype == 'fp32':
        f = float(value)
        if not math.isfinite(f):
            raise ValueError('non-finite value')
        return f
    if vtype == 'bool':
        if isinstance(value, str):
            return value.strip().lower() in ('1', 'true', 'yes', 'on')
        return bool(value)
    if vtype in ('si32', 'ui32', 'ui16', 'ui08'):
        n = int(value)
        lim = {'si32': (-2 ** 31, 2 ** 31 - 1), 'ui32': (0, 2 ** 32 - 1),
               'ui16': (0, 2 ** 16 - 1), 'ui08': (0, 255)}[vtype]
        if not lim[0] <= n <= lim[1]:
            raise ValueError(f'{n} out of range for {vtype}')
        return n
    raise ValueError(f'{vtype} values are read-only in this editor')


def check_name(name, entry, exclude=None):
    if not isinstance(name, str) or not name.strip():
        raise ValueError('name is empty')
    if '.' in name:
        raise ValueError("name must not contain '.' (element ids hash the dotted path)")
    if len(name.encode()) > MAX_NAME_BYTES:
        raise ValueError('name too long')
    taken = element_names(entry)
    if exclude is not None and exclude in taken:
        taken.remove(exclude)
    if name in taken:
        raise ValueError(f'an element named {name!r} already exists in this scene')


def regen_guids(obj, path):
    pa = obj.get('params')
    if isinstance(pa, Arr):
        for it in pa.items:
            if isinstance(it, Obj) and I._val(it.get('name')) == 'guid':
                old = I._val(it.get('value'))
                it.set('value', Val('stri', str(uuid.uuid5(uuid.NAMESPACE_URL, f'swofty-ui:{old}:{path}'))))
    for c in child_objs(obj):
        regen_guids(c, f'{path}.{I._val(c.get("name"))}')


def apply_ops(doc, ops):
    touched = set()
    results = []
    for i, op in enumerate(ops):
        try:
            results.append(apply_op(doc, op, touched))
        except (KeyError, ValueError, TypeError) as ex:
            raise OpError(i, str(ex).strip("'\""))
    for entry in touched:
        nl = entry.get('nodeidlookup')
        ids = [Val('ui32', h) for h in expected_lookup(entry)]
        if isinstance(nl, Arr):
            nl.items[:] = ids
        else:
            entry.set('nodeidlookup', Arr(ids))
    return results


def apply_op(doc, op, touched):
    kind = op.get('op')
    entry, parent, obj, path = resolve_key(doc, op.get('key'))
    if kind == 'set':
        field = op.get('field')
        if field not in TRANSFORM_FIELDS:
            raise ValueError(f'cannot set {field!r}')
        old = obj.get(field)
        vtype = old.type if isinstance(old, Val) else TRANSFORM_FIELDS[field]
        obj.set(field, Val(vtype, coerce(vtype, op.get('value'), old.value if isinstance(old, Val) else None)))
        return {'key': op['key']}
    if kind == 'param':
        name = op.get('name')
        pa = obj.get('params')
        for it in (pa.items if isinstance(pa, Arr) else []):
            if isinstance(it, Obj) and I._val(it.get('name')) == name:
                old = it.get('value')
                if not isinstance(old, Val):
                    raise ValueError(f'param {name!r} is not a scalar')
                if old.type not in EDITABLE_PARAM_TYPES:
                    raise ValueError(f'param {name!r} ({old.type}) is read-only in this editor')
                it.set('value', Val(old.type, coerce(old.type, op.get('value'), old.value)))
                return {'key': op['key']}
        raise KeyError(f'no param {name!r} on this element')
    if parent is None:
        raise ValueError(f'cannot {kind} the scene root')
    siblings = child_objs(parent)
    parent_key = op['key'].rsplit('.', 1)[0]
    parent_path = path.rsplit('.', 1)[0] if '.' in path else ''
    if kind == 'clone':
        new_name = op.get('name')
        check_name(new_name, entry)
        clone = I.clone_node(obj)
        clone.set('name', Val('stri', new_name))
        if op.get('position') is not None:
            old = clone.get('position')
            vt = old.type if isinstance(old, Val) else 'vec3'
            clone.set('position', Val(vt, coerce(vt, op['position'], old.value if isinstance(old, Val) else None)))
        new_path = new_name if parent_path == '' else f'{parent_path}.{new_name}'
        regen_guids(clone, new_path)
        I.recompute_ids(clone, new_path, [])
        dup = element_names(Obj([(Val('stri', 'scene'), clone)]))[1:]
        parent.get('children').items.append(clone)
        touched.add(entry)
        out = {'key': f'{parent_key}.{len(siblings)}', 'path': new_path}
        if dup:
            out['warning'] = (f'{len(dup)} descendant name(s) now appear twice in the scene '
                              f'({", ".join(dup[:4])}{"…" if len(dup) > 4 else ""}); paths stay unique, '
                              f'but stock scenes never repeat a leaf name')
        return out
    if kind == 'rename':
        new_name = op.get('name')
        old_name = I._val(obj.get('name'))
        if new_name == old_name:
            return {'key': op['key']}
        check_name(new_name, entry, exclude=old_name)
        obj.set('name', Val('stri', new_name))
        new_path = new_name if parent_path == '' else f'{parent_path}.{new_name}'
        I.recompute_ids(obj, new_path, [])
        touched.add(entry)
        return {'key': op['key'], 'path': new_path}
    if kind == 'delete':
        items = parent.get('children').items
        for j, it in enumerate(items):
            if it is obj:
                del items[j]
                break
        touched.add(entry)
        return {'key': parent_key}
    raise ValueError(f'unknown op {kind!r}')


def param_list(obj):
    out = []
    pa = obj.get('params')
    for it in (pa.items if isinstance(pa, Arr) else []):
        if not isinstance(it, Obj):
            continue
        v = it.get('value')
        if isinstance(v, Val):
            out.append({'n': I._val(it.get('name')), 't': v.type, 'v': jsonable(v.value),
                        'ed': v.type in EDITABLE_PARAM_TYPES})
        else:
            out.append({'n': I._val(it.get('name')), 't': 'node', 'v': None, 'ed': False})
    return out


def build_model(doc, xrefs=None, prefer_erp=None, eaid=None):
    elements = {}
    scenes = []
    xref_summ = {}
    id_bad = 0
    for si, sc in enumerate(doc.scenes):
        entry = sc.entry
        root_obj = entry.get('scene')
        if not isinstance(root_obj, Obj):
            continue
        root = I.Element(root_obj)
        root.assign_paths()

        def walk(e, key, parent_key, depth):
            nonlocal id_bad
            kids = []
            for ci, c in enumerate(e.children):
                kids.append(walk(c, f'{key}.{ci}', key, depth + 1))
            box = local_box(e)
            file_ref = e.params.get('fileRef') if e.type == 'eXRef' else None
            if file_ref and xrefs is not None:
                if file_ref not in xref_summ:
                    xref_summ[file_ref] = xrefs.summary(file_ref, prefer_erp)
                s = xref_summ[file_ref]
                if s.get('found') and s.get('min') is not None:
                    box = (s['min'], s['max'], 'xref')
            ok = e.id_ok()
            id_bad += 0 if ok else 1
            rot = e.rotation if isinstance(e.rotation, list) else [0.0, 0.0, 0.0, 1.0]
            elements[key] = {
                'k': key, 'parent': parent_key, 'ch': kids, 'depth': depth,
                'name': e.name, 'path': e.path, 'id': e.element_id, 'idOk': ok,
                'type': e.type, 'pos': list(e.position or (0, 0, 0)),
                'scale': list(e.scale or (1, 1, 1)), 'rot': rot[:4],
                'params': param_list(e.obj),
                'geoms': [jsonable(g) for g in e.geoms],
                'anims': [jsonable(a) for a in e.anims],
                'xref': jsonable(I._val(e.obj.get('xref'))) if e.obj.get('xref') is not None else None,
                'fileRef': file_ref,
                'box': {'min': box[0], 'max': box[1], 'src': box[2]} if box else None,
            }
            return key

        root_key = walk(root, str(si), None, 0)
        lookup = [I._val(x) for x in entry.get('nodeidlookup').items] if isinstance(entry.get('nodeidlookup'), Arr) else []
        comp = sc.name
        scenes.append({
            'index': si, 'root': root_key,
            'componentId': str(comp), 'componentIdHex': f'{comp:016x}' if isinstance(comp, int) else None,
            'componentIdMatchesPath': (I.component_id(eaid) == comp) if eaid else None,
            'lookupCount': len(lookup), 'lookupOk': lookup == expected_lookup(entry),
        })
    return {'scenes': scenes, 'elements': elements, 'xrefs': xref_summ,
            'stats': {'elements': len(elements), 'idBad': id_bad, 'xrefs': len(xref_summ)}}


def verify_bytes(data, source=None, component_ids=None):
    checks = {}
    try:
        doc = I.Idf(data)
    except Exception as ex:
        return {'ok': False, 'reparse': False, 'error': str(ex)}
    checks['reparse'] = True
    checks['roundtrip'] = doc.to_bytes() == data
    total = bad = 0
    bad_paths = []
    lookup_ok = True
    for sc in doc.scenes:
        if not sc.root:
            continue
        for e, _d in sc.root.walk():
            total += 1
            if not e.id_ok():
                bad += 1
                if len(bad_paths) < 10:
                    bad_paths.append(e.path)
        if sc.nodeidlookup != expected_lookup(sc.entry):
            lookup_ok = False
    checks['elements'] = total
    checks['elementIdsOk'] = bad == 0
    checks['elementIdsBad'] = bad_paths
    checks['nodeidlookupOk'] = lookup_ok
    if component_ids is not None:
        checks['componentIdsKept'] = [sc.name for sc in doc.scenes] == component_ids
    if source is not None:
        checks['identicalToSource'] = data == source
    checks['ok'] = (checks['roundtrip'] and checks['elementIdsOk'] and lookup_ok
                    and checks.get('componentIdsKept', True))
    return checks
