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


NODES_OUT = f'{ROOT}/tools/uicatalog/catalog.json'
SKIP_PROPS = {'id', 'nameId', 'version', 'name', 'type'}
SCALARS = {'stri', 'bool', 'ui32', 'si32', 'fp32', 'rgba', 'ui64'}
SCENES = {}


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for b in iter(lambda: f.read(1 << 20), b''):
            h.update(b)
    return h.hexdigest()


def stock_archives(stock_dirs, baselines):
    want = {}
    for b in baselines:
        for k, v in json.load(open(b))['files'].items():
            want[k] = (v['sha256'], v['size'])
    picked, refused = {}, []
    for f in sorted(glob.glob(f'{UIP}/**/*.erp', recursive=True)):
        rel = os.path.relpath(f, GAME)
        if '/texture_arrays/' in rel or os.path.basename(rel).startswith('swofty'):
            continue
        cands = [f'{d}/{rel}' for d in stock_dirs] + [f]
        if rel not in want:
            picked[rel] = next(c for c in cands if os.path.exists(c))
            continue
        sha, size = want[rel]
        pick = next((c for c in cands if os.path.exists(c) and os.path.getsize(c) == size and sha256(c) == sha), None)
        if pick:
            picked[rel] = pick
        else:
            refused.append(rel)
    return picked, refused


def load_frags(path, data_off, frags):
    out = []
    with open(path, 'rb') as f:
        for fn, off, size, flags, comp, packed in frags:
            f.seek(data_off + off)
            raw = f.read(packed)
            if comp == 0x11:
                raw = subprocess.run(['zstd', '-dc'], input=raw, capture_output=True, check=True).stdout
            out.append((fn, raw))
    return out


def vshape(v):
    if isinstance(v, Val):
        return '=' + v.type
    if isinstance(v, Arr):
        return 'arr'
    if isinstance(v, Obj):
        keys = [k.value for k, _ in v.items]
        if 'value' in keys:
            inner = v['value']
            base = inner.type if isinstance(inner, Val) else vshape(inner)
            extra = sorted(k for k in keys if k != 'value')
            return base + (('+' + '+'.join(extra)) if extra else '')
        return 'obj{' + ','.join(sorted(keys)) + '}'
    return type(v).__name__


def attr_shape(a, pre=''):
    if isinstance(a, Val):
        return [f'{pre}:{a.type}']
    if isinstance(a, Obj):
        return [x for k, v in a.items for x in attr_shape(v, f'{pre}.{k.value}' if pre else k.value)]
    if isinstance(a, Arr):
        return [f'{pre}[]']
    return [pre]


def el_type(e):
    t = e.get('typeName')
    return t.value if isinstance(t, Val) else '?'


def el_name(e):
    t = e.get('name')
    return t.value if isinstance(t, Val) else '?'


def el_kids(e):
    c = e.get('children')
    return [x for x in c.items if isinstance(x, Obj)] if isinstance(c, Arr) else []


def scene_job(job):
    rname, path, data_off, frags = job
    C, EX = collections.Counter(), {}
    index = {}
    data = load_frags(path, data_off, frags)[0][1]
    if data[:4] != b'!pkg':
        return rname, index, C, EX
    root = Pkg(data).root
    sl = root.get('scenelist')
    for entry in sl.items if isinstance(sl, Arr) else []:
        sid = entry['name'].value
        leaf, xrefs = {}, []
        index[sid] = (leaf, xrefs)

        def walk_el(e, parent, path):
            t = el_type(e)
            nm = el_name(e)
            here = nm if parent is None else (nm if not path else f'{path}.{nm}')
            xr = e.get('xref')
            leaf.setdefault(nm, []).append((t, xr.value if isinstance(xr, Val) else None, here if parent is not None else ''))
            if isinstance(xr, Val):
                xrefs.append(xr.value)
            C[('el', t)] += 1
            EX.setdefault(('el', t), f'{rname}:{here}')
            if parent is not None:
                C[('elparent', t, parent)] += 1
                C[('elchild', parent, t)] += 1
            pa = e.get('params')
            for it in pa.items if isinstance(pa, Arr) else []:
                if isinstance(it, Obj) and isinstance(it.get('name'), Val):
                    v = it.get('value')
                    C[('elparam', t, it['name'].value, v.type if isinstance(v, Val) else vshape(v))] += 1
            for k in ('geoms', 'anims'):
                a = e.get(k)
                if isinstance(a, Arr) and a.items:
                    C[('el' + k, t)] += 1
            if isinstance(xr, Val):
                C[('elxref', t)] += 1
            if t == 'eMask':
                EX.setdefault(('mask', rname), here)
                C[('maskscene', rname)] += 1
            for c in el_kids(e):
                walk_el(c, t, '' if parent is None else here)

        sc = entry.get('scene')
        if isinstance(sc, Obj):
            walk_el(sc, None, '')
    return rname, index, C, EX


