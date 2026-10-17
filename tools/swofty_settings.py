import sys, os, copy, struct, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pkg import Pkg, Obj, Arr, Val
from uinode import (PROPS, CHILDREN, COMMANDS, EVENTS, LAYOUT, COMPONENTS, BEHAVIOURS, child, children, name,
                    clone_node, set_prop_value, drop_key, fix_ids, rename_refs, djb2, is_node)

SCREEN = 'swofty_settings'
DB = '#swofty'
FE_HOME_GROUP = 'asset_groups/ui_package/screens/fe_home.assetgroup'

OPTIONS = [
    dict(key='show_launcher_panel', kind='toggle', value=True,
         label_on='SHOW LAUNCHER PANEL: ON', label_off='SHOW LAUNCHER PANEL: OFF',
         description='Show the "Launched with Swofty Launcher" panel on the home screen.'),
    dict(key='mod_menu_key', kind='info', value='F9',
         label='MOD MENU KEY: F9 / R3',
         description='Key that opens these settings from the home screen. Keyboard F9, controller R3.'),
]

MODS = [
    dict(id='swoftymod', name='SwoftyMod', version='1.0.0', enabled=True, locked=True,
         description='The SwoftyMod core. Always on while the launcher is in use.'),
    dict(id='grid_2026_24', name='F1 2026 24-car grid', version='0.1.0', enabled=True, locked=False,
         description='Adds a 12th team to the F1 2026 season for a 24-car grid.'),
    dict(id='sepang', name='Sepang International Circuit', version='0.1.0', enabled=True, locked=False,
         description='Adds Sepang in the spare bahrain24 track slot.'),
]
MOD_SLOTS = 6


def fourcc(s):
    return struct.unpack('<I', s.encode())[0]


def V(t, v):
    return Val(t, v)


def S(v):
    return Val('stri', v)


def obj(*pairs):
    return Obj([(S(k), v) for k, v in pairs])


def prop(t, v, db=None, **extra):
    items = [('value', V(t, v))]
    if db is not None:
        items.append(('dbentry', S(db)))
    for k, (et, ev) in extra.items():
        items.append((k, V(et, ev)))
    return obj(*items)


def attr_value(v):
    if isinstance(v, dict):
        return obj(*[(k, attr_value(x)) for k, x in v.items()])
    if isinstance(v, tuple) and v[0] == 'db':
        return obj(('value', S('')), ('dbentry', S(v[1])))
    if isinstance(v, bool):
        return obj(('value', V('bool', v)))
    if isinstance(v, int):
        return obj(('value', V('ui32', v)))
    return obj(('value', S(v)))


def sig(to, pin, frm, attrs=None):
    items = [('to_object', V('ui32', djb2(to))), ('to_object_name', S(to)), ('to_pin_name', S(pin)),
             ('to_pin', V('ui32', djb2(pin))), ('from_pin_name', S(frm))]
    if attrs:
        items.append(('attributes', obj(*[(k, attr_value(v)) for k, v in attrs.items()])))
    return obj(*items)


def event(name_, signals):
    return obj(('eventName', S(name_)), ('signals', Arr(signals)), ('event', V('ui32', djb2(name_))))


def command(name_, signals):
    return obj(('commandName', S(name_)), ('command', V('ui32', djb2(name_))), ('signals', Arr(signals)))


def behaviours(*codes):
    return Arr([obj(('type', V('ui32', fourcc(c)))) for c in codes])


def node(name_, type_, props=(), kids=(), commands=(), events=(), layout=None, comps=None, behav=()):
    p = [('id', V('ui32', 0)), ('version', V('ui32', 0)), ('type', V('ui32', fourcc(type_))),
         ('nameId', V('ui32', djb2(name_))), ('name', S(name_))] + list(props)
    lay = Arr([Arr([obj(('nodeId', V('ui32', djb2(layout))), ('debugName', S(layout)))])]) if layout else Arr([])
    return Arr([obj(*p), Arr(list(kids)), Arr(list(commands)), Arr(list(events)), lay,
                comps if comps is not None else Arr([]), behaviours(*behav), Arr([])])


