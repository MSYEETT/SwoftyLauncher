import hashlib
import json
import os
import shutil
import struct
import sys
import tempfile
import threading
import urllib.error
import urllib.request

import idf as I
import pkg as P
from pkg import Arr, Obj, Val

import erpsource
import gamedir
import serve

SCENE = 'eaid://ui_package/art/screens/selftest/grid.idf'
ARCHIVES = ('flow_selftest.erp', 'screens/selftest.erp')


class Failed(Exception):
    pass


def check(cond, what):
    if not cond:
        raise Failed(what)


def element(name, type_name, path, pos=(0.0, 0.0, 0.0), children=(), extents=None):
    params = [Obj([(Val('stri', 'value'), Val('shnm', type_name)), (Val('stri', 'name'), Val('stri', 'className'))]),
              Obj([(Val('stri', 'value'), Val('stri', f'00000000-0000-4000-8000-{I.djb2(name):012x}')),
                   (Val('stri', 'name'), Val('stri', 'guid'))])]
    if extents:
        lo, hi = extents
        params.append(Obj([(Val('stri', 'value'), Val('vec3', [*lo, 0.0])), (Val('stri', 'name'), Val('stri', 'minExtents'))]))
        params.append(Obj([(Val('stri', 'value'), Val('vec3', [*hi, 0.0])), (Val('stri', 'name'), Val('stri', 'maxExtents'))]))
        params.append(Obj([(Val('stri', 'value'), Val('fp32', 0.5)), (Val('stri', 'name'), Val('stri', 'gap'))]))
    return Obj([
        (Val('stri', 'name'), Val('stri', name)),
        (Val('stri', 'elementId'), Val('ui32', I.djb2(path))),
        (Val('stri', 'typeName'), Val('shnm', type_name)),
        (Val('stri', 'position'), Val('vec3', [*pos, 0.0])),
        (Val('stri', 'scale'), Val('vec3', [1.0, 1.0, 1.0, 0.0])),
        (Val('stri', 'rotation'), Val('quat', [0.0, 0.0, 0.0, 1.0])),
        (Val('stri', 'params'), Arr(params)),
        (Val('stri', 'geoms'), Arr([])),
        (Val('stri', 'anims'), Arr([])),
        (Val('stri', 'children'), Arr(list(children))),
    ])


def scene_bytes(eaid=SCENE):
    box = ((-1.0, -0.5, 0.0), (1.0, 0.5, 0.0))
    tiles = [element(f'tile_{i}', 'eNull', f'panel.tile_{i}', (2.5 * (i - 1), 0.0, 0.0), extents=box)
             for i in (1, 2)]
    label = element('label', 'eText', 'panel.label', (0.0, 1.0, 0.0))
    panel = element('panel', 'eNull', 'panel', (-1.0, 0.0, 0.0), tiles + [label])
    root = element('Scene Root', 'eScene', 'Scene Root', children=[panel])
    names = ['Scene Root', 'panel', 'tile_1', 'tile_2', 'label']
    entry = Obj([(Val('stri', 'scene'), root),
                 (Val('stri', 'name'), Val('ui64', I.component_id(eaid))),
                 (Val('stri', 'nodeidlookup'), Arr([Val('ui32', h) for h in sorted({I.djb2(n) for n in names})]))])
    pk = P.Pkg()
    pk.root = Obj([(Val('stri', 'scenelist'), Arr([entry]))])
    return pk.to_bytes()


