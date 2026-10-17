import sys, os, re, json, glob, struct, subprocess, hashlib, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from erp import parse
from pkg import Pkg, Obj, Arr, Val, fourcc, to_py
from uinode import djb2, is_node, name, PROPS, CHILDREN, COMMANDS, EVENTS, LAYOUT, COMPONENTS, BEHAVIOURS

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GAME = os.environ.get('F1_25_DIR') or sys.exit('set F1_25_DIR to the game folder')
UIP = f'{GAME}/2025_asset_groups/ui_package'
OUT = f'{ROOT}/work/ui/catalog'
PRISTINE = f'{ROOT}/work/pristine/2025_asset_groups/ui_package'
LOGS = sorted(glob.glob(f'{OUT}/logs/*.log'))
CACHE = f'{OUT}/cache'

SPAWNERS = {'F1TS', 'UIL2'}


def read_frags(path, meta, frags):
    out = []
    with open(path, 'rb') as f:
        for fn, off, size, flags, comp, packed in frags:
            f.seek(meta['data_off'] + off)
            raw = f.read(packed)
            if comp == 0x11:
                raw = subprocess.run(['zstd', '-dc'], input=raw, capture_output=True, check=True).stdout
            out.append((fn, raw))
    return out


def resource(path, meta, r):
    key = r[4]
    os.makedirs(CACHE, exist_ok=True)
    names = [fr[0] for fr in r[3]]
    files = [f'{CACHE}/{key}.{i}.{n}' for i, n in enumerate(names)]
    if all(os.path.exists(f) for f in files):
        return {n: open(f, 'rb').read() for n, f in zip(names, files)}
    frs = read_frags(path, meta, r[3])
    for f, (n, data) in zip(files, frs):
        open(f, 'wb').write(data)
    return dict(frs)


def index_erps():
    erps = {}
    for f in sorted(glob.glob(f'{UIP}/*.erp')) + sorted(glob.glob(f'{UIP}/screens/*.erp')):
        rel = os.path.relpath(f, UIP)
        pristine = f'{PRISTINE}/{rel}'
        if os.path.exists(pristine):
            f = pristine
        if rel.startswith('screens/swofty'):
            continue
        meta, res = parse(f)
        erps[rel] = (f, meta, res)
    return erps


def log_facts():
    facts = {'loaded_erps': [], 'unloaded_erps': [], 'vm_registered': set(), 'nodes_registered': set(),
             'opened_screens': [], 'vm_wait_screens': set(), 'screen_setup_ids': set()}
    for lp in LOGS:
        if not os.path.exists(lp):
            continue
        before_home = True
        last_open = None
        for line in open(lp, errors='replace'):
            if '[game]' not in line:
                continue
            m = re.search(r'(?:queueLoad|startRead - read|finishedLoadingERP) - "(?:/data_win/)?2025_asset_groups/ui_package/([^"]+\.erp)"', line)
            if m and 'texture_arrays' not in m.group(1):
                facts['loaded_erps'].append((os.path.basename(lp), m.group(1), before_home))
            m = re.search(r'processUnload - "/data_win/2025_asset_groups/ui_package/([^"]+\.erp)"', line)
            if m and 'texture_arrays' not in m.group(1):
                facts['unloaded_erps'].append((os.path.basename(lp), m.group(1)))
            m = re.search(r'registerViewModelForViewModelBinding: \[([^\]]*)\]', line)
            if m:
                facts['vm_registered'].add(m.group(1))
            m = re.search(r'registerNodeForViewModelBinding: \[(\d+)\]', line)
            if m:
                facts['nodes_registered'].add(int(m.group(1)))
            m = re.search(r'OPEN SCREEN \(([^)]+)\)', line)
            if m:
                last_open = m.group(1)
                facts['opened_screens'].append((os.path.basename(lp), m.group(1)))
                if m.group(1) == 'fe_home':
                    before_home = False
            if 'in ViewModelBehaviour' in line and last_open:
                facts['vm_wait_screens'].add(last_open)
            m = re.search(r'ScreenSetup Element:(\d+)', line)
            if m:
                facts['screen_setup_ids'].add(int(m.group(1)))
    return facts