def raise_cmd(path, frm, cmd, data=None):
    c = {'name': cmd}
    if data is not None:
        c['data'] = data
    return sig(path, 'raise_command', frm, {'command': c})


def var(name_, vtype, value, readonly=False):
    t = {'bool': 'bool', 'uistring': 'stri', 'uint32': 'ui32'}[vtype]
    return obj(('ValueType', S(vtype)), ('Name', S(name_)), ('ReadOnly', V('bool', readonly)),
               ('Value', V(t, value)), ('DBType', S('Variable')))


def dictionary(name_, entries):
    return obj(('Name', S(name_)), ('Contents', Obj([(S(k), v) for k, v in entries])), ('DBType', S('Dictionary')))


def array(name_, items):
    return obj(('Name', S(name_)), ('TextureArray', V('bool', False)), ('Contents', Arr(items)), ('DBType', S('Array')))


def mod_entry(i, m):
    if m is None:
        m = dict(id=f'slot_{i}', name='', version='', enabled=False, locked=True, description='')
        visible = False
    else:
        visible = True
    lock = ' (ALWAYS ON)' if m['locked'] and visible else ''
    on = f"{m['name']} {m['version']}: ON{lock}".upper() if visible else ''
    off = f"{m['name']} {m['version']}: OFF".upper() if visible else ''
    return dictionary(m['id'], [
        ('id', var('id', 'uistring', m['id'])),
        ('name', var('name', 'uistring', m['name'])),
        ('version', var('version', 'uistring', m['version'])),
        ('enabled', var('enabled', 'bool', m['enabled'])),
        ('locked', var('locked', 'bool', m['locked'])),
        ('visible', var('visible', 'bool', visible)),
        ('label_on', var('label_on', 'uistring', on)),
        ('label_off', var('label_off', 'uistring', off)),
        ('description', var('description', 'uistring', m['description'])),
    ])


def swofty_db():
    opts = []
    for o in OPTIONS:
        if o['kind'] == 'toggle':
            opts.append((o['key'], dictionary(o['key'], [
                ('value', var('value', 'bool', o['value'])),
                ('label_on', var('label_on', 'uistring', o['label_on'])),
                ('label_off', var('label_off', 'uistring', o['label_off'])),
                ('description', var('description', 'uistring', o['description'])),
            ])))
        else:
            opts.append((o['key'], dictionary(o['key'], [
                ('value', var('value', 'uistring', o['value'])),
                ('label', var('label', 'uistring', o['label'])),
                ('description', var('description', 'uistring', o['description'])),
            ])))
    mods = [mod_entry(i, MODS[i] if i < len(MODS) else None) for i in range(MOD_SLOTS)]
    return dictionary('swofty', [
        ('options', dictionary('options', opts)),
        ('mods', array('mods', mods)),
        ('state', dictionary('state', [
            ('last_set', var('last_set', 'uistring', '')),
            ('mod_count', var('mod_count', 'uint32', len(MODS))),
        ])),
    ])


def patch_ui_data(data):
    p = Pkg(data)
    assert p.root.get('swofty') is None
    p.root.set('swofty', swofty_db())
    return p.to_bytes()


def make_tile(template, row, labels_db=None, label=None):
    t = copy.deepcopy(template)
    tname = name(template)
    rename_refs(t, tname, f'{SCREEN}.{row}')
    t.items[PROPS]['name'].value = row
    t.items[CHILDREN].items = [c for c in children(t) if name(c) not in ('enabled', 'notification_type')]
    t.items[COMMANDS].items = []
    t.items[EVENTS].items = []

    def unbind(n):
        for k, v in n.items[PROPS].items:
            if isinstance(v, Obj) and v.get('dbentry') is not None and not v['dbentry'].value.startswith('#'):
                drop_key(v, 'dbentry')
        for c in children(n):
            if is_node(c):
                unbind(c)
    unbind(t)
    for lbl in ('button_label', 'button_label_highlighted', 'button_label_dummy'):
        c = child(t, lbl)
        tx = c.items[PROPS]['text']
        tx['value'].value = label or ''
        drop_key(tx, 'isloctag')
        drop_key(tx, 'dbentry')
        if labels_db:
            tx.items.append((S('dbentry'), S(labels_db)))
        set_prop_value(c, 'localise', False, 'bool')
    return t


