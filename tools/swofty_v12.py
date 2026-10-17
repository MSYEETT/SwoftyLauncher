import sys, os, copy, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pkg import Pkg, Obj, Arr, Val
from uinode import PROPS, CHILDREN, LAYOUT, EVENTS, name, child, children
import uiops as u

HOME = 'swofty_settings'
STATE = '#ui_menu_data.swofty'
HOME_LAYOUT = 'eaid://ui_package/art/screens/options/options_screen'
PAGE_LAYOUT = 'eaid://ui_package/art/screens/options/options_edit_screen'
SLOTS = {1: ['1a'], 2: ['2a', '2b'], 3: ['3a', '3b', '3c'], 4: ['4a', '4b', '4c', '4d']}


def load(path):
    st = json.load(open(path))
    mods = [m for m in st['mods'] if m.get('enabled') and m.get('options')]
    k = 0
    for i, m in enumerate(mods):
        m['_screen'] = f'swofty_mod_{i}'
        for o in m['options']:
            o['_k'] = k
            k += 1
    return st, mods


def page_name(i):
    return f'swofty_mod_{i}'


class Sources:
    def __init__(self, syst, fe_career_ui, fe_home_ui):
        self.T = {name(t): t for t in Pkg(syst).root['Templates'].items}
        self.career = Pkg(fe_career_ui).root['Screens'].items[0]
        self.home = Pkg(fe_home_ui).root['Screens'].items[0]

    def frame(self, scr, back_key, tips=('select', 'back')):
        return u.frame(scr, child(self.career, 'Action'), child(self.home, 'audio_theme_trigger'),
                       child(self.home, 'back_listener'), child(self.career, 'tooltips'), tips, back_key)

    def title(self, scr, panel, text):
        t = copy.deepcopy(child(self.T[panel], 'screen_title'))
        u.rename_refs(t, f'{panel}.screen_title', f'{name(scr)}.screen_title')
        u.strip_data(t)
        u.literal_label(child(t, 'title'), text)
        u.literal_label(child(t, 'hidden_settings_text'), '')
        for c in children(t):
            if name(c) == 'hidden_settings_bg':
                c.items[PROPS].set('visible', u.prop('bool', False))
        return t


def build_home(src, mods):
    scr = u.screen(HOME, HOME_LAYOUT, props_from=src.career.items[PROPS])
    fr = src.frame(scr, 'back')
    kids = [src.title(scr, 'settings_home_screen', 'MOD SETTINGS')]
    kids += [u.hidden(f'hide_{n}', n) for n in ('filter', 'side_panel', 'scroll_bar', 'scroll_bar_panel')]
    tiles = []
    for b in range(0, len(mods), 4):
        group = mods[b:b + 4]
        block = u.instance(src.T['settings_home_block'], HOME, f'block_{b // 4}')
        for j, m in enumerate(group):
            i = b + j
            t = u.instance(src.T['settings_home_category'], f'{HOME}.{name(block)}', f'cat_{i}',
                           texts={'title': m['name'].upper(), 'subtitle': m.get('description', '')})
            t.items[LAYOUT] = u.layout_path(SLOTS[len(group)][j])
            for c in children(t):
                if name(c) == 'icon':
                    c.items[PROPS].set('visible', u.prop('bool', False))
            u.add_events(t, u.event('pressed', [u.sig(f'{HOME}.audio_theme_trigger', 'confirm', 'pressed'),
                                                u.sig(f'{HOME}.Action', 'route', 'pressed', {'key': f'open_mod_{i}'})]))
            block.items[CHILDREN].items.append(t)
            tiles.append((block, t))
        kids.append(block)
    for (ba, a), (bb, b) in zip(tiles, tiles[1:]):
        pa, pb = f'{HOME}.{name(ba)}.{name(a)}', f'{HOME}.{name(bb)}.{name(b)}'
        nav = ('navDown', 'navUp') if ba is bb else ('navRight', 'navLeft')
        u.add_events(a, u.event(nav[0], [u.sig(pb, 'focus', nav[0])]))
        u.add_events(b, u.event(nav[1], [u.sig(pa, 'focus', nav[1])]))
    first = f'{name(tiles[0][0])}.{name(tiles[0][1])}'
    return u.finish(scr, fr, kids, first)


