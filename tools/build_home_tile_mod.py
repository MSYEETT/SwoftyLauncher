import sys, os, copy, argparse, hashlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pkg import Pkg, Obj, Arr, Val
from erpw import Erp
from uinode import (PROPS, CHILDREN, EVENTS, child, children, name, clone_node, set_prop_value, drop_key,
                    signal, fix_ids)

GAME = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'work/pristine')
UI = '2025_asset_groups/ui_package'
FE_HOME = 'eaid://ui_package/screens/fe_home.ui'
ROUTES = 'eaid://ui_package/routes/routes_frontend_general.uir'
TOOLTIPS = 'eaid://ui_package/databases/tool_tips.uidb'
ACTION_PREFIX = '#input_data.action_sets.menu_actions.actions.'


def patch_fe_home(data, label, action, route_key, tooltip_name, literal_text):
    p = Pkg(data)
    screen = p.root['Screens'].items[0]
    assert name(screen) == 'fe_home'

    tooltips = child(screen, 'tooltips')
    tip = clone_node(child(tooltips, 'settings'), 'fe_home.tooltips', 'mod_menu')
    set_prop_value(tip, 'tooltipsName', tooltip_name)
    text = child(tip, 'text')
    set_prop_value(text, 'localise', False, 'bool')
    if literal_text:
        t = text.items[PROPS]['text']
        t['value'].value = label
        drop_key(t, 'dbentry')
    tooltips.items[CHILDREN].items.append(tip)

    listener = clone_node(child(screen, 'settings'), 'fe_home', 'mod_menu_listener')
    set_prop_value(listener, 'action_name', action)
    ev = listener.items[EVENTS].items[0]
    assert ev['eventName'].value == 'pressed'
    ev['signals'].items = [
        signal('fe_home.Action', 'route', 'pressed', {'key': route_key}),
        signal('fe_home.audio_theme_trigger', 'confirm', 'pressed'),
    ]
    fix_ids(listener, 'fe_home')
    screen.items[CHILDREN].items.append(listener)
    return p.to_bytes()


ITEM_BINDINGS = ('title', 'title_localised', 'is_dlc_content', 'tutorial_ref.layer_override')


def unbind_item_data(n):
    from uinode import is_node
    p = n.items[PROPS]
    for k, v in p.items:
        if isinstance(v, Obj) and v.get('dbentry') is not None and v['dbentry'].value in ITEM_BINDINGS:
            drop_key(v, 'dbentry')
    for c in children(n):
        if is_node(c):
            unbind_item_data(c)


def patch_fe_home_tile(data, syst, label, route_key, tile_name='mod_tile'):
    from uinode import COMMANDS, is_node
    p = Pkg(data)
    screen = p.root['Screens'].items[0]
    tmpl = next(t for t in Pkg(syst).root['Templates'].items if name(t) == 'fe_tile_button_default')
    tile = copy.deepcopy(tmpl)
    tile.items[PROPS]['name'].value = tile_name
    from uinode import rename_refs
    rename_refs(tile, 'fe_tile_button_default', f'fe_home.{tile_name}')
    tile.items[CHILDREN].items = [c for c in children(tile) if name(c) not in ('enabled', 'notification_type')]
    tile.items[COMMANDS].items = []
    unbind_item_data(tile)
    for lbl in ('button_label', 'button_label_highlighted', 'button_label_dummy'):
        c = child(tile, lbl)
        t = c.items[PROPS]['text']
        t['value'].value = label
        drop_key(t, 'isloctag')
        set_prop_value(c, 'localise', False, 'bool')
    tile.items[EVENTS].items = [
        Obj([(Val('stri', 'eventName'), Val('stri', 'pressed')),
             (Val('stri', 'signals'), Arr([signal('fe_home.Action', 'route', 'pressed', {'key': route_key}),
                                            signal('fe_home.audio_theme_trigger', 'confirm', 'pressed')])),
             (Val('stri', 'event'), Val('ui32', 0))]),
        Obj([(Val('stri', 'eventName'), Val('stri', 'navUp')),
             (Val('stri', 'signals'), Arr([signal('fe_home.spawnable_buttons', 'focus_and_reset', 'navUp')])),
             (Val('stri', 'event'), Val('ui32', 0))]),
    ]
    fix_ids(tile, 'fe_home')
    screen.items[CHILDREN].items.append(tile)
    return p.to_bytes()


def literal_text(node, text):
    t = node.items[PROPS]['text']
    t['value'].value = text
    drop_key(t, 'dbentry')
    drop_key(t, 'isloctag')
    set_prop_value(node, 'localise', False, 'bool')


def patch_offline_panel(data, description, prompt, tooltip_name):
    p = Pkg(data)
    screen = p.root['Screens'].items[0]
    cp = child(screen, 'contextual_panels')
    panel = child(cp, 'offline_contextual_panel')
    literal_text(child(panel, 'subtitle'), description)
    literal_text(child(panel, 'tooltip_label'), prompt)
    set_prop_value(child(panel, 'tooltip_extra_2'), 'tooltipsName', tooltip_name)
    decision = child(cp, 'contextual_decision')
    events = {e['eventName'].value: e for e in decision.items[EVENTS].items}
    off = events['offline']['signals']
    off.items = [s for s in off.items if s['to_object_name'].value != 'fe_home.contextual_panels.extra_2_listener']
    events['default']['signals'].items.append(signal('fe_home.contextual_panels.offline_contextual_panel', 'show', 'default'))
    fix_ids(screen)
    return p.to_bytes()