def set_labels(row, frm, text):
    return [sig(f'{SCREEN}.{row}.{l}', 'setText', frm, {'text': text})
            for l in ('button_label', 'button_label_highlighted', 'button_label_dummy')]


def toggle_nodes(row, path, locked_path=None, cmd_data=None):
    st, fl, lk, son, soff = f'{row}_state', f'{row}_flip', f'{row}_lock', f'{row}_set_on', f'{row}_set_off'
    nodes = [
        node(st, 'SVCD', [('value', prop('bool', True)), ('dbEntry', prop('stri', f'{path}.value' if not path.endswith(']') else f'{path}.enabled'))],
             events=[event('yes', set_labels(row, 'yes', ('db', f'{path}.label_on'))),
                     event('no', set_labels(row, 'no', ('db', f'{path}.label_off')))]),
        node(fl, 'SVCD', [('value', prop('bool', True)), ('dbEntry', prop('stri', f'{path}.value' if not path.endswith(']') else f'{path}.enabled'))],
             events=[event('yes', [sig(f'{SCREEN}.{soff}', 'trigger', 'yes')]),
                     event('no', [sig(f'{SCREEN}.{son}', 'trigger', 'no')])]),
        node(son, 'UIDS', [('source', prop('bool', True)), ('target', prop('bool', False, f'{path}.value' if not path.endswith(']') else f'{path}.enabled'))]),
        node(soff, 'UIDS', [('source', prop('bool', False)), ('target', prop('bool', False, f'{path}.value' if not path.endswith(']') else f'{path}.enabled'))]),
    ]
    press = [sig(f'{SCREEN}.{fl}', 'query', 'pressed')]
    if locked_path:
        nodes.append(node(lk, 'SVCD', [('value', prop('bool', True)), ('dbEntry', prop('stri', locked_path))],
                          events=[event('yes', [sig(f'{SCREEN}.audio_theme_trigger', 'back', 'yes')]),
                                  event('no', [sig(f'{SCREEN}.{fl}', 'query', 'no'),
                                               sig(f'{SCREEN}.audio_theme_trigger', 'confirm', 'no'),
                                               raise_cmd(SCREEN, 'no', 'swofty_set', cmd_data)])]))
        press = [sig(f'{SCREEN}.{lk}', 'query', 'pressed')]
    else:
        press += [sig(f'{SCREEN}.audio_theme_trigger', 'confirm', 'pressed'),
                  raise_cmd(SCREEN, 'pressed', 'swofty_set', cmd_data)]
    return nodes, press, st


def tile_templates(templates):
    return (next(t for t in templates if name(t) == 'fe_tile_button_default'),
            next(t for t in templates if name(t) == 'fe_tile_button_secondary'))


def header_tile(head_t, row, text):
    t = make_tile(head_t, row, label=text)
    t.items[PROPS].set('enabled', prop('bool', False))
    t.items[PROPS].set('interactWhenDis', prop('bool', False))
    return t


def focus_desc(t, text):
    t.items[EVENTS].items.append(event('focused', [sig(f'{SCREEN}.description', 'setText', 'focused', {'text': text})]))


