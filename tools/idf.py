"""Reader/decoder for F1 25 .idf scene-layout resources (UILayout, EGO "eScene").

An .idf is a !pkg container (see pkg.py) whose root holds one or more scenes:

    { scenelist: [ { scene: <element>, name: <ui64 componentId>,
                     nodeidlookup: [<ui32>, ...] }, ... ] }

Each <element> is an object with (observed) keys:

    name        stri   slot name a .ui attaches to; scene root is "Scene Root"
    elementId   ui32   == djb2(dotted path, rooted below "Scene Root")
    typeName    shnm   authoring class: eScene eNull eText eImage eStacker ePut ...
    position    vec3   local translation (x, y, z[, pad])
    scale       vec3   local scale
    rotation    quat   local rotation
    params      [ {name, value}, ... ]   named authoring params incl.
                       minExtents/maxExtents (vec3 local bbox = the rect),
                       className, guid, pivot*, eLayer.layer, boxWidth, style,
                       halign, stackerType, gap, *Margin, scaleX/Y/Z, ...
    geoms       [ {resourceid, materialid, extentsmin(vec4), extentsmax(vec4),
                   asset(ui64)} ]   drawn art for this element
    anims       [ {asset} ]         animation-clip refs
    children    [ <element>, ... ]

The component a `.ui` screen attaches to is addressed by componentId =
md5-64 big-endian of the eaid path WITHOUT the ".idf" suffix; the scene's
`name` field carries that same value. A widget attaches to an element by
`nodeId == djb2(element leaf name)`, and `nodeidlookup` is the sorted,
de-duplicated list of djb2(leaf name) for every element, "Scene Root"
included. It is not the list of elementIds.

The container is parsed and re-serialised byte-identically by pkg.py, so this
module is read + write capable: mutate the Pkg tree (helpers below) and call
pkg.Pkg.to_bytes().
"""
import sys, os, json, hashlib, struct

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pkg as P
from pkg import Obj, Arr, Val, Iar


def djb2(s):
    h = 5381
    for c in s.encode():
        h = ((h * 33) + c) & 0xFFFFFFFF
    return h


def component_id(path):
    """md5-64 big-endian of the eaid path (no .idf suffix) == scene `name`."""
    if path.endswith('.idf'):
        path = path[:-4]
    return int.from_bytes(hashlib.md5(path.encode()).digest()[:8], 'big')


def _val(v):
    return v.value if isinstance(v, Val) else v


def _vec3(v):
    """Return (x, y, z) from a vec3/vec4 Val (stored as 4 floats)."""
    if isinstance(v, Val) and isinstance(v.value, list):
        return tuple(round(x, 6) for x in v.value[:3])
    return None


class Element:
    def __init__(self, obj):
        self.obj = obj  # the live pkg.Obj (edit here for write-back)
        self.name = _val(obj.get('name'))
        self.element_id = _val(obj.get('elementId'))
        self.type = _val(obj.get('typeName'))
        self.position = _vec3(obj.get('position'))
        self.scale = _vec3(obj.get('scale'))
        self.rotation = _val(obj.get('rotation'))
        self.params = {}
        pa = obj.get('params')
        if isinstance(pa, Arr):
            for it in pa.items:
                if isinstance(it, Obj):
                    nm = _val(it.get('name'))
                    self.params[nm] = _val(it.get('value'))
        self.geoms = []
        ga = obj.get('geoms')
        if isinstance(ga, Arr):
            for it in ga.items:
                if isinstance(it, Obj):
                    self.geoms.append({k.value: _val(v) for k, v in it.items})
        self.anims = []
        aa = obj.get('anims')
        if isinstance(aa, Arr):
            for it in aa.items:
                if isinstance(it, Obj):
                    self.anims.append({k.value: _val(v) for k, v in it.items})
        self.path = self.name
        self.children = []
        ca = obj.get('children')
        if isinstance(ca, Arr):
            for it in ca.items:
                if isinstance(it, Obj):
                    self.children.append(Element(it))

    def assign_paths(self, prefix=None):
        """elementId == djb2(dotted path). The path is rooted at the top-level
        child's own name (the "Scene Root" element is excluded from child paths)."""
        if prefix is None:                 # this is the scene root
            self.path = self.name
            child_prefix = ''
        else:
            self.path = f'{prefix}.{self.name}' if prefix else self.name
            child_prefix = self.path
        for c in self.children:
            c.assign_paths(child_prefix)

    @property
    def class_name(self):
        return self.params.get('className')

    @property
    def min_extents(self):
        v = self.params.get('minExtents')
        return tuple(round(x, 6) for x in v[:3]) if isinstance(v, list) else None

    @property
    def max_extents(self):
        v = self.params.get('maxExtents')
        return tuple(round(x, 6) for x in v[:3]) if isinstance(v, list) else None

    def rect(self):
        """Local-space (w, h) from extents if present, else from geom extents."""
        mn, mx = self.min_extents, self.max_extents
        if mn and mx:
            return (round(mx[0] - mn[0], 6), round(mx[2] - mn[2], 6),
                    round(mx[1] - mn[1], 6))  # width(x), depth(z), height(y)
        if self.geoms:
            g = self.geoms[0]
            a, b = g.get('extentsmin'), g.get('extentsmax')
            if isinstance(a, list) and isinstance(b, list):
                return (round(b[0] - a[0], 6), round(b[2] - a[2], 6), round(b[1] - a[1], 6))
        return None

    def id_ok(self):
        key = self.path if self.path is not None else self.name
        return key is not None and djb2(key) == self.element_id

    def walk(self, depth=0):
        yield self, depth
        for c in self.children:
            yield from c.walk(depth + 1)

    def find(self, name):
        for e, _ in self.walk():
            if e.name == name:
                return e
        return None


