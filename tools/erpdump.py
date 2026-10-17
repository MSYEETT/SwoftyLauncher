import sys, os, subprocess, re
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from erp import parse

def fragments(path, pred):
    meta, res = parse(path)
    with open(path, 'rb') as f:
        for name, rtype, unk, frags, h in res:
            if not pred(name, rtype):
                continue
            for i, (fn, off, size, flags, comp, packed) in enumerate(frags):
                f.seek(meta['data_off'] + off)
                raw = f.read(packed)
                if comp in (0x11, 0x91):
                    r = subprocess.run(['zstd', '-dc'], input=raw, capture_output=True)
                    data = r.stdout if r.returncode == 0 else raw
                else:
                    data = raw
                yield name, rtype, i, fn, data

def safe(name):
    return re.sub(r'^eaid://', '', name).replace('/', '__')

if __name__ == '__main__':
    path, out = sys.argv[1], sys.argv[2]
    needles = sys.argv[3:]
    os.makedirs(out, exist_ok=True)
    for name, rtype, i, fn, data in fragments(path, lambda n, t: any(x in n or x == t for x in needles)):
        p = os.path.join(out, f"{safe(name)}.{i}.{fn}")
        open(p, 'wb').write(data)
        print(rtype, len(data), p)