def screen_frame(fe_home):
    scr = Arr([copy.deepcopy(fe_home.items[0]), Arr([]), Arr([]), Arr([]), copy.deepcopy(fe_home.items[4]),
               Arr([]), Arr([b for b in copy.deepcopy(fe_home.items[6]).items if b['type'].value != fourcc('F1FS')]),
               Arr([])])
    p = scr.items[PROPS]
    p['name'].value = SCREEN
    p.set('visualNav', obj(('xOffset', prop('fp32', 0.0)), ('yOffset', prop('fp32', 0.0))))
    scr.items[COMMANDS].items = [
        command('open', [sig(f'{SCREEN}.Action', 'route', 'open', {'key': 'open'})]),
        command('close', [sig(f'{SCREEN}.Action', 'route', 'close', {'key': 'close'})]),
    ]
    kids = []

    action = copy.deepcopy(child(fe_home, 'Action'))
    kids.append(action)

    tips = clone_node(child(fe_home, 'tooltips'), 'fe_home', 'tooltips')
    sel = child(tips, 'select')
    back = clone_node(sel, 'fe_home.tooltips', 'back')
    set_prop_value(back, 'tooltipsName', 'back')
    tips.items[CHILDREN].items = [sel, back]
    rename_refs(tips, 'fe_home.tooltips', f'{SCREEN}.tooltips')
    kids.append(tips)

    title = node('title', 'UILB', [('localise', prop('bool', False)), ('text', prop('stri', 'ADDON SETTINGS'))],
                 layout='season_edition_text', behav=('NBEL', 'NBMG', 'NBOS', 'NBXA', 'NBMO'))
    kids.append(title)
    desc = node('description', 'UILB', [('localise', prop('bool', False)),
                                        ('text', prop('stri', 'SwoftyMod settings and the mods loaded this session.'))],
                layout='description_label', behav=('NBEL', 'NBMG', 'NBOS', 'NBXA', 'NBMO'))
    kids.append(desc)
    for hidden in ('details_offline', 'details_news', 'details_leagues', 'beta_side_panel'):
        kids.append(node(f'hide_{hidden}', 'UIGP', [('visible', prop('bool', False))], layout=hidden,
                         behav=('NBEL', 'NBDC', 'NBMG', 'NBVM', 'NBFC', 'NBAN', 'NBAC', 'NBOS', 'NBXA', 'NBMO')))

    audio = copy.deepcopy(child(fe_home, 'audio_theme_trigger'))
    kids.append(audio)
    back_l = copy.deepcopy(child(fe_home, 'back_listener'))
    back_l.items[EVENTS].items = [event('pressed', [sig(f'{SCREEN}.audio_theme_trigger', 'back', 'pressed'),
                                                    sig(f'{SCREEN}.Action', 'route', 'pressed', {'key': 'back'})])]
    kids.append(back_l)
    return scr, kids, action


def finish_screen(scr, action, kids, rows, helpers, states, first):
    action.items[EVENTS].items = [
        event('finishedTranIn', [sig(f'{SCREEN}.tooltips', 'ttActivate', 'finishedTranIn'),
                                 sig(f'{SCREEN}.{first}', 'focus', 'finishedTranIn')]),
        event('finishedTranOut', [sig(f'{SCREEN}.tooltips', 'ttDeactive', 'finishedTranOut')]
              + [sig(f'{SCREEN}.{s}', 'unregister', 'finishedTranOut') for s in states]),
        event('opened_screen', [x for s in states for x in (sig(f'{SCREEN}.{s}', 'register', 'opened_screen'),
                                                            sig(f'{SCREEN}.{s}', 'query', 'opened_screen'))]),
    ]
    scr.items[CHILDREN].items = kids + rows + helpers
    fix_ids(scr)
    return scr


def build_screen(fe_home, templates):
    tile_t, head_t = tile_templates(templates)
    scr, kids, action = screen_frame(fe_home)
    rows, helpers, states = [], [], []
    rows.append(header_tile(head_t, 'header_swoftymod', 'SWOFTYMOD'))
    for o in OPTIONS:
        row = f'option_{o["key"]}'
        path = f'{DB}.options.{o["key"]}'
        if o['kind'] == 'toggle':
            t = make_tile(tile_t, row, label=o['label_on'])
            nodes, press, st = toggle_nodes(row, path, cmd_data=f'options.{o["key"]}')
            helpers += nodes
            states.append(st)
        else:
            t = make_tile(tile_t, row, labels_db=f'{path}.label', label=o['label'])
            press = [sig(f'{SCREEN}.audio_theme_trigger', 'confirm', 'pressed'),
                     raise_cmd(SCREEN, 'pressed', 'swofty_set', f'options.{o["key"]}')]
        t.items[EVENTS].items.append(event('pressed', press))
        focus_desc(t, ('db', f'{path}.description'))
        rows.append(t)
    rows.append(header_tile(head_t, 'header_installed', 'INSTALLED MODS'))
    for i in range(MOD_SLOTS):
        row = f'mod_{i}'
        path = f'{DB}.mods[{i}]'
        m = MODS[i] if i < len(MODS) else None
        initial = f"{m['name']} {m['version']}: {'ON' if m['enabled'] else 'OFF'}{' (ALWAYS ON)' if m['locked'] else ''}".upper() if m else ''
        t = make_tile(tile_t, row, label=initial)
        t.items[PROPS].set('visible', prop('bool', i < len(MODS), f'{path}.visible'))
        nodes, press, st = toggle_nodes(row, path, locked_path=f'{path}.locked', cmd_data=f'mods[{i}].enabled')
        helpers += nodes
        states.append(st)
        t.items[EVENTS].items.append(event('pressed', press))
        focus_desc(t, ('db', f'{path}.description'))
        rows.append(t)
    return finish_screen(scr, action, kids, rows, helpers, states, name(rows[1]))