def lookup(nm, sid, prefix):
    hits = [h + (sid,) for h in SCENES[sid][0].get(nm, []) if h[2].startswith(prefix)]
    if hits or prefix:
        return hits
    seen, queue = set(), [sid]
    while queue:
        s = queue.pop(0)
        if s in seen or s not in SCENES:
            continue
        seen.add(s)
        names, xrefs = SCENES[s]
        if nm in names:
            return [h + (s,) for h in names[nm]]
        queue.extend(xrefs)
    return []


def resolve(names, sid, prefix=''):
    if sid not in SCENES:
        return [], 'no-scene'
    hits = lookup(names[0], sid, prefix)
    if not hits:
        return [], 'no-element'
    if len(names) == 1:
        return [hits[0][0]], hits[0][1]
    best = None
    for t, xr, path, s in sorted(hits, key=lambda h: h[1] is None):
        chain, last = resolve(names[1:], xr, '') if xr is not None else resolve(names[1:], s, path + '.')
        if isinstance(last, (int, type(None))):
            return [t] + chain, last
        best = best or ([t] + chain, last)
    return best


def node_scene(n):
    comps = n.items[COMPONENTS]
    for c in comps.items if isinstance(comps, Arr) else []:
        if isinstance(c, Obj) and isinstance(c.get('componentId'), Val):
            return c['componentId'].value
    for grp in layout_groups(n):
        for b in grp:
            if b.get('isComponentId') is not None and isinstance(b.get('componentId'), Val):
                return b['componentId'].value
    return None


def layout_groups(n):
    lay = n.items[LAYOUT]
    for grp in lay.items if isinstance(lay, Arr) else []:
        ents = grp.items if isinstance(grp, Arr) else [grp]
        yield [b for b in ents if isinstance(b, Obj)]