def walk(n, parent=None):
    path = name(n) if parent is None else f'{parent}.{name(n)}'
    yield path, n
    if isinstance(n.items[CHILDREN], Arr):
        for c in n.items[CHILDREN].items:
            if is_node(c):
                yield from walk(c, path)


def ntype(n):
    t = n.items[PROPS].get('type')
    return fourcc(t.value) if t is not None else '?'


def dbentries(x, out):
    if isinstance(x, Obj):
        for k, v in x.items:
            if k.value in ('dbentry', 'dbEntry') and isinstance(v, Val) and isinstance(v.value, str):
                out.append(v.value)
            else:
                dbentries(v, out)
    elif isinstance(x, Arr):
        for v in x.items:
            dbentries(v, out)


def pval(n, key):
    o = n.items[PROPS].get(key)
    if isinstance(o, Obj) and isinstance(o.get('value'), Val):
        v = o['value'].value
        d = o.get('dbentry')
        return v, (d.value if isinstance(d, Val) else None)
    if isinstance(o, Val):
        return o.value, None
    return None, None


def signals(n):
    for slot in (EVENTS, COMMANDS):
        arr = n.items[slot]
        for e in arr.items if isinstance(arr, Arr) else []:
            if not isinstance(e, Obj) or e.get('signals') is None:
                continue
            ev = e.get('eventName') or e.get('commandName')
            for s in e['signals'].items:
                yield slot, ev.value if ev else None, s


def attr_flat(a):
    return to_py(a) if a is not None else None


def db_root(entry):
    m = re.match(r'#([A-Za-z0-9_]+)', entry)
    return m.group(1) if m else None


def uidb_roots(erps):
    roots = {}
    for rel, (path, meta, res) in erps.items():
        if rel.startswith('screens/'):
            continue
        for r in res:
            if r[1] != 'UIDB':
                continue
            frs = resource(path, meta, r)
            for fn, data in frs.items():
                if data[:4] != b'!pkg':
                    continue
                p = Pkg(data)
                for k in p.root.keys():
                    roots.setdefault(k, []).append({'uidb': r[0], 'erp': rel})
    return roots


def tooltip_names(erps):
    path, meta, res = erps['databases.erp']
    for r in res:
        if r[0].endswith('tool_tips.uidb'):
            py = to_py(Pkg(list(resource(path, meta, r).values())[0]).root)
            return set(py['tool_tips']['Contents']['tooltips_dictionary']['Contents'].keys())
    return set()


def region_map(p):
    out = {}
    for reg in to_py(p.root)['regions']['Contents']:
        for k, v in reg['Contents'].items():
            out[k] = {'region': reg['Name'], 'assetgroup': v['Value']}
    return out


def messages(erps):
    out = {}
    for rel, (path, meta, res) in erps.items():
        if rel.startswith('screens/'):
            continue
        for r in res:
            if r[1] != 'UIMD':
                continue
            for fn, data in resource(path, meta, r).items():
                if data[:4] != b'!pkg':
                    continue
                py = to_py(Pkg(data).root)
                for m in py.get('messages', []) or []:
                    mid = m.get('id') if isinstance(m, dict) else None
                    if isinstance(mid, dict):
                        mid = mid.get('value')
                    if mid:
                        out.setdefault(mid, []).append({'file': r[0], 'erp': rel})
    return out


def persistent_data(erps):
    path, meta, res = erps['persistent.erp']
    data = {}
    for r in res:
        if r[1] in ('UIRO', 'UCON', 'UIContentGroup') or r[0].endswith('flows.ui'):
            frs = resource(path, meta, r)
            data[r[0]] = Pkg(frs.get('temp') or list(frs.values())[0])
    return data