class Scene:
    def __init__(self, entry_obj):
        self.entry = entry_obj
        self.name = _val(entry_obj.get('name'))  # ui64 componentId
        self.root = Element(entry_obj['scene']) if entry_obj.get('scene') else None
        if self.root:
            self.root.assign_paths()
        nl = entry_obj.get('nodeidlookup')
        self.nodeidlookup = [_val(x) for x in nl.items] if isinstance(nl, Arr) else []


class Idf:
    def __init__(self, data):
        self.pkg = P.Pkg(data)
        self.scenes = []
        sl = self.pkg.root.get('scenelist')
        if isinstance(sl, Arr):
            for entry in sl.items:
                if isinstance(entry, Obj):
                    self.scenes.append(Scene(entry))

    @classmethod
    def load(cls, path):
        return cls(open(path, 'rb').read())

    def to_bytes(self):
        return self.pkg.to_bytes()


# ---- formatting -------------------------------------------------------------

def _fmt_vec(v):
    return '(' + ', '.join(f'{x:g}' for x in v) + ')' if v else '-'


def tree_lines(idf, show_params=False, show_geoms=False):
    out = []
    for si, sc in enumerate(idf.scenes):
        out.append(f'scene[{si}] componentId={sc.name} (0x{sc.name:016x})  '
                   f'nodeidlookup={len(sc.nodeidlookup)}')
        if not sc.root:
            continue
        for e, d in sc.root.walk():
            pad = '  ' * (d + 1)
            idtag = '' if e.id_ok() else f' !ID(djb2({e.path!r})={djb2(e.path) if e.path else "?"})'
            rect = e.rect()
            bits = [f'{pad}{e.name!r} [{e.type}]']
            if e.position and any(e.position):
                bits.append(f'pos={_fmt_vec(e.position)}')
            if rect:
                bits.append(f'size(w,d,h)={_fmt_vec(rect)}')
            if e.scale and e.scale != (1.0, 1.0, 1.0):
                bits.append(f'scale={_fmt_vec(e.scale)}')
            if e.geoms:
                bits.append(f'geoms={len(e.geoms)}')
            if e.anims:
                bits.append(f'anims={len(e.anims)}')
            bits.append(f'id={e.element_id}{idtag}')
            out.append('  '.join(bits))
            if show_params and e.params:
                for k, v in e.params.items():
                    if k in ('guid',):
                        continue
                    out.append(f'{pad}    .{k} = {v}')
            if show_geoms and e.geoms:
                for g in e.geoms:
                    out.append(f'{pad}    geom res={g.get("resourceid")}')
                    out.append(f'{pad}         mat={g.get("materialid")}')
    return out


