import sys, subprocess
from erp import parse
def extract(path, needle):
    meta, res = parse(path)
    f = open(path, 'rb')
    for name, rtype, unk, frags, h in res:
        if needle in name:
            for (fn, off, size, flags, comp, packed) in frags:
                f.seek(meta['data_off'] + off)
                raw = f.read(packed)
                yield name, comp, raw, size
if __name__ == '__main__':
    for name, comp, raw, size in extract(sys.argv[1], sys.argv[2]):
        print(name, comp, len(raw), size, raw[:8].hex(), file=sys.stderr)
        r = subprocess.run(['zstd', '-dc'], input=raw, capture_output=True)
        data = r.stdout if r.returncode == 0 else raw
        open(sys.argv[3], 'wb').write(data)
        print('wrote', sys.argv[3], len(data), 'zstd ok' if r.returncode == 0 else 'raw', file=sys.stderr)