def parse_flows(p):
    flows = []
    for f in p.root['Flows'].items:
        o = f.items[PROPS]
        fl = {'name': o['name'].value, 'asset_group': pval(f, 'asset_group')[0], 'allowVMBinding': pval(f, 'allowVMBinding')[0],
              'blocking_flows': pval(f, 'blocking_flows')[0], 'scenes': []}
        for sc in f.items[CHILDREN].items:
            if not is_node(sc):
                continue
            s = {'name': name(sc), 'camera': pval(sc, 'Camera')[0], 'activators': []}
            for a in sc.items[CHILDREN].items:
                if is_node(a):
                    s['activators'].append({'name': name(a), 'screen': pval(a, 'screen')[0], 'command': pval(a, 'command')[0]})
            fl['scenes'].append(s)
        flows.append(fl)
    return flows


def kv(attrs):
    out = {}
    for a in attrs or []:
        out[a.get('Id')] = a.get('Value')
    return out


def parse_routes(pd):
    groupings = {}
    for rn, p in pd.items():
        if not rn.endswith('.uir'):
            continue
        for g in to_py(p.root).get('RouteGroupings', []):
            gid = g['Id']
            routes = []
            for r in g.get('Routes', []):
                cmds = []
                for c in r.get('Commands', []):
                    a = kv(c.get('Attributes'))
                    a.pop('Id', None)
                    cmds.append({'type': c.get('Type'), **a})
                routes.append({'id': r['Id'], 'attrs': kv(r.get('Attributes')), 'commands': cmds})
            groupings.setdefault(gid, []).append({'file': rn.split('/')[-1], 'grouping_attrs': kv(g.get('Attributes')), 'routes': routes})
    return groupings


