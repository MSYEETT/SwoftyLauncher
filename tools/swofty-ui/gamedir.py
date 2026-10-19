import json
import os
import sys

STEAM_APP_ID = 3059520
GAME_EXE = 'F1_25.exe'
UI_PACKAGE = ('2025_asset_groups', 'ui_package')
UI_PACKAGE_REL = '/'.join(UI_PACKAGE)


class NotFound(Exception):
    def __init__(self, message, tried):
        super().__init__(message)
        self.tried = tried


def parse_vdf(text):
    tokens = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
        elif c == '/' and text.startswith('//', i):
            j = text.find('\n', i)
            i = n if j < 0 else j + 1
        elif c in '{}':
            tokens.append(c)
            i += 1
        elif c == '"':
            i += 1
            out = []
            while i < n and text[i] != '"':
                if text[i] == '\\' and i + 1 < n:
                    out.append({'n': '\n', 't': '\t'}.get(text[i + 1], text[i + 1]))
                    i += 2
                else:
                    out.append(text[i])
                    i += 1
            tokens.append(('s', ''.join(out)))
            i += 1
        else:
            j = i
            while j < n and not text[j].isspace() and text[j] not in '{}"':
                j += 1
            tokens.append(('s', text[i:j]))
            i = j
    pos = 0

    def block():
        nonlocal pos
        out = {}
        while pos < len(tokens):
            t = tokens[pos]
            if t == '}':
                pos += 1
                return out
            if t == '{':
                pos += 1
                continue
            key = t[1]
            pos += 1
            if pos >= len(tokens):
                break
            v = tokens[pos]
            if v == '{':
                pos += 1
                out[key] = block()
            elif v == '}':
                break
            else:
                out[key] = v[1]
                pos += 1
        return out

    return block()


def read_vdf(path):
    try:
        with open(path, encoding='utf-8', errors='replace') as f:
            return parse_vdf(f.read())
    except OSError:
        return None


def _real(p):
    return os.path.normcase(os.path.realpath(p))


def _registry_steam_paths():
    if sys.platform != 'win32':
        return []
    try:
        import winreg
    except ImportError:
        return []
    out = []
    for hive, key, value in ((winreg.HKEY_CURRENT_USER, r'Software\Valve\Steam', 'SteamPath'),
                             (winreg.HKEY_LOCAL_MACHINE, r'SOFTWARE\WOW6432Node\Valve\Steam', 'InstallPath'),
                             (winreg.HKEY_LOCAL_MACHINE, r'SOFTWARE\Valve\Steam', 'InstallPath')):
        try:
            with winreg.OpenKey(hive, key) as k:
                out.append(os.path.normpath(winreg.QueryValueEx(k, value)[0]))
        except OSError:
            continue
    return out


def steam_roots():
    out = []
    if os.environ.get('SWOFTY_STEAM_ROOT'):
        out.append(os.environ['SWOFTY_STEAM_ROOT'])
    if sys.platform == 'win32':
        for var in ('ProgramFiles(x86)', 'ProgramFiles'):
            if os.environ.get(var):
                out.append(os.path.join(os.environ[var], 'Steam'))
        out += _registry_steam_paths()
    else:
        home = os.path.expanduser('~')
        out += [os.path.join(home, '.steam', 'steam'),
                os.path.join(home, '.local', 'share', 'Steam'),
                os.path.join(home, '.var', 'app', 'com.valvesoftware.Steam', '.local', 'share', 'Steam'),
                os.path.join(home, '.var', 'app', 'com.valvesoftware.Steam', 'data', 'Steam')]
    seen = set()
    keep = []
    for p in out:
        real = _real(p)
        if os.path.isdir(os.path.join(p, 'steamapps')) and real not in seen:
            keep.append(p)
        seen.add(real)
    return keep


def libraries(steam_root):
    libs = [steam_root]
    v = read_vdf(os.path.join(steam_root, 'steamapps', 'libraryfolders.vdf')) or {}
    entries = v.get('libraryfolders')
    if isinstance(entries, dict):
        for e in entries.values():
            p = e.get('path') if isinstance(e, dict) else None
            if p and not any(_real(lib) == _real(p) for lib in libs):
                libs.append(p)
    return libs


def find_steam_game():
    for root in steam_roots():
        for lib in libraries(root):
            steamapps = os.path.join(lib, 'steamapps')
            acf = read_vdf(os.path.join(steamapps, f'appmanifest_{STEAM_APP_ID}.acf')) or {}
            state = acf.get('AppState')
            installdir = state.get('installdir') if isinstance(state, dict) else None
            if isinstance(installdir, str) and installdir:
                return os.path.join(steamapps, 'common', installdir)
    return None


def launcher_data_dir():
    if os.environ.get('SWOFTY_DATA_DIR'):
        return os.environ['SWOFTY_DATA_DIR']
    if sys.platform == 'win32':
        return os.path.join(os.environ.get('LOCALAPPDATA') or '.', 'Swofty')
    home = os.path.expanduser('~')
    flatpak = os.path.join(home, '.var', 'app', 'com.valvesoftware.Steam', 'data')
    if os.path.isdir(flatpak) and not os.path.exists(os.path.join(home, '.local', 'share', 'Steam')):
        return os.path.join(flatpak, 'swofty')
    base = os.environ.get('XDG_DATA_HOME') or os.path.join(home, '.local', 'share')
    return os.path.join(base, 'swofty')


def launcher_game_dir():
    try:
        with open(os.path.join(launcher_data_dir(), 'config.json'), encoding='utf-8') as f:
            d = json.load(f).get('game_dir')
    except (OSError, ValueError, AttributeError):
        return None
    return d if isinstance(d, str) and d else None


def default_export_dir():
    return os.path.join(launcher_data_dir(), 'swofty-ui', 'exports')


def ui_package_of(path):
    if not path or not os.path.isdir(path):
        return None
    cand = os.path.join(path, *UI_PACKAGE)
    if os.path.isdir(cand) and any(n.endswith('.erp') for n in os.listdir(cand)):
        return cand
    if any(n.endswith('.erp') for n in os.listdir(path)):
        return path
    return None


def game_root_of(ui_dir):
    parent = os.path.dirname(os.path.dirname(os.path.abspath(ui_dir)))
    if os.path.normcase(os.path.abspath(ui_dir)) == os.path.normcase(os.path.join(parent, *UI_PACKAGE)):
        return parent
    return os.path.abspath(ui_dir)


def resolve(explicit=None):
    tried = []
    if explicit:
        ui = ui_package_of(os.path.expanduser(explicit))
        if ui:
            return ui, '--game'
        raise NotFound(f'--game "{explicit}" has no {UI_PACKAGE_REL} folder with .erp archives', [explicit])
    sources = [('F1_25_DIR', lambda: os.environ.get('F1_25_DIR')),
               ('the launcher setting', launcher_game_dir),
               ('Steam', find_steam_game)]
    for how, get in sources:
        path = get()
        if not path:
            continue
        tried.append(path)
        ui = ui_package_of(path)
        if ui:
            return ui, how
    raise NotFound('F1 25 was not found', tried)


def is_inside(child, parent):
    c, p = _real(child), _real(parent)
    try:
        return os.path.commonpath([c, p]) == p
    except ValueError:
        return False
