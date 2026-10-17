import sys, os, copy
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pkg import Pkg, Obj, Arr, Val
from uinode import PROPS, CHILDREN, EVENTS, child, name, fix_ids
from swofty_settings import (SCREEN, S, V, obj, prop, sig, event, node, raise_cmd, var, dictionary, make_tile,
                             tile_templates, screen_frame, patch_routes_v7, _strip_commands)
import swofty_popup as sp

STATE = '#ui_menu_data.swofty'
WATCHDOG_SECONDS = 10.0
UIDM = sp.UIDM

OPTIONS = [
    dict(key='show_launcher_panel', kind='toggle', value=True,
         on='SHOW LAUNCHER PANEL: ON', off='SHOW LAUNCHER PANEL: OFF',
         desc='Show the "Launched with Swofty Mod Launcher" panel on the home screen.'),
    dict(key='mod_menu_key', kind='info', value='F9', label='MOD MENU KEY: F9 / R3',
         desc='Key that opens these settings from the home screen. Keyboard F9, controller R3.'),
]
MODS = [
    dict(id='swoftymod', name='SwoftyMod', version='1.0.0', enabled=True, locked=True,
         desc='The SwoftyMod core. Always on while the launcher is in use.'),
    dict(id='grid_2026_24', name='F1 2026 24-car grid', version='0.1.0', enabled=True, locked=False,
         desc='Adds a 12th team to the F1 2026 season for a 24-car grid. Applies after a restart.'),
    dict(id='sepang', name='Sepang International Circuit', version='0.1.0', enabled=True, locked=False,
         desc='Adds Sepang in the spare bahrain24 track slot. Applies after a restart.'),
]


def mod_label(m, on):
    return f"{m['name']} {m['version']}: {'ON' if on else 'OFF'}{' (ALWAYS ON)' if m['locked'] else ''}".upper()


def patch_ui_data(data):
    p = Pkg(data)
    c = p.root['ui_menu_data']['Contents']
    assert c.get('swofty') is None
    entries = [('show_launcher_panel', var('show_launcher_panel', 'bool', True)),
               ('mod_menu_key', var('mod_menu_key', 'uistring', 'F9'))]
    entries += [(f'mod_{m["id"]}', var(f'mod_{m["id"]}', 'bool', m['enabled'])) for m in MODS]
    c.set('swofty', dictionary('swofty', entries))
    return p.to_bytes()


def labels_to(row, frm, text):
    return [sig(f'{SCREEN}.{row}.{l}', 'setText', frm, {'text': text})
            for l in ('button_label', 'button_label_highlighted', 'button_label_dummy')]


def flip_nodes(row, path, on, off, cmd_data):
    fl, son, soff = f'{row}_flip', f'{row}_set_on', f'{row}_set_off'
    return [
        node(fl, 'SVCD', [('value', prop('bool', True)), ('dbEntry', prop('stri', path))],
             events=[event('yes', [sig(f'{SCREEN}.{soff}', 'trigger', 'yes')] + labels_to(row, 'yes', off)),
                     event('no', [sig(f'{SCREEN}.{son}', 'trigger', 'no')] + labels_to(row, 'no', on))]),
        node(son, 'UIDS', [('source', prop('bool', True)), ('target', prop('bool', False, path))]),
        node(soff, 'UIDS', [('source', prop('bool', False)), ('target', prop('bool', False, path))]),
    ], [sig(f'{SCREEN}.{fl}', 'query', 'pressed'), sig(f'{SCREEN}.audio_theme_trigger', 'confirm', 'pressed'),
        raise_cmd(SCREEN, 'pressed', 'swofty_set', cmd_data)]


def watchdog_reset():
    s = sig(f'{SCREEN}.watchdog', 'reset', 'opened_screen', {'duration': 0})
    s['attributes']['duration'].set('value', V('fp32', WATCHDOG_SECONDS))
    return s