def screen_info(sc, templates, ui_name, others, tooltips, msgs, roots_uidb):
    nodes = list(walk(sc))
    root = sc.items[PROPS]
    props = {}
    for k, v in root.items:
        if k.value in ('id', 'nameId', 'version', 'name', 'type'):
            continue
        props[k.value] = to_py(v)
    layout = []
    for grp in sc.items[LAYOUT].items if isinstance(sc.items[LAYOUT], Arr) else []:
        for b in grp.items if isinstance(grp, Arr) else []:
            if isinstance(b, Obj) and b.get('isComponentId') is not None:
                layout.append(b['debugName'].value if b.get('debugName') else str(b['componentId'].value))
    types = collections.Counter()
    comps, xref_comps = set(), set()
    db_read, db_write, spawners, tts, pops, cmds, routes_fired, notifies, exits, ext_sig = [], [], [], [], [], [], [], [], [], set()
    static_interactive = []
    vm_contexts = 0
    own = name(sc)
    for path, n in nodes:
        t = ntype(n)
        types[t] += 1
        for grp in n.items[LAYOUT].items if isinstance(n.items[LAYOUT], Arr) else []:
            for b in grp.items if isinstance(grp, Arr) else []:
                if isinstance(b, Obj) and b.get('isComponentId') is not None and n is not sc:
                    xref_comps.add(b['debugName'].value if b.get('debugName') else str(b['componentId'].value))
        for c in n.items[COMPONENTS].items if isinstance(n.items[COMPONENTS], Arr) else []:
            if isinstance(c, Obj):
                comps.add(c['debugName'].value if c.get('debugName') else str(c.get('componentId').value))
        ents = []
        dbentries(n.items[PROPS], ents)
        if t == 'UIDS':
            v, d = pval(n, 'target')
            if d:
                db_write.append(d)
        db_read += [e for e in ents if e]
        dc_v, dc = pval(n, 'dataContext')
        if dc and t in SPAWNERS:
            spawners.append({'path': path, 'type': t, 'dataContext': dc, 'layout': [b['debugName'].value for grp in n.items[LAYOUT].items if isinstance(grp, Arr) for b in grp.items if isinstance(b, Obj) and b.get('debugName')]})
        elif dc:
            vm_contexts += 1
        if t in ('TILE', 'UIBT'):
            static_interactive.append(path)
        if t == 'UITL':
            v, d = pval(n, 'tooltipsName')
            if v:
                tts.append(v)
        if t == 'UIMC':
            v, d = pval(n, 'message_id')
            if v:
                pops.append(v)
        if t == 'UINT':
            v, d = pval(n, 'NotifyName')
            if v:
                notifies.append(v)
        if t == 'UIEX':
            exits.append({'path': path, 'migrate_to': pval(n, 'migrate_to')[0], 'migrate_entry': pval(n, 'migrate_entry')[0]})
        for slot, ev, s in signals(n):
            to, pin = s['to_object_name'].value, s['to_pin_name'].value
            at = attr_flat(s.get('attributes'))
            if pin == 'raise_command' and isinstance(at, dict):
                c = at.get('command', {})
                nm = c.get('name', {}).get('value') if isinstance(c.get('name'), dict) else c.get('name')
                if nm:
                    cmds.append(nm)
            if pin == 'route' and isinstance(at, dict):
                k = at.get('key', {})
                k = k.get('value') if isinstance(k, dict) else k
                if k:
                    routes_fired.append(k)
            top = to.split('.')[0]
            if top != own and top not in others:
                ext_sig.add(f'{to}.{pin}')
    reads = collections.Counter()
    for e in db_read:
        r = db_root(e)
        reads[('#' + r) if r else 'vm-relative'] += 1
    tpl_names = [name(t) for t in templates]
    tpl_comps = set()
    for t in templates:
        for p, n in walk(t):
            for c in n.items[COMPONENTS].items if isinstance(n.items[COMPONENTS], Arr) else []:
                if isinstance(c, Obj) and c.get('debugName'):
                    tpl_comps.add(c['debugName'].value)
    if types.get('UITC') and not spawners:
        content = 'native-tabbed'
    elif spawners and static_interactive:
        content = 'mixed'
    elif spawners:
        content = 'native-spawned'
    else:
        content = 'static'
    global_roots = sorted({db_root(e) for e in db_read + db_write if db_root(e)})
    return {
        'screen': own,
        'ui': ui_name,
        'screens_in_same_ui': sorted(others - {own}),
        'props': props,
        'allowVMBinding': bool(props.get('allowVMBinding', {}).get('value')) if isinstance(props.get('allowVMBinding'), dict) else False,
        'sceneAttributes': props.get('sceneAttributes'),
        'behaviours': [fourcc(b['type'].value) for b in sc.items[BEHAVIOURS].items if isinstance(b, Obj)] if isinstance(sc.items[BEHAVIOURS], Arr) else [],
        'layout': layout,
        'xref_components': sorted(xref_comps),
        'instanced_components': sorted(comps),
        'templates': tpl_names,
        'template_components': sorted(tpl_comps),
        'node_count': len(nodes),
        'node_types': dict(types.most_common()),
        'content': content,
        'spawners': spawners,
        'static_interactive': len(static_interactive),
        'vm_context_groups': vm_contexts,
        'db_reads': dict(reads.most_common()),
        'db_roots': [{'root': r, 'kind': 'uidb' if r in roots_uidb else 'native-vm'} for r in global_roots],
        'db_writes': sorted(set(db_write)),
        'tooltips': sorted(set(tts)),
        'tooltips_unknown': sorted(set(tts) - tooltips) if tooltips else [],
        'popups': sorted(set(pops)),
        'popups_unknown': sorted(set(pops) - set(msgs)),
        'raise_commands': sorted(set(cmds)),
        'route_keys_fired': sorted(set(routes_fired)),
        'notifies': sorted(set(notifies)),
        'exits': exits,
        'signals_to_other_screens': sorted(ext_sig),
    }