def patch_routes(data, route_key, src_key='options'):
    p = Pkg(data)
    for g in p.root['RouteGroupings'].items:
        if g['Id'].value == 'fe_home':
            routes = g['Routes'].items
            assert not any(r['Id'].value == route_key for r in routes)
            r = copy.deepcopy(next(r for r in routes if r['Id'].value == src_key))
            r['Id'].value = route_key
            routes.append(r)
            return p.to_bytes()
    raise KeyError('fe_home grouping')


def patch_tooltips(data, tooltip_name, label, action, src='settings_fe_home'):
    p = Pkg(data)
    d = p.root['tool_tips']['Contents']['tooltips_dictionary']['Contents']
    assert d.get(tooltip_name) is None
    e = copy.deepcopy(d[src])
    e['Name'].value = tooltip_name
    c = e['Contents']
    c['action_path']['Value'].value = ACTION_PREFIX + action
    c['text']['Value'].value = label
    c['priority']['Value'].value = '5'
    d.set(tooltip_name, e)
    return p.to_bytes()


def add_tooltip(data, tooltip_name, label, action, src):
    p = Pkg(data)
    d = p.root['tool_tips']['Contents']['tooltips_dictionary']['Contents']
    assert d.get(tooltip_name) is None
    e = copy.deepcopy(d[src])
    e['Name'].value = tooltip_name
    c = e['Contents']
    c['action_path']['Value'].value = ACTION_PREFIX + action
    c['text']['Value'].value = label
    d.set(tooltip_name, e)
    return p.to_bytes()


def write_erp(rel, out_root, patches):
    src = os.path.join(GAME, UI, rel)
    e = Erp(src)
    for res_name, frag, fn in patches:
        r = e.find(res_name)
        new = fn(e.frag_data(r, frag))
        e.set_frag(r, frag, new)
    dst = os.path.join(out_root, UI, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    e.to_file(dst)
    check = Erp(dst)
    for res_name, frag, fn in patches:
        r = check.find(res_name)
        assert hashlib.md5(b''.join(check.frag_data(r, i) for i in range(len(r.frags)))).digest() == r.hash
    print('wrote', dst, os.path.getsize(dst))
    return dst


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('variant', choices=['v1', 'v2', 'v3', 'v4', 'v6', 'v7'])
    ap.add_argument('--out', default=os.path.join(os.path.dirname(GAME), 'ui/mod'))
    ap.add_argument('--dest')
    ap.add_argument('--label')
    ap.add_argument('--prompt-label')
    ap.add_argument('--action', default='Menu Right Stick Click')
    ap.add_argument('--offline-text', default='Launched with Swofty Mod Launcher')
    ap.add_argument('--offline-prompt', default='to open settings')
    a = ap.parse_args()
    if a.label is None:
        a.label = 'MOD SETTINGS' if a.variant in ('v4', 'v6', 'v7') else 'MOD MENU'
    if a.prompt_label is None:
        a.prompt_label = 'Mod Settings' if a.variant in ('v4', 'v6', 'v7') else a.label
    out = a.dest or os.path.join(a.out, a.variant)
    if a.variant == 'v1':
        fe = lambda d: patch_fe_home(d, a.label, a.action, 'options', 'toggle_difficulty', True)
    else:
        fe = lambda d: patch_fe_home(d, a.label, a.action, 'mod_menu', 'mod_menu', False)
        if a.variant in ('v3', 'v4'):
            fe2 = fe
            syst = Erp(os.path.join(GAME, UI, 'screens/fe_home.erp'))
            syst_data = syst.frag_data(syst.find(FE_HOME), 1)
            fe = lambda d: patch_fe_home_tile(fe2(d), syst_data, a.label, 'mod_menu')
        tips = lambda d: patch_tooltips(d, 'mod_menu', a.prompt_label, a.action)
        if a.variant in ('v4', 'v6', 'v7'):
            fe3 = fe
            fe = lambda d: patch_offline_panel(fe3(d), a.offline_text, a.offline_prompt, 'mod_menu_offline')
            tips2 = tips
            tips = lambda d: add_tooltip(tips2(d), 'mod_menu_offline', a.offline_prompt, a.action, 'delete')
        if a.variant == 'v7':
            import swofty_settings as ss
            syst = Erp(os.path.join(GAME, UI, 'screens/fe_home.erp'))
            syst_data = syst.frag_data(syst.find(FE_HOME), 1)
            fe4 = fe
            fe = lambda d: ss.patch_fe_home_screen(ss.patch_fe_home_panel_gate(fe4(d)), syst_data)
            write_erp('persistent.erp', out, [
                (ROUTES, 0, lambda d: ss.patch_routes_v7(patch_routes(d, 'mod_menu'))),
                ('eaid://ui_package/flows.ui', 0, ss.patch_flows),
                ('eaid://ui_package/region_loading.uiconfig', 0, ss.patch_region_loading),
            ])
            write_erp('databases.erp', out, [
                (TOOLTIPS, 0, tips),
                ('eaid://ui_package/databases/ui_data.uidb', 0, ss.patch_ui_data),
            ])
        else:
            write_erp('persistent.erp', out, [(ROUTES, 0, lambda d: patch_routes(d, 'mod_menu'))])
            write_erp('databases.erp', out, [(TOOLTIPS, 0, tips)])
    write_erp('flow_frontend.erp', out, [(FE_HOME, 0, fe)])
    write_erp('screens/fe_home.erp', out, [(FE_HOME, 0, fe)])


if __name__ == '__main__':
    main()
