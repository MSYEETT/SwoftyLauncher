import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pkg import Pkg, Obj, Arr, Val, fourcc

SLOTS = ['props', 'children', 's2', 'events', 'layoutNodes', 's5', 'components', 's7']


def is_node(n):
    return isinstance(n, Arr) and len(n.items) == 8 and isinstance(n.items[0], Obj) and n.items[0].get('name') is not None


def props(o):
    out = []
    for k, v in o.items:
        key = k.value
        if key in ('id', 'nameId', 'version', 'name', 'type'):
            continue
        if isinstance(v, Obj) and isinstance(v.get('value'), Val):
            s = f'{key}={v["value"].value!r}'
            if v.get('dbentry') is not None:
                s += f'@{v["dbentry"].value}'
            out.append(s)
        elif isinstance(v, Val):
            out.append(f'{key}={v.value!r}')
        else:
            out.append(f'{key}={{..}}')
    return out


def walk(n, depth, out, full):
    if is_node(n):
        o = n.items[0]
        t = o.get('type')
        tt = fourcc(t.value) if t is not None else '?'
        line = '  ' * depth + f'{o["name"].value} [{tt}]'
        if full:
            line += '  ' + ' '.join(props(o))
        out.append(line)
        if full:
            ev = n.items[3]
            if isinstance(ev, Arr):
                for e in ev.items:
                    if isinstance(e, Obj):
                        sigs = e.get('signals')
                        for s in (sigs.items if isinstance(sigs, Arr) else []):
                            attrs = s.get('attributes')
                            a = ''
                            if attrs is not None:
                                a = ' ' + str(_flat(attrs))
                            out.append('  ' * depth + f'  ~ on {e["eventName"].value} -> {s["to_object_name"].value}.{s["to_pin_name"].value}{a}')
        for c in n.items[1].items if isinstance(n.items[1], Arr) else []:
            walk(c, depth + 1, out, full)
    elif isinstance(n, Arr):
        for c in n.items:
            walk(c, depth, out, full)
    elif isinstance(n, Obj):
        for k, v in n.items:
            walk(v, depth, out, full)


def _flat(o):
    if isinstance(o, Val):
        return o.value
    if isinstance(o, Obj):
        if len(o.items) == 1 and o.items[0][0].value == 'value':
            return _flat(o.items[0][1])
        return {k.value: _flat(v) for k, v in o.items}
    if isinstance(o, Arr):
        return [_flat(x) for x in o.items]
    return '?'


if __name__ == '__main__':
    full = '-f' in sys.argv
    for f in [a for a in sys.argv[1:] if a != '-f']:
        p = Pkg(open(f, 'rb').read())
        out = []
        walk(p.root, 0, out, full)
        print('\n'.join(out))