def slots_lines(idf):
    out = []
    for si, sc in enumerate(idf.scenes):
        if not sc.root:
            continue
        for e, d in sc.root.walk():
            rect = e.rect()
            out.append('\t'.join([
                str(e.name), str(e.element_id), str(e.type),
                _fmt_vec(e.position) if e.position else '-',
                _fmt_vec(rect) if rect else '-',
            ]))
    return out


def clone_node(node):
    """Deep-copy a live pkg node (Obj/Arr/Val/Iar) so edits don't alias."""
    if isinstance(node, Obj):
        return Obj([(clone_node(k), clone_node(v)) for k, v in node.items], node.version)
    if isinstance(node, Arr):
        return Arr([clone_node(v) for v in node.items], node.tag)
    if isinstance(node, Val):
        return Val(node.type, node.value, None)  # drop ref: re-intern on write
    if isinstance(node, Iar):
        return Iar(node.raw)
    return node


def set_param(elem_obj, name, value, vtype='stri'):
    pa = elem_obj.get('params')
    if not isinstance(pa, Arr):
        pa = Arr([]); elem_obj.set('params', pa)
    for it in pa.items:
        if isinstance(it, Obj) and _val(it.get('name')) == name:
            it.set('value', Val(vtype, value)); return
    pa.items.append(Obj([(Val('stri', 'value'), Val(vtype, value)),
                         (Val('stri', 'name'), Val('stri', name))]))


def recompute_ids(elem_obj, path, out_ids):
    """Set elementId = djb2(path) for this element and recurse into children.
    `path` is the dotted path rooted at the top-level child's name."""
    elem_obj.set('elementId', Val('ui32', djb2(path)))
    out_ids.append(djb2(path))
    ca = elem_obj.get('children')
    if isinstance(ca, Arr):
        for child in ca.items:
            if isinstance(child, Obj):
                cn = _val(child.get('name'))
                recompute_ids(child, f'{path}.{cn}', out_ids)


def relookup(scene_entry):
    """Rebuild nodeidlookup as stock scenes hold it: sorted unique djb2 of
    every element's leaf name, "Scene Root" included."""
    root = scene_entry.get('scene')
    names = set()

    def walk(obj):
        names.add(djb2(_val(obj.get('name'))))
        ca = obj.get('children')
        if isinstance(ca, Arr):
            for c in ca.items:
                if isinstance(c, Obj):
                    walk(c)

    walk(root)
    old = scene_entry.get('nodeidlookup')
    tag = old.tag if isinstance(old, Arr) else '!ili'
    scene_entry.set('nodeidlookup', Arr([Val('ui32', h) for h in sorted(names)], tag))


def clone_slot(idf, src_name, new_name, pos, out_path, guid=None):
    """Clone layout slot `src_name` to a new sibling `new_name` at `pos`
    (write-back). Fixes elementId (djb2 of dotted path) for the clone and all
    its descendants, sets a fresh guid, appends to the parent's children and to
    the scene nodeidlookup. Demonstrates editor-grade write capability."""
    import uuid
    for sc in idf.scenes:
        if not sc.root:
            continue
        # locate src element + its parent Obj + parent path
        found = {}

        def search(obj, path_prefix):
            # obj is an element; path_prefix is its dotted path ('' for Scene Root,
            # which is itself excluded from child paths).
            ca = obj.get('children')
            if not isinstance(ca, Arr):
                return
            for child in ca.items:
                if isinstance(child, Obj):
                    nm = _val(child.get('name'))
                    cpath = nm if path_prefix == '' else f'{path_prefix}.{nm}'
                    if nm == src_name and 'obj' not in found:
                        found.update(obj=child, parent=obj, parent_path=path_prefix)
                    search(child, cpath)

        search(sc.root.obj, '')
        if 'obj' not in found:
            continue
        parent = found['parent']
        parent_path = found['parent_path']
        clone = clone_node(found['obj'])
        clone.set('name', Val('stri', new_name))
        clone.set('position', Val('vec3', [pos[0], pos[1], pos[2], 0.0]))
        set_param(clone, 'guid', guid or str(uuid.uuid4()))
        new_path = new_name if parent_path == '' else f'{parent_path}.{new_name}'
        new_ids = []
        recompute_ids(clone, new_path, new_ids)
        parent.get('children').items.append(clone)
        before = len(sc.entry.get('nodeidlookup').items)
        relookup(sc.entry)
        after = len(sc.entry.get('nodeidlookup').items)
        open(out_path, 'wb').write(idf.to_bytes())
        print(f'cloned {src_name!r} -> {new_name!r} at {pos}')
        print(f'  new path = {new_path}  elementId = {djb2(new_path)}  nodeId = {djb2(new_name)}')
        print(f'  nodeidlookup {before} -> {after} name hash(es); wrote {out_path}')
        return 0
    print(f'slot {src_name!r} not found')
    return 1


