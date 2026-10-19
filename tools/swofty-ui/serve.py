#!/usr/bin/env python3
"""Swofty UI: a local viewer and editor for F1 25 .idf UI scenes.

    python3 tools/swofty-ui/serve.py            # opens the editor in the browser

Reads the game's UI ERPs read-only and writes exports to the launcher's data folder.
"""
import argparse
import hashlib
import ipaddress
import json
import os
import re
import socket
import subprocess
import sys
import threading
import time
import traceback
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, quote, urlparse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.dirname(HERE)]

import idf as I

import assets as A
import erpsource
import gamedir
import scene as S

APP = 'Swofty UI'
BUNDLE = getattr(sys, '_MEIPASS', HERE)
STATIC = os.path.join(BUNDLE, 'static')
FROZEN = bool(getattr(sys, 'frozen', False))
DEFAULT_PORT = 7341
MAX_BODY = 64 * 1024 * 1024
CONTENT_TYPES = {'.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
                 '.css': 'text/css; charset=utf-8', '.svg': 'image/svg+xml', '.png': 'image/png'}


class DocExpired(Exception):
    pass


class Forbidden(Exception):
    pass


class Docs:
    def __init__(self):
        self.items = {}
        self.lock = threading.Lock()

    def add(self, data, origin):
        doc_id = hashlib.sha1(data).hexdigest()[:16]
        with self.lock:
            self.items[doc_id] = {'data': data, 'origin': origin}
        return doc_id

    def get(self, doc_id):
        with self.lock:
            return self.items.get(doc_id)


class App:
    def __init__(self, catalog, export_dir, game_root=None):
        self.catalog = catalog
        self.export_dir = export_dir
        self.game_root = game_root
        self.docs = Docs()
        self.xrefs = S.XrefResolver(catalog)
        self.assets = A.AssetResolver(catalog) if catalog else None
        self._components = None
        self._components_lock = threading.Lock()

    def scene_for_component(self, cid):
        if self.catalog is None:
            return None
        with self._components_lock:
            if self._components is None:
                self._components = {I.component_id(name): name for name in self.catalog.scenes}
        return self._components.get(cid)

    def identify(self, data, origin):
        if origin.get('eaid') or self.catalog is None:
            return origin
        try:
            doc = I.Idf(data)
        except Exception:
            return origin
        for sc in doc.scenes:
            eaid = self.scene_for_component(sc.name)
            if eaid:
                erps = self.catalog.erps_for(eaid)
                return dict(origin, eaid=eaid, erps=erps, erp=erps[0] if erps else None)
        return origin

    def open_bytes(self, data, origin):
        if data[:4] != b'!pkg':
            raise ValueError('not an .idf (!pkg) file')
        doc = I.Idf(data)
        origin = self.identify(data, origin)
        doc_id = self.docs.add(data, origin)
        return self.model(doc_id, doc, origin)

    def model(self, doc_id, doc, origin, results=None):
        m = S.build_model(doc, self.xrefs, origin.get('erp'), origin.get('eaid'))
        m.update(doc=doc_id, origin=origin, size=len(self.docs.get(doc_id)['data']))
        if results is not None:
            m['results'] = results
        return m

    def replay(self, doc_id, ops):
        d = self.docs.get(doc_id)
        if d is None:
            raise DocExpired('document expired; reopen it')
        doc = I.Idf(d['data'])
        results = S.apply_ops(doc, ops or [])
        return d, doc, results


def safe_stem(name):
    stem = re.sub(r'[^A-Za-z0-9_.-]+', '_', name or 'scene').strip('._') or 'scene'
    return stem.removesuffix('.idf')


def operations_for(origin, stem):
    eaid = origin.get('eaid')
    erps = origin.get('erps') or ([origin['erp']] if origin.get('erp') else [])
    if not eaid or not erps:
        return None
    return {'schema_version': 1,
            'ops': [{'op': 'replace-resource', 'erp': f'{gamedir.UI_PACKAGE_REL}/{rel}',
                     'resource': eaid, 'source': f'ui/{stem}.idf'} for rel in erps]}