def _option_nodes(scr_name, row_path, value_label, o):
    k, states = o['_k'], o['states']
    n = len(states)
    path = f'{STATE}.o{k}'
    wrap = o['type'] != 'slider'
    helpers = [u.node(f'o{k}_set_{j}', 'UIDS', [('source', u.prop('stri', str(j))), ('target', u.prop('stri', '', path))])
               for j in range(n)]

    def step(dirn):
        evs = []
        for j in range(n):
            t = j + dirn
            if not 0 <= t < n:
                if not wrap:
                    continue
                t %= n
            cmd = states[t]['command']
            evs.append(u.event(str(j), [u.sig(f'{scr_name}.o{k}_set_{t}', 'trigger', str(j)),
                                        u.sig(value_label, 'setText', str(j), {'text': states[t]['text']}),
                                        u.sig(f'{scr_name}.audio_theme_trigger', 'confirm', str(j)),
                                        u.raise_cmd(scr_name, str(j), cmd, cmd)]))
        return u.node(f'o{k}_{"next" if dirn > 0 else "prev"}', 'MVCD', [('dbEntry', u.prop('stri', path))], events=evs)
    helpers += [step(1), step(-1)]
    return helpers, f'{scr_name}.o{k}_next', f'{scr_name}.o{k}_prev'


def build_page(src, m, i):
    sname = page_name(i)
    scr = u.screen(sname, PAGE_LAYOUT, props_from=src.career.items[PROPS])
    fr = src.frame(scr, 'back')
    sel = next(c for c in children(fr['tooltips']) if name(c) == 'select')
    for tip, tname in (('decrease', 'slider_decrease'), ('increase', 'slider_increase')):
        c = copy.deepcopy(sel)
        u.rename_refs(c, 'fe_career.tooltips.select', f'fe_career.tooltips.{tip}')
        c.items[PROPS]['name'].value = tip
        u.set_prop_value(c, 'tooltipsName', tname)
        fr['tooltips'].items[CHILDREN].items.append(c)
    kids = [src.title(scr, 'settings_edit_screen', m['name'].upper())]
    kids += [u.hidden(f'hide_{n}', n) for n in ('filter', 'scroll_bar')]
    desc = u.instance(src.T['settings_edit_description_panel'], sname, 'description')
    dset = u.instance(src.T['settings_edit_description_set'], f'{sname}.description', 'set',
                      texts={'title': m['name'].upper(), 'body': m.get('description', '')})
    desc.items[CHILDREN].items.append(dset)
    kids.append(desc)
    rows, helpers = [], []
    for o in m['options']:
        k = o['_k']
        cur = o['states'][o.get('current', 0)]
        row = f'row_{k}'
        if o['type'] == 'slider':
            t = u.instance(src.T['settings_edit_slider'], sname, row,
                           texts={'slider.label': o['label'].upper(), 'slider.value': cur['text']})
            sl = child(t, 'slider')
            vals = [s['value'] for s in o['states']]
            for key, v in (('min', vals[0]), ('max', vals[-1]), ('value', cur['value'])):
                sl.items[PROPS][key]['value'].value = int(v)
            value_label = f'{sname}.{row}.slider.value'
        else:
            t = u.instance(src.T['settings_edit_toggle'], sname, row,
                           texts={'toggle.label': o['label'].upper(), 'toggle.value': cur['text']})
            value_label = f'{sname}.{row}.toggle.value'
        for c in children(t):
            for cc in children(c):
                if name(cc) == 'indicator':
                    cc.items[PROPS].set('visible', u.prop('bool', False))
        hs, nxt, prv = _option_nodes(sname, row, value_label, o)
        helpers += hs
        desc_text = o.get('description', '') + (' Applies after a restart.' if o.get('binds') == 'db' else '')
        u.add_events(t,
                     u.event('navRight', [u.sig(nxt, 'query', 'navRight')]),
                     u.event('navLeft', [u.sig(prv, 'query', 'navLeft')]),
                     u.event('focused', [u.sig(f'{sname}.description.set.title', 'setText', 'focused', {'text': o['label'].upper()}),
                                         u.sig(f'{sname}.description.set.body', 'setText', 'focused', {'text': desc_text})]))
        if o['type'] != 'slider':
            u.add_events(t, u.event('pressed', [u.sig(nxt, 'query', 'pressed')]))
        rows.append(t)
    u.chain_focus(sname, rows)
    return u.finish(scr, fr, kids + rows + helpers, name(rows[0]))