def ui_job(job):
    rname, path, data_off, frags = job
    C, EX, VALS = collections.Counter(), {}, collections.defaultdict(set)
    for fn, data in load_frags(path, data_off, frags):
        if data[:4] != b'!pkg':
            continue
        root = Pkg(data).root
        for key, role in (('Screens', 'screen_root'), ('Templates', 'template_root')):
            arr = root.get(key) if isinstance(root, Obj) else None
            if not isinstance(arr, Arr):
                continue
            tops = [t for t in arr.items if is_node(t)]
            types = {p: ntype(n) for t in tops for p, n in walk(t)}

            def visit(n, path, parent_t, ctx, scopes, depth):
                t = ntype(n)
                loc = f'{rname}#{fn}:{path}'
                C[('node', t)] += 1
                C[('role', t, role if depth == 0 else 'child')] += 1
                C[('res', t, rname)] += 1
                EX.setdefault(('node', t), loc)
                if parent_t:
                    C[('child', parent_t, t)] += 1
                for k, v in n.items[PROPS].items:
                    if k.value in SKIP_PROPS:
                        continue
                    sh = vshape(v)
                    C[('prop', t, k.value, sh)] += 1
                    EX.setdefault(('prop', t, k.value), loc)
                    if isinstance(v, Obj) and isinstance(v.get('dbentry'), Val):
                        VALS[('db', t, k.value)].add(str(v['dbentry'].value))
                    elif isinstance(v, Obj) and isinstance(v.get('value'), Val) and v['value'].type in SCALARS:
                        VALS[('v', t, k.value)].add(repr(v['value'].value))
                for slot, nkey, hkey, kind in ((COMMANDS, 'commandName', 'command', 'command'), (EVENTS, 'eventName', 'event', 'event')):
                    arr2 = n.items[slot]
                    for e in arr2.items if isinstance(arr2, Arr) else []:
                        if not isinstance(e, Obj):
                            continue
                        nm = e.get(nkey)
                        nm = nm.value if isinstance(nm, Val) else '?'
                        sigs = e.get('signals')
                        sigs = [s for s in sigs.items if isinstance(s, Obj)] if isinstance(sigs, Arr) else []
                        C[(kind, t, nm)] += 1
                        if not sigs:
                            C[(kind + '_nosig', t, nm)] += 1
                        EX.setdefault((kind, t, nm), loc)
                        h = e.get(hkey)
                        if isinstance(h, Val) and h.value != djb2(nm):
                            C[('badhash', kind)] += 1
                        for s in sigs:
                            to = s['to_object_name'].value if isinstance(s.get('to_object_name'), Val) else '?'
                            pin = s['to_pin_name'].value if isinstance(s.get('to_pin_name'), Val) else '?'
                            fp = s.get('from_pin_name')
                            fp = fp.value if isinstance(fp, Val) else None
                            tt = types.get(to, '?')
                            C[('pin_in', tt, pin)] += 1
                            EX.setdefault(('pin_in', tt, pin), f'{loc} -> {to}')
                            C[('pin_out', t, fp)] += 1
                            if fp != nm:
                                C[('pin_out_renamed', t, nm, fp)] += 1
                            C[('wire', t, kind, nm, tt, pin)] += 1
                            C[('link', t, tt, pin)] += 1
                            a = s.get('attributes')
                            C[('pin_attr', tt, pin, ' '.join(sorted(attr_shape(a))) if a is not None else '')] += 1
                            if to == path:
                                C[('self_signal', t, pin)] += 1
                beh = n.items[BEHAVIOURS]
                bs = [fourcc(b['type'].value) for b in beh.items if isinstance(b, Obj) and isinstance(b.get('type'), Val)] if isinstance(beh, Arr) else []
                for b in bs:
                    C[('beh', t, b)] += 1
                C[('bset', t, '+'.join(bs))] += 1
                own = node_scene(n)
                if own is not None:
                    C[('component', t, role if depth == 0 else 'child')] += 1
                my_xref = None
                for grp in layout_groups(n):
                    kinds = ['comp' if b.get('isComponentId') is not None else 'node' if b.get('nodeId') is not None else '?' for b in grp]
                    C[('layout', t, '>'.join(kinds), role if depth == 0 else 'child')] += 1
                    names = [b['debugName'].value for b in grp if isinstance(b.get('debugName'), Val) and b.get('nodeId') is not None]
                    if not names or len(names) != len(grp):
                        continue
                    if depth == 0:
                        C[('layout_el', t, 'outer:' + str(len(names)))] += 1
                        continue
                    chain, last, how = [], 'no-scene', 'ctx'
                    for i, sid in enumerate(([ctx] if ctx is not None else []) + scopes[::-1]):
                        chain, last = resolve(names, sid)
                        if isinstance(last, (int, type(None))):
                            how = 'ctx' if i == 0 and ctx is not None else 'xref-scope'
                            break
                    if isinstance(last, (int, type(None))):
                        ch = '>'.join(chain)
                        C[('layout_el', t, ch)] += 1
                        C[('bound_by', chain[-1], t)] += 1
                        if len(chain) > 1:
                            C[('bound_via', chain[-2], chain[-1], t)] += 1
                        C[('layout_res', how)] += 1
                        EX.setdefault(('layout_el', t, ch), loc)
                        if last is not None:
                            my_xref = last
                    else:
                        C[('layout_res', last)] += 1
                kid_ctx = own if own is not None else ctx
                kids = n.items[CHILDREN]
                for c in kids.items if isinstance(kids, Arr) else []:
                    if is_node(c):
                        visit(c, f'{path}.{name(c)}', t, kid_ctx, scopes + [my_xref] if my_xref is not None else scopes, depth + 1)

            for top in tops:
                C[('rootname', key, name(top), rname)] += 1
                visit(top, name(top), None, node_scene(top), [], 0)
    return rname, C, EX, VALS