def patch_fe_home_screen(data, syst):
    p = Pkg(data)
    screens = p.root['Screens']
    fe_home = screens.items[0]
    assert name(fe_home) == 'fe_home' and not any(name(s) == SCREEN for s in screens.items)
    templates = Pkg(syst).root['Templates'].items
    screens.items.append(build_screen(fe_home, templates))
    return p.to_bytes()


def _rename_values(n, old, new):
    def walk(x):
        if isinstance(x, Val):
            if x.type == 'stri' and x.value == old:
                x.value = new
        elif isinstance(x, Obj):
            for k, v in x.items:
                walk(v)
        elif isinstance(x, Arr):
            for v in x.items:
                walk(v)
    walk(n)


def patch_routes_v7(data):
    p = Pkg(data)
    groups = p.root['RouteGroupings'].items
    home = next(g for g in groups if g['Id'].value == 'fe_home')
    career = next(g for g in groups if g['Id'].value == 'fe_career')
    routes = home['Routes'].items
    home['Routes'].items = [r for r in routes if r['Id'].value != 'mod_menu']
    r = copy.deepcopy(next(r for r in routes if r['Id'].value == 'career'))
    r['Id'].value = 'mod_menu'
    _rename_values(r, 'fe_career', SCREEN)
    home['Routes'].items.append(r)
    g = copy.deepcopy(career)
    g['Routes'].items = [x for x in g['Routes'].items if x['Id'].value in ('back_1', 'open', 'open_no_input', 'close')]
    for x in g['Routes'].items:
        if x['Id'].value == 'back_1':
            x['Id'].value = 'back'
    _rename_values(g, 'fe_career', SCREEN)
    assert not any(x['Id'].value == SCREEN for x in groups)
    groups.append(g)
    return p.to_bytes()


def patch_region_loading(data):
    p = Pkg(data)
    reg = next(r for r in p.root['regions']['Contents'].items if r['Name'].value == 'region_undefined')
    c = reg['Contents']
    assert c.get(SCREEN) is None
    e = copy.deepcopy(c['fe_home'])
    e['Name'].value = SCREEN
    e['Value'].value = FE_HOME_GROUP
    c.set(SCREEN, e)
    return p.to_bytes()


def patch_flows(data):
    p = Pkg(data)
    flow = next(f for f in p.root['Flows'].items if name(f) == 'frontend_flow')
    scene = flow.items[CHILDREN].items[0]
    entries = scene.items[CHILDREN].items
    assert not any(name(e) == f'enter_{SCREEN}' for e in entries)
    e = copy.deepcopy(next(e for e in entries if name(e) == 'enter_fe_career'))
    e.items[PROPS]['name'].value = f'enter_{SCREEN}'
    e.items[PROPS]['screen']['value'].value = SCREEN
    entries.append(e)
    fix_ids(flow)
    return p.to_bytes()