def main(argv):
    if len(argv) < 2:
        print(__doc__.splitlines()[0])
        print('usage: idf.py {tree|params|geoms|slots|json|find|scene-id|roundtrip|'
              'clone-slot|set-pos} <file.idf> [args]')
        return 2
    cmd = argv[1]
    if cmd == 'roundtrip':
        bad = 0
        for f in argv[2:]:
            d = open(f, 'rb').read()
            if d[:4] != b'!pkg':
                print('SKIP (not !pkg)', f); continue
            try:
                r = Idf(d).to_bytes()
            except Exception as e:
                print('ERR', f, e); bad += 1; continue
            if r != d:
                i = next((i for i in range(min(len(r), len(d))) if r[i] != d[i]),
                         min(len(r), len(d)))
                print('DIFF', f, 'len', len(d), '!=', len(r), 'first@', hex(i)); bad += 1
            else:
                print('OK  ', f, len(d), 'bytes')
        print('bad', bad)
        return 1 if bad else 0

    idf = Idf.load(argv[2])
    if cmd == 'tree':
        print('\n'.join(tree_lines(idf)))
    elif cmd == 'params':
        print('\n'.join(tree_lines(idf, show_params=True)))
    elif cmd == 'geoms':
        print('\n'.join(tree_lines(idf, show_geoms=True)))
    elif cmd == 'slots':
        print('name\telementId\ttype\tposition\tsize(w,d,h)')
        print('\n'.join(slots_lines(idf)))
    elif cmd == 'json':
        print(json.dumps(P.to_py(idf.pkg.root), indent=1, ensure_ascii=False))
    elif cmd == 'scene-id':
        for sc in idf.scenes:
            print(f'componentId={sc.name}  0x{sc.name:016x}')
            if len(argv) > 3:
                exp = component_id(argv[3])
                print(f'  md5-64(path)={exp}  match={exp == sc.name}')
    elif cmd == 'find':
        name = argv[3]
        for sc in idf.scenes:
            e = sc.root.find(name) if sc.root else None
            if not e:
                continue
            print(f'element {e.name!r} [{e.type}] id={e.element_id} (djb2 ok={e.id_ok()})')
            print(f'  position   = {_fmt_vec(e.position)}')
            print(f'  scale      = {_fmt_vec(e.scale)}')
            print(f'  rotation   = {e.rotation}')
            print(f'  minExtents = {_fmt_vec(e.min_extents)}')
            print(f'  maxExtents = {_fmt_vec(e.max_extents)}')
            print(f'  size(w,d,h)= {_fmt_vec(e.rect())}')
            print(f'  children   = {[c.name for c in e.children]}')
            for k, v in e.params.items():
                print(f'  param .{k} = {v}')
            for g in e.geoms:
                print(f'  geom res={g.get("resourceid")} mat={g.get("materialid")}')
    elif cmd == 'clone-slot':
        # clone-slot <file> <src_name> <new_name> <x> <y> <z> <out>
        src, new, x, y, z, out = argv[3], argv[4], float(argv[5]), float(argv[6]), float(argv[7]), argv[8]
        return clone_slot(idf, src, new, (x, y, z), out)
    elif cmd == 'set-pos':
        # set-pos <file> <element> <x> <y> <z> <out>   (write-back demo)
        name, x, y, z, out = argv[3], float(argv[4]), float(argv[5]), float(argv[6]), argv[7]
        for sc in idf.scenes:
            e = sc.root.find(name) if sc.root else None
            if e:
                e.obj.set('position', Val('vec3', [x, y, z, 0.0]))
                open(out, 'wb').write(idf.to_bytes())
                print(f'set {name} position -> ({x},{y},{z}); wrote {out}')
                return 0
        print('element not found')
        return 1
    else:
        print('unknown command', cmd)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