AREAS = [
    ('boot', ['flow_boot']), ('persistent', ['flow_persistent', 'persistent']), ('frontend', ['flow_frontend', 'common_front_end', 'flow_upsell', 'flow_playercard', 'flow_playercard_menu', 'flow_f1_life_driver_tags', 'flow_render_badges', 'flow_render_names', 'flow_paddock_pass', 'flow_fz_environment']),
    ('options', ['flow_options']), ('loading', ['flow_loading']), ('customisation', ['flow_customisation']),
    ('f1_world', ['flow_f1_world']), ('career', ['flow_career_season_setup', 'flow_career_hub', 'flow_career_rewards', 'flow_team_hq', 'flow_driver_hq_environment', 'flow_r_and_d', 'flow_st_hub', 'flow_press_interview', 'flow_season_complete']),
    ('in_session_menu', ['flow_pause', 'flow_garage', 'flow_pre_session', 'flow_post_race', 'flow_post_race_rewards', 'flow_car_monitor', 'flow_award_ceremony', 'flow_replay', 'flow_highlight_export', 'flow_race_intro']),
    ('hud', ['flow_race', 'flow_race_director', 'flow_race_assets_f1', 'flow_steering_screen', 'flow_hide_ui']),
]


def area_of(erp):
    b = erp[:-4]
    for a, lst in AREAS:
        if b in lst:
            return a
    return 'other'