def write_erp(path, resources):
    try:
        from compression import zstd
    except ImportError:
        zstd = None
    entries = bytearray()
    blob = bytearray()
    nfrag = 0
    for name, kind, data in resources:
        packed = zstd.compress(data) if zstd else data
        comp = erpsource.ZSTD if zstd else erpsource.STORED
        nb = name.encode() + b'\0'
        body = (struct.pack('<H', len(nb)) + nb + kind.encode().ljust(16, b'\0') + struct.pack('<I', 1) + b'\x01'
                + b'temp' + struct.pack('<QQI', len(blob), len(data), 16) + bytes([comp])
                + struct.pack('<Q', len(packed)) + hashlib.md5(data).digest())
        entries += struct.pack('<I', len(body)) + body
        blob += packed
        nfrag += 1
    info_size = 8 + len(entries)
    data_off = 0x30 + info_size
    head = b'ERPK' + struct.pack('<I', 3) + b'\0' * 8 + struct.pack('<QQQ', 0x30, info_size, data_off) + b'\0' * 8
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'wb') as f:
        f.write(head + struct.pack('<II', len(resources), nfrag) + entries + blob)


class Client:
    def __init__(self, port):
        self.base = f'http://127.0.0.1:{port}'

    def call(self, path, body=None, headers=None, raw=False):
        data = None
        hdrs = dict(headers or {})
        if body is not None:
            data = body if isinstance(body, bytes) else json.dumps(body).encode()
            hdrs.setdefault('Content-Type', 'application/json')
            hdrs.setdefault('Origin', self.base)
        req = urllib.request.Request(self.base + path, data=data, headers=hdrs, method='POST' if data is not None else 'GET')
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                payload = r.read()
                return r.status, payload if raw else json.loads(payload)
        except urllib.error.HTTPError as ex:
            payload = ex.read()
            try:
                return ex.code, json.loads(payload)
            except ValueError:
                return ex.code, payload


def test_discovery(tmp):
    lib = os.path.join(tmp, 'library')
    steam = os.path.join(tmp, 'steam')
    game = os.path.join(lib, 'steamapps', 'common', 'F1 25')
    os.makedirs(os.path.join(steam, 'steamapps'))
    with open(os.path.join(steam, 'steamapps', 'libraryfolders.vdf'), 'w') as f:
        f.write('"libraryfolders"\n{\n\t"0"\n\t{\n\t\t"path"\t\t"%s"\n\t}\n\t"1"\n\t{\n\t\t"path"\t\t"%s"\n\t}\n}\n'
                % (steam.replace('\\', '\\\\'), lib.replace('\\', '\\\\')))
    os.makedirs(os.path.join(lib, 'steamapps'))
    with open(os.path.join(lib, 'steamapps', f'appmanifest_{gamedir.STEAM_APP_ID}.acf'), 'w') as f:
        f.write('"AppState"\n{\n\t"appid"\t\t"3059520"\n\t"installdir"\t\t"F1 25"\n\t"buildid"\t\t"1"\n}\n')
    ui = os.path.join(game, *gamedir.UI_PACKAGE)
    data = scene_bytes()
    for rel in ARCHIVES:
        write_erp(os.path.join(ui, *rel.split('/')), [(SCENE, 'UILayout', data)])

    saved = {k: os.environ.get(k) for k in ('SWOFTY_STEAM_ROOT', 'SWOFTY_DATA_DIR', 'F1_25_DIR')}
    roots = gamedir.steam_roots
    try:
        os.environ.pop('F1_25_DIR', None)
        os.environ['SWOFTY_DATA_DIR'] = os.path.join(tmp, 'data')
        os.environ['SWOFTY_STEAM_ROOT'] = steam
        found, how = gamedir.resolve()
        check(os.path.samefile(found, ui) and how == 'Steam', f'Steam discovery found {found} via {how}')
        check(os.path.samefile(gamedir.game_root_of(found), game), 'game root of the ui_package folder')
        os.makedirs(os.environ['SWOFTY_DATA_DIR'])
        with open(os.path.join(os.environ['SWOFTY_DATA_DIR'], 'config.json'), 'w') as f:
            json.dump({'game_dir': game}, f)
        found, how = gamedir.resolve()
        check(how == 'the launcher setting', f'launcher setting discovery used {how}')
        found, how = gamedir.resolve(ui)
        check(os.path.samefile(found, ui) and how == '--game', '--game accepts the ui_package folder')
        try:
            gamedir.resolve(os.path.join(tmp, 'nowhere'))
            check(False, '--game with a wrong folder must fail')
        except gamedir.NotFound as ex:
            check('--game' in serve.game_not_found(ex), 'the not-found message explains --game')
        os.remove(os.path.join(os.environ['SWOFTY_DATA_DIR'], 'config.json'))
        gamedir.steam_roots = lambda: []
        try:
            gamedir.resolve()
            check(False, 'discovery without Steam must fail')
        except gamedir.NotFound as ex:
            msg = serve.game_not_found(ex)
            check('--game' in msg and '--no-game' in msg, 'the not-found message lists --game and --no-game')
    finally:
        gamedir.steam_roots = roots
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    check(gamedir.is_inside(os.path.join(game, 'exports'), game), 'an export folder in the game folder is caught')
    check(not gamedir.is_inside(os.path.join(tmp, 'exports'), game), 'an export folder outside the game is allowed')
    return game, ui, data