def build_all(syst, fe_career_ui, fe_home_ui, settings_path):
    st, mods = load(settings_path)
    src = Sources(syst, fe_career_ui, fe_home_ui)
    screens = {HOME: u.ui_bytes(build_home(src, mods))}
    for i, m in enumerate(mods):
        screens[page_name(i)] = u.ui_bytes(build_page(src, m, i))
    return st, mods, screens


def routes(mods):
    home_routes = u.screen_routes(HOME) + [u.migrate_route('back', HOME, 'frontend_flow', 'fe_home')]
    home_routes += [u.subpage_route(f'open_mod_{i}', HOME, page_name(i)) for i in range(len(mods))]
    groups = [u.route_group(HOME, home_routes)]
    for i in range(len(mods)):
        groups.append(u.route_group(page_name(i), u.screen_routes(page_name(i)) +
                                    [u.subpage_route('back', page_name(i), HOME, rtype=2)]))
    return groups


def mod_menu_route(routes_bytes):
    p = Pkg(routes_bytes)
    home = next(g for g in p.root['RouteGroupings'].items if g['Id'].value == 'fe_home')
    src = next(r for r in home['Routes'].items if r['Id'].value == 'options')
    r = copy.deepcopy(src)
    r['Id'].value = 'mod_menu'

    def walk(x):
        if isinstance(x, Val):
            if x.type == 'stri' and x.value == 'settings_container':
                x.value = HOME
            elif x.type == 'stri' and x.value == 'open_no_input':
                x.value = 'open'
        elif isinstance(x, Obj):
            for k, v in x.items:
                walk(v)
        elif isinstance(x, Arr):
            for v in x.items:
                walk(v)
    walk(r)
    return r


def state_entries(mods):
    return [(f'o{o["_k"]}', u.uidb_var(f'o{o["_k"]}', 'uistring', str(o.get('current', 0)))) for m in mods for o in m['options']]


def panel_baked_off(st):
    for m in st['mods']:
        if m['id'] == 'swofty.core':
            for o in m.get('options', []):
                if o['id'] == 'show_launcher_panel':
                    return o['states'][o.get('current', 1)]['value'] is False
    return False


def bake_home_panel(fe_home_bytes, st):
    if not panel_baked_off(st):
        return fe_home_bytes
    p = Pkg(fe_home_bytes)
    fe_home = p.root['Screens'].items[0]
    dec = child(child(fe_home, 'contextual_panels'), 'contextual_decision')
    for e in dec.items[EVENTS].items:
        e['signals'].items = [s for s in e['signals'].items
                              if not (s['to_object_name'].value.endswith('offline_contextual_panel') and s['to_pin_name'].value == 'show')]
    return p.to_bytes()


PAGE_COMPONENT = 'eaid://ui_package/art/screens/options/options_edit_screen'


def _details(m):
    by = f"by {m.get('author', 'unknown')}  ·  v{m.get('version', '')}"
    return by