def main():
    erps = index_erps()
    facts = log_facts()
    roots = uidb_roots(erps)
    tooltips = tooltip_names(erps)
    msgs = messages(erps)
    pd = persistent_data(erps)
    flows = parse_flows(pd['eaid://ui_package/flows.ui'])
    groupings = parse_routes(pd)
    region = region_map(pd['eaid://ui_package/region_loading.uiconfig'])
    flow_by_group = {f['asset_group']: f['name'] for f in flows if f['asset_group']}
    activators = collections.defaultdict(list)
    for f in flows:
        for s in f['scenes']:
            for a in s['activators']:
                activators[a['screen']].append({'flow': f['name'], 'scene': s['name'], 'activator': a['name'], 'command': a['command']})

    loaded_main_menu = sorted({e for _, e, _ in facts['loaded_erps']} - {e for _, e in facts['unloaded_erps']})
    loaded_ever = sorted({e for _, e, _ in facts['loaded_erps']} | {e for _, e in facts['unloaded_erps']})

    carriers = collections.defaultdict(list)
    per_screen = collections.defaultdict(list)
    ui_res = {}
    for rel, (path, meta, res) in erps.items():
        for r in res:
            if r[1] != 'UIDF':
                continue
            if rel.startswith('screens/'):
                per_screen[r[0]].append({'erp': rel, 'md5': r[4]})
            else:
                carriers[r[0]].append({'erp': rel, 'md5': r[4], 'flow': flow_by_group.get(f'asset_groups/ui_package/{rel[:-4]}.assetgroup')})
                ui_res.setdefault(r[0], (path, meta, r))
    only_per_screen = sorted(set(per_screen) - set(carriers))
    for u in only_per_screen:
        rel = per_screen[u][0]['erp']
        path, meta, res = erps[rel]
        ui_res[u] = (path, meta, next(r for r in res if r[0] == u))

    reached = collections.defaultdict(list)
    migrations = []
    for gid, gs in groupings.items():
        for g in gs:
            for r in g['routes']:
                for c in r['commands']:
                    tgt = c.get('Target')
                    if c['type'] in ('OpenScreen', 'TriggerSubRoute', 'TransitionScreen') and tgt and tgt != gid:
                        if c['type'] != 'TriggerSubRoute' or str(c.get('SubRoute', '')).startswith('open') or c.get('SubRoute') in ('back_to', 'show'):
                            reached[tgt].append(f'{gid}.{r["id"]}')
                    if c['type'] == 'Migrate':
                        migrations.append({'grouping': gid, 'route': r['id'], **{k: v for k, v in c.items() if k != 'type'}})

    screens = []
    for u, (path, meta, r) in sorted(ui_res.items()):
        frs = resource(path, meta, r)
        temp = Pkg(frs['temp']) if 'temp' in frs else None
        syst = Pkg(frs['syst']) if 'syst' in frs else None
        if temp is None or temp.root.get('Screens') is None:
            continue
        scs = [s for s in temp.root['Screens'].items if is_node(s)]
        tpls = [t for t in syst.root['Templates'].items if is_node(t)] if syst is not None and syst.root.get('Templates') is not None else []
        others = {name(s) for s in scs}
        for sc in scs:
            info = screen_info(sc, tpls, u, others, tooltips, msgs, roots)
            nm = info['screen']
            car = carriers.get(u, [])
            md5s = {c['md5'] for c in car}
            info['carriers'] = car
            info['carrier_copies_identical'] = len(md5s) <= 1
            info['per_screen_erps'] = per_screen.get(u, [])
            info['per_screen_copy_matches_flow'] = (not car) or all(p['md5'] in md5s for p in per_screen.get(u, []))
            info['loaded_on_pc'] = bool(car)
            info['loaded_at_main_menu'] = any(c['erp'] in loaded_main_menu for c in car)
            info['area'] = area_of(car[0]['erp']) if car else 'per-screen-only'
            info['region'] = region.get(nm)
            info['activators'] = activators.get(nm, [])
            gs = groupings.get(nm, [])
            info['routes'] = [{'file': g['file'], 'attrs': g['grouping_attrs'], 'routes': [{'id': x['id'], 'destination': x['attrs'].get('Destination'),
                              'commands': [c['type'] + ('(' + ','.join(f'{k}={v}' for k, v in c.items() if k != 'type') + ')' if len(c) > 1 else '') for c in x['commands']]} for x in g['routes']]} for g in gs]
            info['reached_from'] = sorted(set(reached.get(nm, [])))
            info['migrations_in_routes'] = [m for m in migrations if m['grouping'] == nm]
            info['vm_registered'] = sorted(v for v in facts['vm_registered'] if v.split('.')[-1] == nm or (v.count('.') >= 1 and v.split('.')[1] == nm))
            info['vm_node_registered_at_boot'] = djb2(nm) in facts['nodes_registered']
            info['opened_in_log'] = any(s == nm for _, s in facts['opened_screens'])
            info['log_waited_for_vm'] = nm in facts['vm_wait_screens']
            screens.append(info)

    os.makedirs(OUT, exist_ok=True)
    summary = {
        'source': {'game': GAME, 'logs': LOGS},
        'loaded_ui_erps_at_main_menu': loaded_main_menu,
        'loaded_ui_erps_in_log': loaded_ever,
        'flows': flows,
        'route_groupings': {k: [g['file'] for g in v] for k, v in groupings.items()},
        'migrations': migrations,
        'uidb_roots': roots,
        'ui_only_in_per_screen_erps': only_per_screen,
        'vm_registered': sorted(facts['vm_registered']),
        'opened_in_log': facts['opened_screens'],
    }
    json.dump({'meta': summary, 'screens': screens}, open(f'{OUT}/screens.json', 'w'), indent=1, ensure_ascii=False, default=list)
    cols = ['screen', 'area', 'carrier', 'main_menu', 'allowVMBinding', 'vm_registered', 'content', 'spawners', 'layout', 'ui', 'activators', 'routes', 'reached_from', 'tooltips', 'popups', 'raise_commands']
    with open(f'{OUT}/screens.tsv', 'w') as f:
        f.write('\t'.join(cols) + '\n')
        for s in screens:
            row = [s['screen'], s['area'], ','.join(c['erp'] for c in s['carriers']) or 'screens/*.erp only', int(s['loaded_at_main_menu']), int(s['allowVMBinding']),
                   int(bool(s['vm_registered'])), s['content'], ','.join(f"{x['type']}:{x['dataContext']}" for x in s['spawners']),
                   ','.join(l.split('/')[-1] for l in s['layout']), s['ui'].split('/')[-1], ','.join(f"{a['flow']}.{a['activator']}" for a in s['activators']),
                   ','.join(r['id'] for g in s['routes'] for r in g['routes']), ','.join(s['reached_from']), ','.join(s['tooltips']), ','.join(s['popups']), ','.join(s['raise_commands'])]
            f.write('\t'.join(str(x) for x in row) + '\n')
    print(len(screens), 'screens', len(ui_res), '.ui', len(only_per_screen), 'per-screen-only')


if __name__ == '__main__':
    if len(sys.argv) > 1:
        LOGS = sys.argv[1:]
    main()