def top(counter, n=None):
    return dict(sorted(counter.items(), key=lambda kv: (-kv[1], str(kv[0])))[:n])


def build_catalog(C, EX, VALS, resources, meta):
    T = collections.defaultdict(lambda: collections.defaultdict(lambda: collections.defaultdict(collections.Counter)))
    nodes = {}

    def nt(t):
        if t not in nodes:
            nodes[t] = {'count': 0, 'roles': collections.Counter(), 'resources': 0, 'example': EX.get(('node', t)),
                        'props': {}, 'commands': {}, 'events': {}, 'pins_in': {}, 'pins_out': collections.Counter(),
                        'wiring': collections.Counter(), 'links': collections.Counter(), 'children': collections.Counter(), 'parents': collections.Counter(),
                        'behaviours': collections.Counter(), 'behaviour_sets': collections.Counter(),
                        'layout': collections.Counter(), 'layout_elements': collections.Counter(), 'components': collections.Counter()}
        return nodes[t]

    for k, v in C.items():
        kind = k[0]
        if kind == 'node':
            nt(k[1])['count'] += v
        elif kind == 'role':
            nt(k[1])['roles'][k[2]] += v
        elif kind == 'res':
            nt(k[1])['resources'] += 1
        elif kind == 'child':
            nt(k[1])['children'][k[2]] += v
            nt(k[2])['parents'][k[1]] += v
        elif kind == 'prop':
            p = nt(k[1])['props'].setdefault(k[2], {'count': 0, 'shapes': collections.Counter()})
            p['count'] += v
            p['shapes'][k[3]] += v
        elif kind in ('command', 'event'):
            e = nt(k[1])[kind + 's'].setdefault(k[2], {'count': 0, 'no_signals': 0})
            e['count'] += v
        elif kind in ('command_nosig', 'event_nosig'):
            e = nt(k[1])[kind.split('_')[0] + 's'].setdefault(k[2], {'count': 0, 'no_signals': 0})
            e['no_signals'] += v
        elif kind == 'pin_in':
            p = nt(k[1])['pins_in'].setdefault(k[2], {'count': 0, 'attributes': collections.Counter()})
            p['count'] += v
        elif kind == 'pin_attr':
            p = nt(k[1])['pins_in'].setdefault(k[2], {'count': 0, 'attributes': collections.Counter()})
            if k[3]:
                p['attributes'][k[3]] += v
        elif kind == 'pin_out':
            nt(k[1])['pins_out'][k[2]] += v
        elif kind == 'wire':
            nt(k[1])['wiring'][f'{k[2]} {k[3]} -> {k[4]}.{k[5]}'] += v
        elif kind == 'link':
            nt(k[1])['links'][f'{k[2]}.{k[3]}'] += v
        elif kind == 'beh':
            nt(k[1])['behaviours'][k[2]] += v
        elif kind == 'bset':
            nt(k[1])['behaviour_sets'][k[2]] += v
        elif kind == 'layout':
            nt(k[1])['layout'][f'{k[3]}:{k[2]}'] += v
        elif kind == 'layout_el':
            nt(k[1])['layout_elements'][k[2]] += v
        elif kind == 'component':
            nt(k[1])['components'][k[2]] += v
    for t, d in nodes.items():
        for key, p in d['props'].items():
            p['shapes'] = top(p['shapes'])
            vals = VALS.get(('v', t, key), set())
            dbs = VALS.get(('db', t, key), set())
            if vals:
                p['distinct_values'] = len(vals)
                p['values'] = sorted(vals)[:12]
            if dbs:
                p['bound'] = len(dbs)
                p['dbentries'] = sorted(dbs)[:6]
            p['example'] = EX.get(('prop', t, key))
        for kind in ('commands', 'events'):
            for nm, e in d[kind].items():
                e['example'] = EX.get((kind[:-1], t, nm))
        for pin, p in d['pins_in'].items():
            p['attributes'] = top(p['attributes'])
            p['example'] = EX.get(('pin_in', t, pin))
        for key in ('roles', 'pins_out', 'links', 'children', 'parents', 'behaviours', 'layout', 'layout_elements', 'components'):
            d[key] = top(d[key])
        d['wiring'] = top(d['wiring'], 40)
        d['behaviour_sets'] = top(d['behaviour_sets'], 6)
        for key in ('props', 'commands', 'events', 'pins_in'):
            d[key] = dict(sorted(d[key].items(), key=lambda kv: (-kv[1]['count'], kv[0])))
    els = {}
    for k, v in C.items():
        kind = k[0]
        if not kind.startswith('el') and kind not in ('bound_by', 'bound_via'):
            continue
        t = k[2] if kind == 'bound_via' else k[1]
        d = els.setdefault(t, {'count': 0, 'params': {}, 'parents': collections.Counter(), 'children': collections.Counter(),
                               'geoms': 0, 'anims': 0, 'xref': 0, 'bound_by': collections.Counter(), 'bound_via': collections.Counter(),
                               'example': EX.get(('el', t))})
        if kind == 'el':
            d['count'] += v
        elif kind == 'elparam':
            p = d['params'].setdefault(k[2], {'count': 0, 'types': collections.Counter()})
            p['count'] += v
            p['types'][k[3]] += v
        elif kind == 'elparent':
            d['parents'][k[2]] += v
        elif kind == 'elchild':
            d['children'][k[2]] += v
        elif kind in ('elgeoms', 'elanims', 'elxref'):
            d[kind[2:]] += v
        elif kind == 'bound_by':
            d['bound_by'][k[2]] += v
        elif kind == 'bound_via':
            d['bound_via'][f'{k[1]}>{k[2]} by {k[3]}'] += v
    for t, d in els.items():
        for p in d['params'].values():
            p['types'] = top(p['types'])
        d['params'] = dict(sorted(d['params'].items(), key=lambda kv: (-kv[1]['count'], kv[0])))
        for key in ('parents', 'children', 'bound_by', 'bound_via'):
            d[key] = top(d[key])
    masks = sorted(k[1] for k in C if k[0] == 'maskscene')
    meta['layout_resolution'] = top(collections.Counter({k[1]: v for k, v in C.items() if k[0] == 'layout_res'}))
    meta['bad_hashes'] = top(collections.Counter({k[1]: v for k, v in C.items() if k[0] == 'badhash'}))
    meta['self_signals'] = sum(v for k, v in C.items() if k[0] == 'self_signal')
    return {'meta': meta, 'node_types': dict(sorted(nodes.items())), 'scene_elements': dict(sorted(els.items())),
            'mask_scenes': {m: {'elements': C[('maskscene', m)], 'first': EX.get(('mask', m)), 'archives': resources.get(m, [])[:4]} for m in masks},
            'resources': {k: v[:4] for k, v in sorted(resources.items()) if k.endswith('.ui')},
            'root_names': {kind: {n: sorted({k[3] for k in C if k[0] == 'rootname' and k[1] == kind and k[2] == n})
                                  for n in sorted({k[2] for k in C if k[0] == 'rootname' and k[1] == kind})}
                           for kind in ('Screens', 'Templates')},
            'scene_ids': sorted(f'{s:016x}' for s in SCENES)}