def write_text(path, text):
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(text)


def restore_library_path():
    if FROZEN and sys.platform.startswith('linux'):
        orig = os.environ.pop('LD_LIBRARY_PATH_ORIG', None)
        if orig is not None:
            os.environ['LD_LIBRARY_PATH'] = orig
        else:
            os.environ.pop('LD_LIBRARY_PATH', None)


def open_folder(path):
    if sys.platform == 'win32':
        os.startfile(path)
        return
    cmd = 'open' if sys.platform == 'darwin' else 'xdg-open'
    subprocess.Popen([cmd, path], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def is_loopback(host):
    if host == 'localhost':
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


class Handler(BaseHTTPRequestHandler):
    app = None
    allowed_hosts = None
    server_version = 'swofty-ui/1'

    def log_message(self, fmt, *args):
        if os.environ.get('SWOFTY_UI_LOG'):
            sys.stderr.write(f'{self.address_string()} {fmt % args}\n')

    def send(self, code, body, ctype='application/json', headers=None):
        if isinstance(body, (dict, list)):
            body = json.dumps(body, separators=(',', ':')).encode()
        elif isinstance(body, str):
            body = body.encode()
        self.send_response(code)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('X-Content-Type-Options', 'nosniff')
        headers = headers or {}
        if 'Cache-Control' not in headers:
            self.send_header('Cache-Control', 'no-store')
        for k, v in headers.items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)

    def fail(self, code, msg, **extra):
        self.send(code, dict(error=msg, **extra))

    def body(self):
        n = int(self.headers.get('Content-Length') or 0)
        if n > MAX_BODY:
            raise ValueError('request body too large')
        return self.rfile.read(n) if n else b''

    def guard(self, method):
        hosts = self.allowed_hosts
        if hosts is None:
            return
        if (self.headers.get('Host') or '').lower() not in hosts:
            raise Forbidden('unexpected Host header')
        origin = self.headers.get('Origin')
        if method == 'POST' and origin is not None and origin.lower() not in {f'http://{h}' for h in hosts}:
            raise Forbidden('cross-origin request refused')

    def do_GET(self):
        self.route('GET')

    def do_POST(self):
        self.route('POST')

    def route(self, method):
        u = urlparse(self.path)
        q = {k: v[0] for k, v in parse_qs(u.query).items()}
        try:
            self.guard(method)
            if method == 'GET' and not u.path.startswith('/api/'):
                return self.static(u.path)
            fn = getattr(self, 'api_' + u.path[5:].replace('-', '_'), None)
            if fn is None:
                return self.fail(404, 'no such endpoint')
            return fn(method, q)
        except Forbidden as ex:
            self.fail(403, str(ex))
        except S.OpError as ex:
            self.fail(400, f'edit #{ex.index + 1} failed: {ex.message}', opIndex=ex.index)
        except DocExpired as ex:
            self.fail(410, str(ex))
        except erpsource.NoDecoder as ex:
            self.fail(500, str(ex))
        except (KeyError, ValueError) as ex:
            self.fail(400, str(ex).strip("'\""))
        except Exception as ex:
            traceback.print_exc()
            self.fail(500, f'{type(ex).__name__}: {ex}')

    def static(self, path):
        rel = 'index.html' if path in ('', '/') else path.lstrip('/')
        full = os.path.normpath(os.path.join(STATIC, rel))
        if not full.startswith(STATIC + os.sep) or not os.path.isfile(full):
            return self.fail(404, 'not found')
        ext = os.path.splitext(full)[1]
        with open(full, 'rb') as f:
            self.send(200, f.read(), CONTENT_TYPES.get(ext, 'application/octet-stream'))

    def require_post(self, method):
        if method != 'POST':
            raise ValueError('use POST')

    def api_health(self, method, q):
        self.send(200, {'ok': True, 'app': 'swofty-ui'})

    def api_catalog(self, method, q):
        c = self.app.catalog
        if c is None:
            return self.send(200, {'uiDir': None, 'prefix': erpsource.EAID_PREFIX, 'erps': [], 'scenes': [],
                                   'exportDir': self.app.export_dir})
        self.send(200, dict(c.catalog_json(), exportDir=self.app.export_dir))

    def api_open(self, method, q):
        eaid = q.get('eaid', '')
        if not eaid.startswith('eaid://'):
            eaid = erpsource.EAID_PREFIX + eaid.lstrip('/')
        if self.app.catalog is None:
            raise ValueError('no game folder loaded (start Swofty UI with --game)')
        rel, data = self.app.catalog.read_scene(eaid, q.get('erp'))
        origin = {'kind': 'erp', 'eaid': eaid, 'erp': rel,
                  'erps': self.app.catalog.erps_for(eaid),
                  'label': eaid[len(erpsource.EAID_PREFIX):]}
        self.send(200, self.app.open_bytes(data, origin))

    def api_open_file(self, method, q):
        path = os.path.abspath(os.path.expanduser(q.get('path', '')))
        if not os.path.isfile(path):
            raise ValueError(f'no such file: {path}')
        with open(path, 'rb') as f:
            data = f.read(MAX_BODY + 1)
        if len(data) > MAX_BODY:
            raise ValueError('file too large for an .idf')
        origin = {'kind': 'file', 'file': path, 'label': os.path.basename(path)}
        eaid = q.get('eaid')
        if eaid:
            origin['eaid'] = eaid
        self.send(200, self.app.open_bytes(data, origin))

    def api_upload(self, method, q):
        self.require_post(method)
        data = self.body()
        name = q.get('name') or 'upload.idf'
        origin = {'kind': 'upload', 'label': name, 'name': name}
        self.send(200, self.app.open_bytes(data, origin))

    def api_apply(self, method, q):
        self.require_post(method)
        req = json.loads(self.body() or b'{}')
        d, doc, results = self.app.replay(req.get('doc'), req.get('ops'))
        self.send(200, self.app.model(req['doc'], doc, d['origin'], results))

    def api_export(self, method, q):
        self.require_post(method)
        req = json.loads(self.body() or b'{}')
        d, doc, _results = self.app.replay(req.get('doc'), req.get('ops'))
        source = d['data']
        component_ids = [sc.name for sc in I.Idf(source).scenes]
        data = doc.to_bytes()
        checks = S.verify_bytes(data, source, component_ids)
        origin = d['origin']
        stem = safe_stem(req.get('name') or os.path.basename(origin.get('eaid') or origin.get('label') or 'scene'))
        export_dir = self.app.export_dir
        os.makedirs(export_dir, exist_ok=True)
        out = os.path.join(export_dir, stem + '.idf')
        with open(out, 'wb') as f:
            f.write(data)
        operations = operations_for(origin, stem)
        sidecar = os.path.join(export_dir, stem + '.edits.json')
        write_text(sidecar, json.dumps({
            'source': {k: v for k, v in origin.items() if k != 'erps'},
            'sourceErps': origin.get('erps') or [],
            'sourceSha1': hashlib.sha1(source).hexdigest(),
            'exportSha1': hashlib.sha1(data).hexdigest(),
            'exportedAt': time.strftime('%Y-%m-%dT%H:%M:%S%z'),
            'ops': req.get('ops') or []}, indent=1))
        ops_file = None
        if operations:
            ops_file = os.path.join(export_dir, stem + '.erp.json')
            write_text(ops_file, json.dumps(operations, indent=2) + '\n')
        self.send(200, {'file': out, 'sidecar': sidecar, 'operationsFile': ops_file, 'operations': operations,
                        'exportDir': export_dir, 'size': len(data), 'checks': checks,
                        'download': '/api/download?f=' + quote(os.path.basename(out))})

    def api_download(self, method, q):
        name = os.path.basename(q.get('f', ''))
        full = os.path.join(self.app.export_dir, name)
        if not name or not os.path.isfile(full):
            return self.fail(404, 'no such export')
        with open(full, 'rb') as f:
            self.send(200, f.read(), 'application/octet-stream',
                      {'Content-Disposition': f'attachment; filename="{name}"'})

    def api_reveal(self, method, q):
        self.require_post(method)
        os.makedirs(self.app.export_dir, exist_ok=True)
        open_folder(self.app.export_dir)
        self.send(200, {'ok': True, 'path': self.app.export_dir})

    def api_xref(self, method, q):
        self.send(200, S.jsonable(self.app.xrefs.ghosts(q.get('eaid', ''), q.get('erp'), q.get('reveal') == '1')))

    def api_visuals(self, method, q):
        self.require_post(method)
        req = json.loads(self.body() or b'{}')
        out = {}
        if self.app.assets is not None:
            for it in req.get('items', [])[:4000]:
                res, mat, erp = it.get('res'), it.get('mat'), it.get('erp')
                if res:
                    out[f'{res}|{mat}'] = self.app.assets.geom(res, mat, erp)
        self.send(200, {'visuals': out})

    def api_texture(self, method, q):
        if self.app.assets is None:
            return self.fail(404, 'no game folder loaded')
        info, data = self.app.assets.surface_pixels(q.get('name', ''), q.get('erp'))
        if info['format'] not in A.BC7_TYPES:
            return self.fail(415, f"texture format {info['format']} not supported")
        slice_size = ((info['w'] + 3) // 4) * ((info['h'] + 3) // 4) * 16
        self.send(200, data[:slice_size], 'application/octet-stream', {
            'X-Tex-Format': 'bc7', 'X-Tex-Width': str(info['w']), 'X-Tex-Height': str(info['h']),
            'X-Tex-Array': str(info['array']), 'Cache-Control': 'max-age=3600'})

    def api_resources(self, method, q):
        if self.app.catalog is None:
            return self.send(200, {'resources': []})
        erp, prefix = q.get('erp'), q.get('prefix', '')
        if not erp or not prefix:
            raise ValueError('need erp and prefix')
        self.send(200, {'erp': erp, 'resources': self.app.catalog.resources_with_prefix(erp, prefix)})

    def api_resource(self, method, q):
        if self.app.catalog is None:
            return self.fail(404, 'no game folder loaded')
        data = self.app.catalog.read_fragment(q['erp'], q['name'], int(q.get('frag', 0)))
        self.send(200, data, 'application/octet-stream')


def free_port(host, port):
    for p in [port] + list(range(port + 1, port + 40)):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            if sys.platform == 'win32':
                s.setsockopt(socket.SOL_SOCKET, socket.SO_EXCLUSIVEADDRUSE, 1)
            else:
                s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                s.bind((host, p))
                return p
            except OSError:
                continue
    return 0


class Server(ThreadingHTTPServer):
    allow_reuse_address = sys.platform != 'win32'
    daemon_threads = True


def make_server(app, host, port):
    Handler.app = app
    httpd = Server((host, free_port(host, port)), Handler)
    bound = httpd.server_address[1]
    if is_loopback(host):
        Handler.allowed_hosts = {f'{h}:{bound}' for h in ('127.0.0.1', 'localhost', '[::1]', host)}
    else:
        Handler.allowed_hosts = None
    return httpd


def pause():
    if FROZEN and sys.stdin is not None and sys.stdin.isatty():
        try:
            input('Press Enter to close this window.')
        except (EOFError, KeyboardInterrupt):
            pass


def fail(msg):
    print(f'\n{APP} could not start.\n\n{msg}\n', file=sys.stderr, flush=True)
    pause()
    return 1


def game_not_found(ex):
    if FROZEN:
        where = 'In a terminal, from the extracted launcher folder, run:'
        exe = r'bin\swofty-ui.exe' if sys.platform == 'win32' else 'bin/swofty-ui'
    else:
        where = 'From the repository, run:'
        exe = 'python3 tools/swofty-ui/serve.py'
    example = (r'"D:\SteamLibrary\steamapps\common\F1 25"' if sys.platform == 'win32'
               else '"$HOME/.local/share/Steam/steamapps/common/F1 25"')
    lines = [f'{ex}.']
    if ex.tried:
        lines.append('Looked in:')
        lines += [f'  {p}' for p in ex.tried]
    lines += ['',
              'Pass the F1 25 folder, the one with F1_25.exe and 2025_asset_groups, with --game.',
              where,
              f'  {exe} --game {example}',
              '',
              'Swofty UI also uses the folder you picked with Locate game in Swofty Launcher, or the F1_25_DIR',
              'environment variable.',
              f'To open .idf files from disk without the game, run: {exe} --no-game']
    return '\n'.join(lines)


def parse_args(argv):
    ap = argparse.ArgumentParser(prog='swofty-ui', description=f'{APP}: view and edit F1 25 .idf UI scenes.')
    ap.add_argument('--game', help='the F1 25 folder, a pristine copy of it, or its 2025_asset_groups/ui_package '
                                   'folder; found automatically when left out. Read-only.')
    ap.add_argument('--no-game', action='store_true', help='start without the game: open .idf files only')
    ap.add_argument('--host', default='127.0.0.1')
    ap.add_argument('--port', type=int, default=DEFAULT_PORT, help='first port to try (default 7341)')
    ap.add_argument('--export-dir', help='where exports go (default: the launcher data folder, swofty-ui/exports)')
    ap.add_argument('--open', help='scene to open first: an eaid path or a local .idf file')
    ap.add_argument('--no-browser', action='store_true', help='do not open the browser')
    ap.add_argument('--self-test', action='store_true', help=argparse.SUPPRESS)
    return ap.parse_args(argv)


def main(argv=None):
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(errors='replace')
        sys.stderr.reconfigure(errors='replace')
    restore_library_path()
    args = parse_args(argv)
    if args.self_test:
        import selftest
        return selftest.run()

    print(APP, flush=True)
    catalog = None
    game_root = None
    if not args.no_game:
        try:
            ui_dir, how = gamedir.resolve(args.game)
        except gamedir.NotFound as ex:
            return fail(game_not_found(ex))
        if erpsource.decoder() is None:
            return fail('Reading the game archives needs zstd: run Python 3.14 or newer, '
                        'or install the zstandard module or the zstd command.')
        game_root = gamedir.game_root_of(ui_dir)
        t = time.time()
        print(f'Reading the UI archives in {ui_dir} (found via {how})', flush=True)
        catalog = erpsource.ErpCatalog(ui_dir).build()
        if not catalog.scenes:
            return fail(f'No .idf scenes were found in {ui_dir}. Pass the F1 25 folder with --game.')
        print(f'Indexed {len(catalog.scenes)} scenes in {len(catalog.erps)} archives ({time.time() - t:.1f}s)',
              flush=True)

    export_dir = os.path.abspath(os.path.expanduser(args.export_dir or gamedir.default_export_dir()))
    if game_root and gamedir.is_inside(export_dir, game_root):
        return fail(f'The export folder {export_dir} is inside the game folder. '
                    f'{APP} never writes to the game folder; pick another --export-dir.')

    try:
        httpd = make_server(App(catalog, export_dir, game_root), args.host, args.port)
    except OSError as ex:
        return fail(f'Could not listen on {args.host}: {ex}')
    shown_host = '127.0.0.1' if args.host in ('0.0.0.0', '') else args.host
    base = f'http://{shown_host}:{httpd.server_address[1]}/'
    url = base
    if args.open:
        if os.path.isfile(args.open):
            url += '#file=' + quote(os.path.abspath(args.open))
        else:
            url += '#scene=' + quote(args.open.replace(erpsource.EAID_PREFIX, ''))
    print(f'Exports go to {export_dir}')
    print(f'\n{APP} is running at {base} \u2014 close this window or press Ctrl+C to stop\n', flush=True)
    if not args.no_browser:
        threading.Thread(target=open_browser, args=(url,), daemon=True).start()
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        httpd.server_close()
    return 0


def open_browser(url):
    try:
        ok = webbrowser.open(url)
    except Exception:
        ok = False
    if not ok:
        print(f'Open {url} in your browser.', flush=True)


def entry():
    try:
        code = main()
    except SystemExit:
        raise
    except KeyboardInterrupt:
        code = 130
    except BaseException:
        traceback.print_exc()
        pause()
        code = 1
    sys.exit(code)


if __name__ == '__main__':
    entry()
