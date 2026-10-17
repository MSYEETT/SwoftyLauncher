import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pkg import Pkg, Obj, Arr, Val
from uinode import PROPS, CHILDREN, COMMANDS, EVENTS, LAYOUT, djb2, is_node, name


def walk(n, parent=None):
    path = name(n) if parent is None else f'{parent}.{name(n)}'
    yield path, n
    for c in n.items[CHILDREN].items:
        if is_node(c):
            yield from walk(c, path)


def strings(x):
    if isinstance(x, Val):
        if x.type == 'stri' and isinstance(x.value, str):
            yield x.value
    elif isinstance(x, Obj):
        for k, v in x.items:
            yield from strings(v)
    elif isinstance(x, Arr):
        for v in x.items:
            yield from strings(v)


def check(data, screen=None, extern=(), banned=None):
    root = Pkg(data).root
    tops = [s for key in ('Screens', 'Templates') if root.get(key) is not None for s in root[key].items]
    nodes = [x for t in tops for x in walk(t)]
    paths = {p for p, _ in nodes}
    bad, outside, nsig = [], [], 0
    for p, n in nodes:
        if screen and p != screen and not p.startswith(screen + '.'):
            continue
        pr = n.items[PROPS]
        if banned:
            for i in (PROPS, COMMANDS, EVENTS):
                bad += [f'{p}: binds {x}' for x in strings(n.items[i]) if x.startswith(banned)]
        if pr.get('id') is not None and pr['id'].value != djb2(p):
            bad.append(f'{p}: id')
        if pr.get('nameId') is not None and pr['nameId'].value != djb2(name(n)):
            bad.append(f'{p}: nameId')
        for grp in n.items[LAYOUT].items if isinstance(n.items[LAYOUT], Arr) else []:
            for b in grp.items if isinstance(grp, Arr) else []:
                if isinstance(b, Obj) and b.get('debugName') is not None and b.get('nodeId') is not None and b['nodeId'].value != djb2(b['debugName'].value):
                    bad.append(f'{p}: layout {b["debugName"].value}')
        for slot, key in ((EVENTS, 'eventName'), (COMMANDS, 'commandName')):
            for e in n.items[slot].items if isinstance(n.items[slot], Arr) else []:
                if not isinstance(e, Obj) or e.get(key) is None:
                    continue
                h = e.get('event' if key == 'eventName' else 'command')
                if h is not None and h.value != djb2(e[key].value):
                    bad.append(f'{p}: {key} {e[key].value}')
                for s in e['signals'].items:
                    nsig += 1
                    to, pin = s['to_object_name'].value, s['to_pin_name'].value
                    if s['to_object'].value != djb2(to) or s['to_pin'].value != djb2(pin):
                        bad.append(f'{p}: hash {to}.{pin}')
                    if to not in paths and to.split('.')[0] in extern:
                        outside.append(f'{p}: on {e[key].value} -> {to}.{pin}')
                    elif to not in paths:
                        bad.append(f'{p}: on {e[key].value} -> missing {to}.{pin}')
                    elif screen and to != screen and not to.startswith(screen + '.'):
                        outside.append(f'{p}: on {e[key].value} -> {to}.{pin}')
    return len(nodes), nsig, bad, outside


if __name__ == '__main__':
    args, screen, extern, banned = sys.argv[1:], None, [], None
    while args and args[0] in ('-s', '-x', '-d'):
        if args[0] == '-s':
            screen = args[1]
        elif args[0] == '-x':
            extern.append(args[1])
        else:
            banned = args[1]
        args = args[2:]
    rc = 0
    for f in args:
        n, s, bad, outside = check(open(f, 'rb').read(), screen, extern, banned)
        print(f'{f}: {n} nodes, {s} signals checked, {len(bad)} problems, {len(outside)} signals leave {screen or "file"}')
        for b in bad + [f'outside: {o}' for o in sorted(set(outside))]:
            print('  ', b)
        rc |= bool(bad)
    sys.exit(rc)
