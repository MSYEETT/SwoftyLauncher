import sys, os, copy, struct, hashlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pkg import Pkg, Obj, Arr, Val
from uinode import (PROPS, CHILDREN, COMMANDS, EVENTS, LAYOUT, COMPONENTS, BEHAVIOURS, djb2, is_node, name, child,
                    children, fix_ids, rename_refs, set_prop_value, drop_key)

SCREEN_BEHAVIOURS = ('NBEL', 'NBDC', 'NBMG', 'NBVM', 'NBAC', 'NBOS', 'NBXA', 'NBMO', 'NBTR')
GROUP_BEHAVIOURS = ('NBEL', 'NBDC', 'NBMG', 'NBVM', 'NBFC', 'NBAN', 'NBAC', 'NBOS', 'NBXA', 'NBMO')
LABEL_BEHAVIOURS = ('NBEL', 'NBMG', 'NBOS', 'NBXA', 'NBMO')
DECISIONS = ('SVCD', 'MVCD', 'DBVV', 'DBRV', 'DBCD', 'DBAM', 'WUDN', 'UIVF')


def S(v):
    return Val('stri', v)


def obj(*pairs):
    return Obj([(S(k), v) for k, v in pairs])


def fourcc(s):
    return struct.unpack('<I', s.encode())[0]


def component_id(path):
    return int.from_bytes(hashlib.md5(path.encode()).digest()[:8], 'big')


def prop(t, v, db=None):
    items = [('value', Val(t, v))]
    if db is not None:
        items.append(('dbentry', S(db)))
    return obj(*items)


def attr(v):
    if isinstance(v, dict):
        return obj(*[(k, attr(x)) for k, x in v.items()])
    if isinstance(v, tuple) and v[0] == 'db':
        return obj(('value', S('')), ('dbentry', S(v[1])))
    if isinstance(v, bool):
        return obj(('value', Val('bool', v)))
    if isinstance(v, float):
        return obj(('value', Val('fp32', v)))
    if isinstance(v, int):
        return obj(('value', Val('ui32', v)))
    return obj(('value', S(v)))


def sig(to, pin, frm, attrs=None):
    items = [('to_object', Val('ui32', djb2(to))), ('to_object_name', S(to)), ('to_pin_name', S(pin)),
             ('to_pin', Val('ui32', djb2(pin))), ('from_pin_name', S(frm))]
    if attrs:
        items.append(('attributes', obj(*[(k, attr(v)) for k, v in attrs.items()])))
    return obj(*items)


def event(name_, signals):
    return obj(('eventName', S(name_)), ('signals', Arr(list(signals))), ('event', Val('ui32', djb2(name_))))


def command(name_, signals):
    return obj(('commandName', S(name_)), ('command', Val('ui32', djb2(name_))), ('signals', Arr(list(signals))))


def behaviours(codes):
    return Arr([obj(('type', Val('ui32', fourcc(c)))) for c in codes])


def layout_path(*names):
    return Arr([Arr([obj(('nodeId', Val('ui32', djb2(n))), ('debugName', S(n))) for n in names])])


def node(name_, type_, props=(), kids=(), events=(), layout=None, behav=()):
    p = [('id', Val('ui32', 0)), ('version', Val('ui32', 0)), ('type', Val('ui32', fourcc(type_))),
         ('nameId', Val('ui32', djb2(name_))), ('name', S(name_))] + list(props)
    return Arr([obj(*p), Arr(list(kids)), Arr([]), Arr(list(events)),
                layout_path(*layout) if layout else Arr([]), Arr([]), behaviours(behav), Arr([])])


def add_events(n, *evs):
    for ev in evs:
        same = [e for e in n.items[EVENTS].items if e['eventName'].value == ev['eventName'].value]
        if same:
            same[0]['signals'].items += ev['signals'].items
        else:
            n.items[EVENTS].items.append(ev)


def raise_cmd(screen, frm, cmd, data=None):
    c = {'name': cmd}
    if data is not None:
        c['data'] = data
    return sig(screen, 'raise_command', frm, {'command': c})