def patch_fe_home_panel_gate(data):
    p = Pkg(data)
    fe_home = p.root['Screens'].items[0]
    cp = child(fe_home, 'contextual_panels')
    decision = child(cp, 'contextual_decision')
    panel = 'fe_home.contextual_panels.offline_contextual_panel'
    gate = 'fe_home.contextual_panels.swofty_panel_gate'
    for e in decision.items[EVENTS].items:
        if e['eventName'].value in ('default', 'offline'):
            sigs = e['signals'].items
            e['signals'].items = [sig(gate, 'query', e['eventName'].value) if s['to_object_name'].value == panel and s['to_pin_name'].value == 'show' else s for s in sigs]
    cp.items[CHILDREN].items.append(
        node('swofty_panel_gate', 'SVCD', [('value', prop('bool', True)),
                                           ('dbEntry', prop('stri', f'{DB}.options.show_launcher_panel.value'))],
             events=[event('yes', [sig(panel, 'show', 'yes')])]))
    fix_ids(fe_home)
    return p.to_bytes()


RESTART_NOTE = 'Applies after a restart.'
RESTART_DESC = 'Close the game and start it again with the changed settings.'


def load_settings(path):
    with open(path, encoding='utf-8') as f:
        st = json.load(f)
    assert st.get('schema') == 1, 'settings schema'
    return st


def clip(s, n=255):
    return s.encode('utf-8')[:n].decode('utf-8', 'ignore')


def all_options(st):
    return [(i, m, o) for i, m in enumerate(st['mods']) for o in m.get('options', [])]


def option_desc(o):
    return clip(' '.join(x for x in (o.get('description', ''), RESTART_NOTE if o['binds'] == 'db' else '') if x))


def option_texts(o):
    return [clip(f"{o['label']}: {s['text']}".upper()) for s in o['states']]


def launcher_panel_on(st):
    return next((bool(o['states'][o.get('current', 0)]['value']) for i, m, o in all_options(st)
                 if m['id'] == 'swofty.core' and o['id'] == 'show_launcher_panel' and o['type'] == 'toggle'), True)


def bake_panel_gate(data, on):
    if on:
        return data
    p = Pkg(data)
    fe_home = p.root['Screens'].items[0]
    decision = child(child(fe_home, 'contextual_panels'), 'contextual_decision')
    panel = 'fe_home.contextual_panels.offline_contextual_panel'
    for e in decision.items[EVENTS].items:
        if e['eventName'].value in ('default', 'offline'):
            e['signals'].items = [s for s in e['signals'].items
                                  if (s['to_object_name'].value, s['to_pin_name'].value) != (panel, 'show')]
    return p.to_bytes()


def state_moves(kind, i, n):
    if kind == 'slider':
        return [('pressed', (i + 1) % n), ('navLeft', i - 1 if i > 0 else None), ('navRight', i + 1 if i < n - 1 else None)]
    return [('pressed', (i + 1) % n), ('navLeft', (i - 1) % n), ('navRight', (i + 1) % n)]


def build_screen_literal(fe_home, templates, st, target=SCREEN):
    tile_t, head_t = tile_templates(templates)
    scr, kids, action = screen_frame(fe_home)
    rows, focusable = [], []
    audio = lambda pin, f: sig(f'{SCREEN}.audio_theme_trigger', pin, f)
    restart = st.get('restart_command', 'swofty_restart')
    t = make_tile(tile_t, 'apply_restart', label='APPLY & RESTART')
    t.items[PROPS].set('visible', prop('bool', False))
    t.items[EVENTS].items.append(event('pressed', [audio('confirm', 'pressed'), raise_cmd(target, 'pressed', restart, restart)]))
    focus_desc(t, RESTART_DESC)
    rows.append(t)

    def states(base, texts, cmds, cur, kind, desc, db):
        names = [f'{base}_{i}' for i in range(len(texts))]
        for i, text in enumerate(texts):
            t = make_tile(tile_t, names[i], label=text)
            t.items[PROPS].set('visible', prop('bool', i == cur))
            for ev, j in state_moves(kind, i, len(texts)):
                if j is None or j == i:
                    continue
                to = f'{SCREEN}.{names[j]}'
                t.items[EVENTS].items.append(event(ev, [sig(to, 'show', ev), sig(to, 'focus', ev), raise_cmd(target, ev, cmds[j], cmds[j])]
                                                   + ([sig(f'{SCREEN}.apply_restart', 'show', ev)] if db else [])
                                                   + [audio('confirm' if ev == 'pressed' else 'navigate', ev),
                                                      sig(f'{SCREEN}.{names[i]}', 'hide', ev)]))
            focus_desc(t, desc)
            rows.append(t)
        focusable.append(names[cur])

    k = 0
    for i, m in enumerate(st['mods']):
        rows.append(header_tile(head_t, f'm{i}_header', clip(f"{m['name']} {m['version']}".upper())))
        desc = clip(m.get('description', ''))
        if m['locked']:
            t = make_tile(tile_t, f'm{i}_enabled', label='ENABLED: ALWAYS ON')
            for ev in ('pressed', 'navLeft', 'navRight'):
                t.items[EVENTS].items.append(event(ev, [audio('back', ev)]))
            focus_desc(t, desc)
            rows.append(t)
            focusable.append(f'm{i}_enabled')
        else:
            states(f'm{i}_enabled', ['ENABLED: OFF', 'ENABLED: ON'], m['enable_commands'], int(bool(m['enabled'])),
                   'toggle', desc, True)
        for o in m.get('options', []):
            if m['enabled']:
                states(f's{k}', option_texts(o), [x['command'] for x in o['states']], o.get('current', 0), o['type'],
                       option_desc(o), o['binds'] == 'db')
            k += 1
    scr = finish_screen(scr, action, kids, rows, [], [], focusable[0] if focusable else 'apply_restart')
    action.items[EVENTS].items = [e for e in action.items[EVENTS].items if e['signals'].items]
    return scr