def nodes_main(argv):
    import argparse
    import multiprocessing
    ap = argparse.ArgumentParser(prog='ui_catalog.py nodes')
    ap.add_argument('--stock', action='append', default=[], help='a folder laid out like the game folder holding stock copies')
    ap.add_argument('--baseline', action='append', default=[], help='a swofty baseline.json; archives it lists must match its sha256')
    ap.add_argument('--out', default=NODES_OUT)
    ap.add_argument('-j', type=int, default=os.cpu_count())
    a = ap.parse_args(argv)
    picked, refused = stock_archives(a.stock, a.baseline)
    if refused:
        print('skipped, no stock copy:', ', '.join(refused), file=sys.stderr)
    seen_ui, seen_idf, resources = {}, {}, collections.defaultdict(list)
    for rel, path in sorted(picked.items()):
        meta, res = parse(path)
        short = os.path.relpath(rel, '2025_asset_groups/ui_package')
        for r in res:
            nm, typ = r[0], r[1]
            if typ == 'UIDF' and nm.endswith('.ui'):
                resources[nm].append(short)
                seen_ui.setdefault((nm, r[4]), (nm, path, meta['data_off'], r[3]))
            elif typ == 'UILayout' and nm.endswith('.idf'):
                resources[nm].append(short)
                seen_idf.setdefault(r[4], (nm, path, meta['data_off'], r[3]))
    C, EX, VALS = collections.Counter(), {}, collections.defaultdict(set)
    ctx = multiprocessing.get_context('fork')
    with ctx.Pool(a.j) as pool:
        for rname, index, c, ex in pool.imap(scene_job, sorted(seen_idf.values()), chunksize=8):
            SCENES.update(index)
            C.update(c)
            for k, v in ex.items():
                EX.setdefault(k, v)
    with ctx.Pool(a.j) as pool:
        for rname, c, ex, vals in pool.imap(ui_job, sorted(seen_ui.values()), chunksize=2):
            C.update(c)
            for k, v in ex.items():
                EX.setdefault(k, v)
            for k, v in vals.items():
                VALS[k] |= v
    meta = {'generator': 'python3 tools/ui_catalog.py nodes', 'archives': len(picked), 'skipped_archives': refused,
            'ui_resources': len({k[0] for k in seen_ui}), 'ui_copies': len(seen_ui), 'scenes': len(SCENES), 'scene_copies': len(seen_idf),
            'nodes': sum(v for k, v in C.items() if k[0] == 'node'), 'signals': sum(v for k, v in C.items() if k[0] == 'pin_in'),
            'scene_elements': sum(v for k, v in C.items() if k[0] == 'el')}
    cat = build_catalog(C, EX, VALS, resources, meta)
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    with open(a.out, 'w') as f:
        json.dump(cat, f, indent=1, sort_keys=False, ensure_ascii=False)
        f.write('\n')
    print(json.dumps(meta, indent=1))