def build_screen(fe_home, templates):
    tile_t, _ = tile_templates(templates)
    scr, kids, action = screen_frame(fe_home)
    drop = [k for k, v in scr.items[PROPS].items if k.value == 'visualNav']
    scr.items[PROPS].items = [(k, v) for k, v in scr.items[PROPS].items if k.value != 'visualNav']
    assert drop
    rows, helpers, focusable = [], [], []

    def header(row, text):
        t = make_tile(tile_t, row, label=text)
        t.items[PROPS].set('enabled', prop('bool', False))
        rows.append(t)

    def item(row, label, desc, press):
        t = make_tile(tile_t, row, label=label)
        t.items[EVENTS].items += [event('pressed', press),
                                  event('focused', [sig(f'{SCREEN}.description', 'setText', 'focused', {'text': desc})])]
        rows.append(t)
        focusable.append(t)

    header('header_swoftymod', 'SWOFTYMOD')
    for o in OPTIONS:
        row = f'option_{o["key"]}'
        if o['kind'] == 'toggle':
            nodes, press = flip_nodes(row, f'{STATE}.{o["key"]}', o['on'], o['off'], f'options.{o["key"]}')
            helpers += nodes
            item(row, o['on'] if o['value'] else o['off'], o['desc'], press)
        else:
            item(row, o['label'], o['desc'], [sig(f'{SCREEN}.audio_theme_trigger', 'confirm', 'pressed'),
                                              raise_cmd(SCREEN, 'pressed', 'swofty_set', f'options.{o["key"]}')])
    header('header_installed', 'INSTALLED MODS')
    for i, m in enumerate(MODS):
        row = f'mod_{m["id"]}'
        if m['locked']:
            item(row, mod_label(m, True), m['desc'], [sig(f'{SCREEN}.audio_theme_trigger', 'back', 'pressed')])
        else:
            nodes, press = flip_nodes(row, f'{STATE}.mod_{m["id"]}', mod_label(m, True), mod_label(m, False),
                                      f'mods.{m["id"]}.enabled')
            helpers += nodes
            item(row, mod_label(m, m['enabled']), m['desc'], press)

    for a, b in zip(focusable, focusable[1:]):
        a.items[EVENTS].items.append(event('navDown', [sig(f'{SCREEN}.{name(b)}', 'focus', 'navDown')]))
        b.items[EVENTS].items.append(event('navUp', [sig(f'{SCREEN}.{name(a)}', 'focus', 'navUp')]))

    watchdog = node('watchdog', 'TIME', [],
                    events=[event('time_out_expired', [sig(f'{SCREEN}.Action', 'route', 'time_out_expired', {'key': 'back'})])])
    helpers.append(watchdog)
    action.items[EVENTS].items = [
        event('opened_screen', [watchdog_reset()]),
        event('finishedTranIn', [sig(f'{SCREEN}.watchdog', 'stop', 'finishedTranIn'),
                                 sig(f'{SCREEN}.tooltips', 'ttActivate', 'finishedTranIn'),
                                 sig(f'{SCREEN}.{name(focusable[0])}', 'focus', 'finishedTranIn')]),
        event('finishedTranOut', [sig(f'{SCREEN}.watchdog', 'stop', 'finishedTranOut'),
                                  sig(f'{SCREEN}.tooltips', 'ttDeactive', 'finishedTranOut')]),
    ]
    scr.items[CHILDREN].items = kids + rows + helpers
    fix_ids(scr)
    return scr


def build_screen_ui(fe_home_ui, syst):
    fe_home = Pkg(fe_home_ui).root['Screens'].items[0]
    assert name(fe_home) == 'fe_home'
    p = Pkg()
    p.root = Obj([(S('Screens'), Arr([build_screen(fe_home, Pkg(syst).root['Templates'].items)]))])
    return p.to_bytes()


def patch_routes(data):
    p = Pkg(patch_routes_v7(data))
    g = next(x for x in p.root['RouteGroupings'].items if x['Id'].value == SCREEN)
    for r in g['Routes'].items:
        if r['Id'].value in ('open', 'open_no_input'):
            _strip_commands(r, {'ViewModelCallback'})
    return p.to_bytes()


def patch_fe_home_gate(data):
    p = Pkg(data)
    fe_home = p.root['Screens'].items[0]
    cp = child(fe_home, 'contextual_panels')
    decision = child(cp, 'contextual_decision')
    panel = 'fe_home.contextual_panels.offline_contextual_panel'
    gate = 'fe_home.contextual_panels.swofty_panel_gate'
    for e in decision.items[EVENTS].items:
        if e['eventName'].value in ('default', 'offline'):
            e['signals'].items.append(sig(gate, 'query', e['eventName'].value))
    cp.items[CHILDREN].items.append(
        node('swofty_panel_gate', 'SVCD', [('value', prop('bool', False)),
                                           ('dbEntry', prop('stri', f'{STATE}.show_launcher_panel'))],
             events=[event('yes', [sig(panel, 'hide', 'yes')])]))
    fix_ids(fe_home)
    return p.to_bytes()


def patch_persistent_gate(data):
    p = Pkg(sp.patch_persistent_gate(data))
    scr = p.root['Screens'].items[0]
    m = child(child(scr, 'consume_input_wrapper'), 'persistent_multichoice_message')
    once = child(m, 'swofty_once_check')
    seen = child(m, 'swofty_seen_check')
    restore = [sig(f'{UIDM}.title', 'setLocalise', 'no', {'localise': True}),
               sig(f'{UIDM}.body', 'setLocalise', 'no', {'localise': True})]
    literal = [sig(f'{UIDM}.title', 'setLocalise', 'no', {'localise': False}),
               sig(f'{UIDM}.title', 'setText', 'no', {'text': sp.TITLE}),
               sig(f'{UIDM}.body', 'setLocalise', 'no', {'localise': False}),
               sig(f'{UIDM}.body', 'setText', 'no', {'text': sp.BODY})]
    for e in once.items[EVENTS].items:
        if e['eventName'].value == 'no':
            e['signals'].items = restore + e['signals'].items
    for e in seen.items[EVENTS].items:
        if e['eventName'].value == 'no':
            e['signals'].items = e['signals'].items + literal
    fix_ids(scr)
    return p.to_bytes()
