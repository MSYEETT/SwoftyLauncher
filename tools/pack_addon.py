#!/usr/bin/env python3
"""Package an addon folder as <out>/<id>.swoftyaddon with swofty.json at the zip root.

  pack_addon.py <addon dir> <out dir>

Only swofty.json, LICENSE, README.md, the icon and the files the manifest's operations name go in. Entries are
sorted with a fixed timestamp and mode, so the same inputs give the same bytes.
"""
import json, os, sys, zipfile

OPTIONAL = ('LICENSE', 'README.md')


def wanted(m):
    ops = m.get('operations', {})
    out = ['swofty.json', *OPTIONAL, m.get('icon')]
    for key in ('database', 'erp', 'ui', 'assets'):
        out += ops.get(key, [])
    out += [f['source'] for f in ops.get('files', [])]
    out += [s['file'] for s in ops.get('screens', [])]
    for n in ops.get('native', []):
        out += [n['dll'], *n.get('files', [])]
    return [r.rstrip('/') for r in out if r]


def collect(addon, rels):
    names = set()
    for rel in rels:
        path = os.path.join(addon, rel)
        if os.path.isdir(path):
            for d, dirs, files in os.walk(path):
                dirs[:] = [x for x in dirs if not x.startswith('.')]
                names |= {os.path.relpath(os.path.join(d, f), addon).replace(os.sep, '/')
                          for f in files if not f.startswith('.')}
        elif os.path.isfile(path):
            names.add(rel)
        elif rel not in OPTIONAL:
            sys.exit(f'pack_addon.py: {addon}: {rel} is missing')
    return sorted(names)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    addon, out = sys.argv[1], sys.argv[2]
    m = json.load(open(os.path.join(addon, 'swofty.json')))
    names = collect(addon, wanted(m))
    os.makedirs(out, exist_ok=True)
    pkg = os.path.join(out, f"{m['id']}.swoftyaddon")
    with zipfile.ZipFile(pkg, 'w') as z:
        for name in names:
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            with open(os.path.join(addon, name), 'rb') as f:
                z.writestr(info, f.read())
    print(f"packaged {pkg}: {', '.join(names)}")


if __name__ == '__main__':
    main()