def literal_label(n, text):
    t = n.items[PROPS].get('text')
    if t is None:
        n.items[PROPS].set('text', prop('stri', text))
    else:
        t['value'].value = text
        drop_key(t, 'dbentry')
        drop_key(t, 'isloctag')
    set_prop_value(n, 'localise', False, 'bool')


def label(name_, text, *layout):
    return node(name_, 'UILB', [('localise', prop('bool', False)), ('text', prop('stri', text))],
                layout=layout, behav=LABEL_BEHAVIOURS)


def hidden(name_, *layout):
    return node(name_, 'UIGP', [('visible', prop('bool', False))], layout=layout, behav=GROUP_BEHAVIOURS)


def strip_data(n):
    p = n.items[PROPS]
    p.items = [(k, v) for k, v in p.items if k.value != 'allowVMBinding']
    for k, v in p.items:
        if isinstance(v, Obj) and v.get('dbentry') is not None and not v['dbentry'].value.startswith('#'):
            drop_key(v, 'dbentry')
            drop_key(v, 'isloctag')
    n.items[COMMANDS].items = []
    n.items[CHILDREN].items = [c for c in children(n) if not (is_node(c) and _type(c) in DECISIONS)]
    for c in children(n):
        if is_node(c):
            strip_data(c)


def _type(n):
    return struct.pack('<I', n.items[PROPS]['type'].value).decode()


def drop_events_to(n, prefix_pins=('command_vmContextValid',)):
    for e in n.items[EVENTS].items:
        e['signals'].items = [s for s in e['signals'].items if s['to_pin_name'].value not in prefix_pins]
    n.items[EVENTS].items = [e for e in n.items[EVENTS].items if e['signals'].items]
    for c in children(n):
        if is_node(c):
            drop_events_to(c, prefix_pins)


def instance(template, path, new_name, texts=None, keep_events=True):
    t = copy.deepcopy(template)
    rename_refs(t, name(template), path + '.' + new_name)
    t.items[PROPS]['name'].value = new_name
    strip_data(t)
    drop_events_to(t)
    if not keep_events:
        t.items[EVENTS].items = []
    for sub, text in (texts or {}).items():
        n = t
        for part in sub.split('.'):
            n = child(n, part)
        literal_label(n, text)
    return t


def find(n, path):
    for part in path.split('.'):
        n = child(n, part)
    return n


def screen(name_, layout, props_from=None, drop_props=('allowVMBinding', 'sceneAttributes')):
    if props_from is not None:
        p = copy.deepcopy(props_from)
        p.items = [(k, v) for k, v in p.items if k.value not in drop_props]
        p['name'].value = name_
    else:
        p = obj(('id', Val('ui32', 0)), ('version', Val('ui32', 0)), ('type', Val('ui32', fourcc('UISC'))),
                ('nameId', Val('ui32', 0)), ('name', S(name_)))
    lay = Arr([Arr([obj(('componentId', Val('ui64', component_id(layout))), ('isComponentId', Val('bool', True)),
                        ('debugName', S(layout + '.idf')))])])
    return Arr([p, Arr([]), Arr([
        command('open', [sig(f'{name_}.Action', 'route', 'open', {'key': 'open'})]),
        command('close', [sig(f'{name_}.Action', 'route', 'close', {'key': 'close'})]),
    ]), Arr([]), lay, Arr([]), behaviours(SCREEN_BEHAVIOURS), Arr([])])


def frame(scr, action_src, audio_src, back_src, tooltips_src, tooltip_names, back_key='back', watchdog_s=10.0,
          tooltips_prefix='fe_career.tooltips'):
    sname = name(scr)
    action = copy.deepcopy(action_src)
    action.items[EVENTS].items = []
    audio = copy.deepcopy(audio_src)
    tips = copy.deepcopy(tooltips_src)
    old = tips.items[PROPS]['name'].value
    tips.items[CHILDREN].items = [c for c in children(tips) if is_node(c) and name(c) in tooltip_names]
    back = copy.deepcopy(back_src)
    back.items[PROPS]['name'].value = 'back_listener'
    back.items[EVENTS].items = [event('pressed', [sig(f'{sname}.audio_theme_trigger', 'back', 'pressed'),
                                                  sig(f'{sname}.Action', 'route', 'pressed', {'key': back_key})])]
    wd = node('watchdog', 'TIME', events=[event('time_out_expired',
                                                [sig(f'{sname}.Action', 'route', 'time_out_expired', {'key': back_key})])])
    return dict(action=action, audio=audio, tooltips=tips, tooltips_old=tooltips_prefix, back=back, watchdog=wd, seconds=watchdog_s)