def _set_components(n, path, cname):
    n.items[u.COMPONENTS] = Arr([u.obj(('componentId', Val('ui64', u.component_id(path))), ('debugName', u.S(path)),
                                       ('name', u.S(cname)))])


def _back_listener(src, nm, enabled, signals):
    b = copy.deepcopy(child(src.home, 'back_listener'))
    b.items[PROPS]['name'].value = nm
    if not enabled:
        b.items[PROPS].set('enabled', u.prop('bool', False))
    b.items[EVENTS].items = [u.event('pressed', signals)]
    return b


def build_single(src, mods):
    scr = u.screen(HOME, HOME_LAYOUT, props_from=src.career.items[PROPS])
    fr = src.frame(scr, 'back')
    title = src.title(scr, 'settings_home_screen', 'MOD SETTINGS')
    kids = [title]
    kids += [u.hidden(f'hide_{n}', n) for n in ('filter', 'side_panel', 'scroll_bar', 'scroll_bar_panel')]
    blocks, tiles = [], []
    for b in range(0, len(mods), 4):
        group = mods[b:b + 4]
        block = u.instance(src.T['settings_home_block'], HOME, f'block_{b // 4}')
        for j, m in enumerate(group):
            i = b + j
            sub = f"{_details(m)}  ·  {m.get('description', '')}".strip(' ·')
            t = u.instance(src.T['settings_home_category'], f'{HOME}.{name(block)}', f'cat_{i}',
                           texts={'title': m['name'].upper(), 'subtitle': sub})
            t.items[LAYOUT] = u.layout_path(SLOTS[len(group)][j])
            for c in children(t):
                if name(c) == 'icon':
                    c.items[PROPS].set('visible', u.prop('bool', False))
            block.items[CHILDREN].items.append(t)
            tiles.append((block, t, i))
        blocks.append(block)
        kids.append(block)
    for (ba, a, _), (bb, b, _) in zip(tiles, tiles[1:]):
        pa, pb = f'{HOME}.{name(ba)}.{name(a)}', f'{HOME}.{name(bb)}.{name(b)}'
        nav = ('navDown', 'navUp') if ba is bb else ('navRight', 'navLeft')
        u.add_events(a, u.event(nav[0], [u.sig(pb, 'focus', nav[0])]))
        u.add_events(b, u.event(nav[1], [u.sig(pa, 'focus', nav[1])]))

    home_parts = [f'{HOME}.screen_title'] + [f'{HOME}.{name(bl)}' for bl in blocks]
    pages, page_backs, helpers = [], [], []
    for (block, tile, i), m in zip(tiles, mods):
        pname = f'page_{i}'
        ppath = f'{HOME}.{pname}'
        page = u.node(pname, 'UIGP', [('visible', u.prop('bool', False))], layout=('transition_holder',),
                      behav=u.GROUP_BEHAVIOURS)
        _set_components(page, PAGE_COMPONENT, 'options_edit_screen')
        ptitle = copy.deepcopy(child(src.T['settings_edit_screen'], 'screen_title'))
        u.rename_refs(ptitle, 'settings_edit_screen.screen_title', f'{ppath}.screen_title')
        u.strip_data(ptitle)
        u.literal_label(child(ptitle, 'title'), m['name'].upper())
        u.literal_label(child(ptitle, 'hidden_settings_text'), _details(m).upper())
        for c in children(ptitle):
            if name(c) in ('hidden_settings_bg', 'hidden_settings_text'):
                c.items[PROPS].set('visible', u.prop('bool', True))
        pk = [ptitle, u.hidden('hide_filter', 'filter'), u.hidden('hide_scroll_bar', 'scroll_bar')]
        desc_body = f"{m.get('description', '')} ({_details(m)})".strip()
        desc = u.instance(src.T['settings_edit_description_panel'], ppath, 'description')
        dset = u.instance(src.T['settings_edit_description_set'], f'{ppath}.description', 'set',
                          texts={'title': m['name'].upper(), 'body': desc_body})
        desc.items[CHILDREN].items.append(dset)
        pk.append(desc)
        rows = []
        for o in m['options']:
            k = o['_k']
            cur = o['states'][o.get('current', 0)]
            row = f'row_{k}'
            if o['type'] == 'slider':
                t = u.instance(src.T['settings_edit_slider'], ppath, row,
                               texts={'slider.label': o['label'].upper(), 'slider.value': cur['text']})
                sl = child(t, 'slider')
                vals = [s['value'] for s in o['states']]
                for key, v in (('min', vals[0]), ('max', vals[-1]), ('value', cur['value'])):
                    sl.items[PROPS][key]['value'].value = int(v)
                value_label = f'{ppath}.{row}.slider.value'
            else:
                t = u.instance(src.T['settings_edit_toggle'], ppath, row,
                               texts={'toggle.label': o['label'].upper(), 'toggle.value': cur['text']})
                value_label = f'{ppath}.{row}.toggle.value'
            for c in children(t):
                for cc in children(c):
                    if name(cc) == 'indicator':
                        cc.items[PROPS].set('visible', u.prop('bool', False))
            hs, nxt, prv = _option_nodes(HOME, row, value_label, o)
            helpers += hs
            dtext = o.get('description', '') + (' Applies after a restart.' if o.get('binds') == 'db' else '')
            u.add_events(t,
                         u.event('navRight', [u.sig(nxt, 'query', 'navRight')]),
                         u.event('navLeft', [u.sig(prv, 'query', 'navLeft')]),
                         u.event('focused', [u.sig(f'{ppath}.description.set.title', 'setText', 'focused', {'text': o['label'].upper()}),
                                             u.sig(f'{ppath}.description.set.body', 'setText', 'focused', {'text': dtext})]))
            if o['type'] != 'slider':
                u.add_events(t, u.event('pressed', [u.sig(nxt, 'query', 'pressed')]))
            rows.append(t)
        u.chain_focus(ppath, rows)
        page.items[CHILDREN].items = pk + rows
        pages.append(page)
        first_row = f'{ppath}.{name(rows[0])}'
        u.add_events(tile, u.event('pressed',
                                   [u.sig(f'{HOME}.audio_theme_trigger', 'confirm', 'pressed')]
                                   + [u.sig(p, 'hide', 'pressed') for p in home_parts]
                                   + [u.sig(ppath, 'show', 'pressed'),
                                      u.sig(f'{HOME}.back_listener', 'disable', 'pressed'),
                                      u.sig(f'{HOME}.back_{pname}', 'enable', 'pressed'),
                                      u.sig(first_row, 'focus', 'pressed')]))
        page_backs.append(_back_listener(src, f'back_{pname}', False,
                                         [u.sig(f'{HOME}.audio_theme_trigger', 'back', 'pressed'),
                                          u.sig(ppath, 'hide', 'pressed')]
                                         + [u.sig(p, 'show', 'pressed') for p in home_parts]
                                         + [u.sig(f'{HOME}.back_{pname}', 'disable', 'pressed'),
                                            u.sig(f'{HOME}.back_listener', 'enable', 'pressed'),
                                            u.sig(f'{HOME}.{name(block)}.{name(tile)}', 'focus', 'pressed')]))
    first = f'{name(tiles[0][0])}.{name(tiles[0][1])}'
    return u.finish(scr, fr, kids + pages + page_backs + helpers, first)


def build_all_single(syst, fe_career_ui, fe_home_ui, settings_path):
    st, mods = load(settings_path)
    src = Sources(syst, fe_career_ui, fe_home_ui)
    return st, mods, {HOME: u.ui_bytes(build_single(src, mods))}


def routes_single():
    return [u.route_group(HOME, u.screen_routes(HOME) + [u.migrate_route('back', HOME, 'frontend_flow', 'fe_home')])]