ROLES = {
    'UIGP': 'Group: the general node. Holds children, shows and hides, carries textures and colours, forwards commands and events, and is the root of most templates',
    'UILB': 'Label: draws text into an eText element; `text` is a literal or a view-model binding',
    'SVCD': 'Value condition: on `query` it compares the value at `dbEntry` with `value` and fires `yes` or `no`',
    'MVCD': 'Multi-value condition: on `query` it fires the event named after the value at its binding',
    'UIXA': 'Animation player: plays named clips on a scene element and reports their completion',
    'UILE': 'List entry: one item of a UIL2 list, bound to the item view model',
    'UIMO': 'Parameter override: animates a parameter such as Alpha on an element',
    'UITL': 'Tooltip: one button prompt in a tooltip bar',
    'UITT': 'Tooltip bar: the group of button prompts at the bottom of a screen',
    'UIKL': 'Key listener: fires `pressed` for an input action such as Menu Back',
    'UIDS': 'Data setter: writes a value to a view-model or database target',
    'TIME': 'Timer: fires after a delay',
    'UISN': 'Sound: plays an audio event',
    'UIAM': 'Audio theme trigger: plays the screen audio theme cue named by the pin it is signalled on (navigate, confirm, back, tab)',
    'DBVV': 'Database value check',
    'DBRV': 'Database range check',
    'DBCD': 'Database condition',
    'UISL': 'Slider',
    'UIVF': 'Video frame',
    'UIBT': 'Button: a focusable static item',
    'UIAC': 'Screen action: the screen lifecycle node (startTranIn, startTranOut, close_screen)',
    'UISC': 'Screen: the root of every screen in fragment 0',
    'TILE': 'Tile: a focusable item, the root of most spawned tile templates',
    'UISB': 'Scrollbar',
    'F1TS': 'Tile spawner: builds one template instance per entry of the list at `dataContext` into a stacker, and handles focus, selection and navigation inside the row',
    'UIL2': 'List: builds UILE entries from the list at `data`',
    'UILT': 'List (older list type)',
    'UIRU': 'Route trigger: fires a route from routes .uir files',
    'UITC': 'Tab controller: hosts tab templates in a content element and switches between them',
    'UITG': 'Toggle group',
    'UITB': 'Tab bar',
    'SCRL': 'Scroller: drives an eScroller element',
    'DDMC': 'Message container: shows popups from the message queue',
    'UIMC': 'Message: one popup message',
    'UITO': 'Toggle',
    'UIEX': 'Exit: migrates to another flow',
}