def finish(scr, fr, kids, first_focus):
    sname = name(scr)
    reset = sig(f'{sname}.watchdog', 'reset', 'opened_screen', {'duration': 0})
    reset['attributes']['duration'].set('value', Val('fp32', fr['seconds']))
    fr['action'].items[EVENTS].items = [
        event('opened_screen', [reset, sig(f'{sname}.{first_focus}', 'focus', 'opened_screen')]),
        event('finishedTranIn', [sig(f'{sname}.watchdog', 'stop', 'finishedTranIn'),
                                 sig(f'{sname}.tooltips', 'ttActivate', 'finishedTranIn'),
                                 sig(f'{sname}.{first_focus}', 'focus', 'finishedTranIn')]),
        event('finishedTranOut', [sig(f'{sname}.watchdog', 'stop', 'finishedTranOut'),
                                  sig(f'{sname}.tooltips', 'ttDeactive', 'finishedTranOut')]),
    ]
    tips = fr['tooltips']
    tips.items[PROPS]['name'].value = 'tooltips'
    rename_refs(tips, fr['tooltips_old'], f'{sname}.tooltips')
    first = None
    for k in kids:
        if is_node(k):
            try:
                first = find(k, first_focus[len(name(k)) + 1:]) if first_focus.startswith(name(k) + '.') else (k if name(k) == first_focus else None)
            except KeyError:
                first = None
            if first is not None:
                break
    if first is not None:
        add_events(first, event('focused', [sig(f'{sname}.watchdog', 'stop', 'focused')]))
    scr.items[CHILDREN].items = [fr['action'], tips, fr['audio'], fr['back']] + kids + [fr['watchdog']]
    fix_ids(scr)
    return scr


def chain_focus(screen_name, nodes, horizontal=False):
    for a, b in zip(nodes, nodes[1:]):
        add_events(a, event('navDown', [sig(f'{screen_name}.{name(b)}', 'focus', 'navDown')]))
        add_events(b, event('navUp', [sig(f'{screen_name}.{name(a)}', 'focus', 'navUp')]))
        if horizontal:
            add_events(a, event('navRight', [sig(f'{screen_name}.{name(b)}', 'focus', 'navRight')]))
            add_events(b, event('navLeft', [sig(f'{screen_name}.{name(a)}', 'focus', 'navLeft')]))


def ui_bytes(*screens):
    p = Pkg()
    p.root = Obj([(S('Screens'), Arr(list(screens)))])
    return p.to_bytes()


def route_cmd(type_, cid, attrs, deps=None):
    a = [obj(('Id', S('Id')), ('Value', S(str(cid))))] + [obj(('Id', S(k)), ('Value', S(v))) for k, v in attrs]
    items = [('Attributes', Arr(a)), ('Type', S(type_))]
    if deps:
        items.append(('Dependencies', S(deps)))
    return obj(*items)


def route(id_, cmds, deps, rtype, destination=None):
    items = [('Id', S(id_)), ('Commands', Arr(cmds))]
    if destination:
        items.append(('Attributes', Arr([obj(('Id', S('Destination')), ('Value', S(destination)))])))
    items += [('Type', Val('si32', rtype)), ('Dependencies', S(deps))]
    return obj(*items)


