#!/usr/bin/env python3
"""Package an addon folder as <out>/<id>.swoftyaddon with swofty.json at the zip root.

  pack_addon.py [--zip] <addon dir> <out dir>

--zip writes <out>/<id>.zip instead; the launcher installs either.

Only swofty.json, LICENSE, README.md, the icon, the launcher plugin's programs and the files the manifest's
operations name go in, plus every
file an operation file names in a "source" or "sources" field (texture fragments, VTFs, UI files). Entries are
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
    program = m.get('launcher', {}).get('program')
    out += list(program.values()) if isinstance(program, dict) else [program]
    return [r.rstrip('/') for r in out if r]


def sources(addon, rel):
    out = []

    def walk(o):
        if isinstance(o, dict):
            for k, v in o.items():
                refs = [v] if k == 'source' and isinstance(v, str) else v if k == 'sources' and isinstance(v, list) else None
                if refs is None:
                    walk(v)
                    continue
                out.extend(r for r in refs if isinstance(r, str) and os.path.isfile(os.path.join(addon, r)))
        elif isinstance(o, list):
            for v in o:
                walk(v)

    path = os.path.join(addon, rel)
    if rel.endswith('.json') and os.path.isfile(path):
        walk(json.load(open(path)))
    return out


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
    args = sys.argv[1:]
    ext = 'zip' if '--zip' in args else 'swoftyaddon'
    args = [a for a in args if a != '--zip']
    if len(args) != 2:
        sys.exit(__doc__)
    addon, out = args
    m = json.load(open(os.path.join(addon, 'swofty.json')))
    rels = wanted(m)
    ops = m.get('operations', {})
    for key in ('database', 'erp', 'ui', 'assets'):
        for rel in ops.get(key, []):
            rels += sources(addon, rel)
    names = collect(addon, rels)
    os.makedirs(out, exist_ok=True)
    pkg = os.path.join(out, f"{m['id']}.{ext}")
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