def doc_table(rows, cols):
    out = ['| ' + ' | '.join(cols) + ' |', '|' + '---|' * len(cols)]
    for r in rows:
        out.append('| ' + ' | '.join(str(x).replace('|', '\\|') for x in r) + ' |')
    return out


def short(loc):
    if not loc:
        return ''
    res, _, path = loc.partition('#')
    return f"`{res.replace('eaid://ui_package/', '')}` `{path}`"


def nodes_doc(argv):
    cat = json.load(open(NODES_OUT))
    m = cat['meta']
    out = ['---', 'title: UI node reference', 'description: Every node type in the stock .ui files and every element type in the stock .idf scenes, with the properties, commands, events, pins, wiring and layout bindings stock uses, generated from tools/uicatalog/catalog.json.', '---', '',
           '# UI node reference', '',
           f"Generated by `python3 tools/ui_catalog.py nodes-doc` from `tools/uicatalog/catalog.json`, which `python3 tools/ui_catalog.py nodes` builds from {m['ui_resources']} stock `.ui` files ({m['nodes']:,} nodes, {m['signals']:,} signals) and {m['scenes']:,} stock `.idf` scenes ({m['scene_elements']:,} elements) in {m['archives']} UI archives. The NeFS archive inside `F1_25.exe` holds no `.ui` or `.idf` resources.", '',
           'Counts are stock occurrences. A property shape is the value type, with `+dbentry` when the value is bound to a view-model or database path. The one-line roles are read from property, command and pin names and from wiring; they are not taken from engine code. [Patterns](./patterns) explains how the nodes combine, and [Checking UI edits](./patterns#checking-ui-edits) how edits are checked against this data.', '',
           '## Node structure', '',
           'A node is an 8-slot list: properties, children, commands, events, layout attaches, components, behaviours and an unused slot. Commands and events are `{ commandName | eventName, command | event: djb2(name), signals: [...] }`. A signal is `{ to_object, to_object_name, to_pin, to_pin_name, from_pin_name, attributes? }`, where `to_object_name` is the dotted path of the target node from its root. A template root carries its scene in the components slot (`componentId`, `debugName`, `name`); a screen root attaches its scene with a `comp` layout entry.', '',
           '## Node types', '']
    nt = cat['node_types']
    rows = []
    for t, d in sorted(nt.items(), key=lambda kv: -kv[1]['count']):
        if t == '?':
            continue
        rows.append([f'[`{t}`](#{t.lower()})' if d['count'] >= 20 else f'`{t}`', d['count'], d['resources'], ', '.join(f'{k} {v}' for k, v in d['roles'].items()), ROLES.get(t, '')])
    out += doc_table(rows, ['Type', 'Nodes', '.ui files', 'Roles', 'What it is'])
    out.append('')
    for t, d in sorted(nt.items(), key=lambda kv: -kv[1]['count']):
        if t == '?' or d['count'] < 20:
            continue
        out += [f'### {t}', '']
        if t in ROLES:
            out += [ROLES[t] + '.', '']
        out += [f"{d['count']:,} nodes in {d['resources']} files. Example: {short(d['example'])}.", '']
        if d['props']:
            out += ['**Properties**', '']
            out += doc_table([[f'`{k}`', p['count'], ', '.join(f'`{s}`' for s in p['shapes']), ', '.join(p.get('values', [])[:4]) or ', '.join(p.get('dbentries', [])[:3])] for k, p in list(d['props'].items())[:16]], ['Key', 'Count', 'Shapes', 'Sample values or bindings'])
            out.append('')
        for kind in ('commands', 'events'):
            if d[kind]:
                items = list(d[kind].items())
                out += [f"**{kind.title()}** ({len(items)} names): " + ', '.join(f"`{k}` {v['count']}" + (f" ({v['no_signals']} without signals)" if v['no_signals'] else '') for k, v in items[:14]) + (', …' if len(items) > 14 else ''), '']
        if d['pins_in']:
            items = list(d['pins_in'].items())
            out += ['**Pins signals reach on it**: ' + ', '.join(f"`{k}` {v['count']}" + (f" (attributes `{next(iter(v['attributes']))}`)" if v['attributes'] else '') for k, v in items[:14]) + (', …' if len(items) > 14 else ''), '']
        if d['wiring']:
            out += ['**Typical wiring**: ' + '; '.join(f'`{k}` {v}' for k, v in list(d['wiring'].items())[:8]), '']
        out += ['**Parents**: ' + (', '.join(f'{k} {v}' for k, v in list(d['parents'].items())[:6]) or 'none (root)') + '. **Children**: ' + (', '.join(f'{k} {v}' for k, v in list(d['children'].items())[:6]) or 'none') + '.', '']
        if d['layout_elements']:
            out += ['**Binds scene elements**: ' + ', '.join(f'`{k}` {v}' for k, v in list(d['layout_elements'].items())[:6]) + '.', '']
        out += ['**Behaviours**: ' + ', '.join(f'`{k}`' for k in d['behaviours']) + '.', '']
    out += ['## Scene element types', '', 'Element types in the stock scenes, their params, where they sit and which node types bind them.', '']
    els = cat['scene_elements']
    out += doc_table([[f'`{t}`', d['count'], ', '.join(f'{k} {v}' for k, v in list(d['parents'].items())[:4]), ', '.join(f'{k} {v}' for k, v in list(d['children'].items())[:4]), ', '.join(f'{k} {v}' for k, v in list(d['bound_by'].items())[:4])] for t, d in sorted(els.items(), key=lambda kv: -kv[1]['count'])], ['Type', 'Elements', 'Parents', 'Children', 'Bound by'])
    out.append('')
    for t, d in sorted(els.items(), key=lambda kv: -kv[1]['count']):
        specific = [(k, p) for k, p in d['params'].items() if k not in ('className', 'guid', 'pivotRot', 'pivotTrans', 'scaleRot')]
        out += [f'### {t}', '', f"{d['count']:,} elements; {d['geoms']} with geoms, {d['anims']} with anims. Example: `{(d['example'] or '').replace('eaid://ui_package/', '')}`.", '']
        if specific:
            out += ['Params: ' + ', '.join(f"`{k}` {p['count']} ({'/'.join(p['types'])})" for k, p in specific[:18]) + '.', '']
    with open(f'{ROOT}/docs/reference/ui/nodes.md', 'w') as f:
        f.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    if sys.argv[1:2] == ['nodes-doc']:
        nodes_doc(sys.argv[2:])
        sys.exit(0)
    if sys.argv[1:2] == ['nodes']:
        nodes_main(sys.argv[2:])
        sys.exit(0)
    if len(sys.argv) > 1:
        LOGS = sys.argv[1:]
    main()