SCREEN_UI = f'eaid://ui_package/screens/{SCREEN}.ui'
OWN_GROUP = f'asset_groups/ui_package/screens/{SCREEN}.assetgroup'


def build_screen_ui(fe_home_ui, syst, st=None, target=SCREEN):
    src = Pkg(fe_home_ui)
    fe_home = src.root['Screens'].items[0]
    assert name(fe_home) == 'fe_home'
    templates = Pkg(syst).root['Templates'].items
    p = Pkg()
    screen = build_screen(fe_home, templates) if st is None else build_screen_literal(fe_home, templates, st, target)
    p.root = Obj([(S('Screens'), Arr([screen]))])
    return p.to_bytes()


def patch_region_loading_own(data):
    p = Pkg(data)
    reg = next(r for r in p.root['regions']['Contents'].items if r['Name'].value == 'region_undefined')
    c = reg['Contents']
    assert c.get(SCREEN) is None
    e = copy.deepcopy(c['fe_home'])
    e['Name'].value = SCREEN
    e['Value'].value = OWN_GROUP
    c.set(SCREEN, e)
    return p.to_bytes()


def _strip_commands(route, drop_types):
    cmds = route['Commands'].items
    ids = {}
    for c in cmds:
        at = {a['Id'].value: a for a in c['Attributes'].items}
        ids[at['Id']['Value'].value] = (c['Type'].value, c.get('Dependencies').value if c.get('Dependencies') is not None else '')
    dropped = {i for i, (t, d) in ids.items() if t in drop_types}

    def resolve(dep):
        out = []
        for d in [x for x in dep.split(',') if x]:
            if d in dropped:
                out += resolve(ids[d][1])
            else:
                out.append(d)
        return out
    keep = []
    for c in cmds:
        cid = next(a['Value'].value for a in c['Attributes'].items if a['Id'].value == 'Id')
        if cid in dropped:
            continue
        if c.get('Dependencies') is not None:
            deps = resolve(c['Dependencies'].value)
            if deps:
                c['Dependencies'].value = ','.join(dict.fromkeys(deps))
            else:
                c.items = [(k, v) for k, v in c.items if k.value != 'Dependencies']
        keep.append(c)
    route['Commands'].items = keep
    if route.get('Dependencies') is not None:
        route['Dependencies'].value = ','.join(dict.fromkeys(resolve(route['Dependencies'].value)))


def patch_routes_own(data):
    p = Pkg(patch_routes_v7(data))
    g = next(x for x in p.root['RouteGroupings'].items if x['Id'].value == SCREEN)
    for r in g['Routes'].items:
        if r['Id'].value in ('open', 'open_no_input'):
            _strip_commands(r, {'FrontendSceneSwitch', 'ViewModelCallback'})
    return p.to_bytes()