def test_server(tmp, game, ui, source):
    catalog = erpsource.ErpCatalog(ui).build()
    check(sorted(catalog.erps) == sorted(ARCHIVES), f'catalog archives {catalog.erps}')
    check(list(catalog.scenes) == [SCENE], 'catalog scenes')
    export_dir = os.path.join(tmp, 'exports')
    httpd = serve.make_server(serve.App(catalog, export_dir, game), '127.0.0.1', 0)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    try:
        c = Client(httpd.server_address[1])
        code, page = c.call('/', raw=True)
        check(code == 200 and b'<title>Swofty UI</title>' in page, 'index.html is served and branded')
        for f in ('app.js', 'bc7.js', 'app.css', 'icon-32.png', 'icon-64.png'):
            code, body = c.call('/' + f, raw=True)
            check(code == 200 and len(body) > 100, f'static {f}')
        code, _ = c.call('/api/health', headers={'Host': 'attacker.example:80'})
        check(code == 403, 'a foreign Host header is refused')
        code, _ = c.call('/api/export', {}, headers={'Origin': 'http://attacker.example'})
        check(code == 403, 'a cross-origin POST is refused')

        code, cat = c.call('/api/catalog')
        check(code == 200 and [s[0] for s in cat['scenes']] == ['art/screens/selftest/grid.idf'], 'GET /api/catalog')
        code, m = c.call('/api/open?eaid=art/screens/selftest/grid.idf')
        check(code == 200, f'GET /api/open: {m}')
        check(m['stats']['elements'] == 5 and m['stats']['idBad'] == 0, 'scene model ids')
        check(m['scenes'][0]['componentIdMatchesPath'] and m['scenes'][0]['lookupOk'], 'componentId and lookup')
        doc = m['doc']
        tile1 = next(k for k, e in m['elements'].items() if e['name'] == 'tile_1')
        label = next(k for k, e in m['elements'].items() if e['name'] == 'label')
        ops = [{'op': 'set', 'key': tile1, 'field': 'position', 'value': [0.25, -1.0, 0.0]},
               {'op': 'param', 'key': tile1, 'name': 'gap', 'value': 0.75},
               {'op': 'clone', 'key': tile1, 'name': 'tile_3', 'position': [5.0, 0.0, 0.0]},
               {'op': 'rename', 'key': label, 'name': 'caption'}]
        code, applied = c.call('/api/apply', {'doc': doc, 'ops': ops})
        check(code == 200 and applied['stats']['elements'] == 6, f'POST /api/apply: {applied}')
        check(applied['scenes'][0]['lookupOk'] and applied['stats']['idBad'] == 0, 'ids after edits')
        clone_key = applied['results'][2]['key']
        ops.append({'op': 'delete', 'key': clone_key})
        code, bad = c.call('/api/apply', {'doc': doc, 'ops': ops + [{'op': 'rename', 'key': label, 'name': 'a.b'}]})
        check(code == 400 and bad.get('opIndex') == 5, 'a bad edit reports its index')
        ops.pop()

        code, r = c.call('/api/export', {'doc': doc, 'ops': ops[:4]})
        check(code == 200, f'POST /api/export: {r}')
        ch = r['checks']
        check(ch['ok'] and ch['roundtrip'] and ch['componentIdsKept'] and not ch['identicalToSource'], f'checks {ch}')
        check(os.path.dirname(r['file']) == export_dir and os.path.isfile(r['sidecar']), 'export files')
        stanzas = r['operations']['ops']
        check(sorted(s['erp'] for s in stanzas) == sorted(f'2025_asset_groups/ui_package/{a}' for a in ARCHIVES),
              f'operations stanzas {stanzas}')
        check(all(s['op'] == 'replace-resource' and s['resource'] == SCENE and s['source'] == 'ui/grid.idf'
                  for s in stanzas), 'stanza fields')
        with open(r['operationsFile'], encoding='utf-8') as f:
            check(json.load(f) == r['operations'], 'the .erp.json file holds the stanzas')
        with open(r['file'], 'rb') as f:
            exported = f.read()
        out = I.Idf(exported)
        names = [e.name for e, _ in out.scenes[0].root.walk()]
        check('tile_3' in names and 'caption' in names and 'label' not in names, f'exported names {names}')
        check(out.scenes[0].nodeidlookup == sorted({I.djb2(n) for n in names}), 'exported nodeidlookup')
        check(all(e.id_ok() for e, _ in out.scenes[0].root.walk()), 'exported elementIds')
        with open(r['sidecar'], encoding='utf-8') as f:
            side = json.load(f)
        check(side['ops'] == ops[:4] and side['sourceSha1'] == hashlib.sha1(source).hexdigest(), 'edit script')
        code, dl = c.call(r['download'], raw=True)
        check(code == 200 and dl == exported, 'download')

        code, again = c.call('/api/export', {'doc': doc, 'ops': ops[:4], 'name': 'grid_again'})
        with open(again['file'], 'rb') as f:
            check(code == 200 and f.read() == exported, 'replaying the same edits gives the same bytes')
        code, same = c.call('/api/export', {'doc': doc, 'ops': [], 'name': 'grid_same'})
        check(code == 200 and same['checks']['identicalToSource'], 'an export without edits is identical')
        code, up = c.call('/api/upload?name=grid.idf', exported, headers={'Content-Type': 'application/octet-stream'})
        check(code == 200 and up['origin'].get('eaid') == SCENE, 'an uploaded export is matched to its scene')
        code, vis = c.call('/api/visuals', {'items': []})
        check(code == 200 and vis == {'visuals': {}}, 'POST /api/visuals')
        code, _ = c.call('/api/resource?erp=../../x.erp&name=a')
        check(code == 400, 'a path outside the UI archives is refused')
    finally:
        httpd.shutdown()
        httpd.server_close()


def run():
    print('Swofty UI self-test', flush=True)
    check_decoder = erpsource.decoder()
    tmp = tempfile.mkdtemp(prefix='swofty-ui-selftest-')
    try:
        check(check_decoder is not None, 'a zstd decoder is available')
        check(not serve.FROZEN or check_decoder == 'compression.zstd', 'the bundle carries compression.zstd')
        check(os.path.isfile(os.path.join(serve.STATIC, 'index.html')), f'static files at {serve.STATIC}')
        data = scene_bytes()
        check(I.Idf(data).to_bytes() == data, 'a synthetic scene round-trips')
        game, ui, source = test_discovery(tmp)
        test_server(tmp, game, ui, source)
    except Failed as ex:
        print(f'self-test FAILED: {ex}', file=sys.stderr, flush=True)
        return 1
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    print(f'self-test passed (decoder: {check_decoder}, python {sys.version.split()[0]})', flush=True)
    return 0