def screen_routes(name_):
    return [
        route('open', [route_cmd('OpenScreen', 1, [('Target', name_)]), route_cmd('InputState', 2, [], '1'),
                       route_cmd('TransitionScreen', 3, [('Direction', 'in'), ('Target', name_)], '1')], '2,3', 0),
        route('open_no_input', [route_cmd('OpenScreen', 1, [('Target', name_)]),
                                route_cmd('TransitionScreen', 2, [('Direction', 'in'), ('Target', name_)], '1')], '2', 0),
        route('close', [route_cmd('TransitionScreen', 1, [('Direction', 'out'), ('Target', name_)]),
                        route_cmd('CloseScreen', 2, [('Target', name_)], '1')], '2', 0),
    ]


def subpage_route(id_, src, dst, rtype=1, sub='open'):
    return route(id_, [route_cmd('TriggerSubRoute', 1, [('SubRoute', 'close'), ('Target', src), ('To', dst)]),
                       route_cmd('TriggerSubRoute', 2, [('SubRoute', sub), ('Target', dst), ('From', src), ('Message', '')], '1')],
                 '2', rtype, dst)


def migrate_route(id_, src, flow, dst, sub='open_no_input', rtype=2):
    return route(id_, [route_cmd('TriggerSubRoute', 1, [('SubRoute', 'close'), ('Target', src), ('To', dst)]),
                       route_cmd('Migrate', 2, [('Flow', flow), ('SkipActivator', 'true')], '1'),
                       route_cmd('TriggerSubRoute', 3, [('SubRoute', sub), ('Target', dst), ('From', src), ('Message', '')], '2')],
                 '3', rtype, dst)


def route_group(name_, routes):
    return obj(('Id', S(name_)), ('Routes', Arr(routes)), ('Attributes', Arr([obj(('Id', S('Source')), ('Value', S(name_)))])))


def add_route_groups(routes_bytes, groups, replace_routes=None):
    p = Pkg(routes_bytes)
    gs = p.root['RouteGroupings'].items
    for g in groups:
        assert not any(x['Id'].value == g['Id'].value for x in gs), g['Id'].value
        gs.append(g)
    for gid, rt in (replace_routes or []):
        grp = next(x for x in gs if x['Id'].value == gid)
        grp['Routes'].items = [r for r in grp['Routes'].items if r['Id'].value != rt['Id'].value] + [rt]
    return p.to_bytes()


def add_flow_entries(flows_bytes, flow_name, scene_index, entries):
    p = Pkg(flows_bytes)
    flow = next(f for f in p.root['Flows'].items if name(f) == flow_name)
    scene = flow.items[CHILDREN].items[scene_index]
    ents = scene.items[CHILDREN].items
    tmpl = ents[0]
    for entry_name, screen_name in entries:
        assert not any(name(e) == entry_name for e in ents)
        e = copy.deepcopy(tmpl)
        p0 = e.items[PROPS]
        p0['name'].value = entry_name
        p0['screen']['value'].value = screen_name
        p0.items = [(k, v) for k, v in p0.items if k.value != 'command']
        ents.append(e)
    fix_ids(flow)
    return p.to_bytes()


def add_regions(region_bytes, screens, like='fe_home'):
    p = Pkg(region_bytes)
    reg = next(r for r in p.root['regions']['Contents'].items if r['Name'].value == 'region_undefined')
    c = reg['Contents']
    for s in screens:
        assert c.get(s) is None
        e = copy.deepcopy(c[like])
        e['Name'].value = s
        e['Value'].value = f'asset_groups/ui_package/screens/{s}.assetgroup'
        c.set(s, e)
    return p.to_bytes()


def uidb_var(name_, vtype, value):
    t = {'bool': 'bool', 'uistring': 'stri', 'uint32': 'ui32', 'float': 'fp32'}[vtype]
    return obj(('ValueType', S(vtype)), ('Name', S(name_)), ('ReadOnly', Val('bool', False)),
               ('Value', Val(t, value)), ('DBType', S('Variable')))


def uidb_dict(name_, entries):
    return obj(('Name', S(name_)), ('Contents', Obj([(S(k), v) for k, v in entries])), ('DBType', S('Dictionary')))


def add_uidb_child(uidb_bytes, root, key, entries):
    p = Pkg(uidb_bytes)
    c = p.root[root]['Contents']
    assert c.get(key) is None
    c.set(key, uidb_dict(key, entries))
    return p.to_bytes()
