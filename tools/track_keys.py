import argparse, hashlib, json, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gamedb

BASE = 0x140000000

BUILDS = {
    'd873a1c8d2044cef2fd76c99c9d5a97649c5b815d055540995105f46619865ae': {
        'api': [0x1445a22f0, 0x1445a25c0, 0x1445a28f0, 0x1445a2bd0, 0x1445a2cf0, 0x1445a3900],
        'readers': [0x142e4c640, 0x142e58f20, 0x142e3ff00, 0x142e44b50, 0x142ed4870, 0x1413e4cf0],
    },
}

REGISTRY = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'packages', 'core', 'src', 'activation',
                        'track_tables.json')


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for b in iter(lambda: f.read(1 << 20), b''):
            h.update(b)
    return h.hexdigest()


class Exe:
    def __init__(self, path):
        import numpy, pefile
        self.np = numpy
        pe = pefile.PE(path, fast_load=True)
        self.data = numpy.memmap(path, dtype=numpy.uint8, mode='r')
        self.secs = [(s.VirtualAddress, max(s.Misc_VirtualSize, s.SizeOfRawData), s.PointerToRawData, s.SizeOfRawData,
                      bool(s.Characteristics & 0x20000000)) for s in pe.sections]
        d = pe.OPTIONAL_HEADER.DATA_DIRECTORY[3]
        o = self.off(d.VirtualAddress)
        t = numpy.frombuffer(bytes(self.data[o:o + d.Size - d.Size % 12]), dtype=numpy.uint32).reshape(-1, 3)
        t = t[t[:, 0] != 0]
        self.funcs = t[numpy.argsort(t[:, 0])]

    def off(self, rva):
        for va, vs, ro, rs, _ in self.secs:
            if va <= rva < va + rs:
                return rva - va + ro
        return None

    def raw(self, rva, n):
        o = self.off(rva)
        return bytes(self.data[o:o + n]) if o is not None else b''

    def cstr(self, rva):
        b = self.raw(rva, 256)
        z = b.find(b'\0')
        s = b[:z] if z >= 0 else b
        return s.decode() if s and all(32 <= c < 127 for c in s) else None

    def func_of(self, rva):
        f = self.funcs
        i = int(self.np.searchsorted(f[:, 0], rva, side='right')) - 1
        if i >= 0 and f[i, 0] <= rva < f[i, 1]:
            return int(f[i, 0]), int(f[i, 1])
        return None

    def callers(self, targets):
        np = self.np
        want = np.array(sorted(t - BASE for t in targets), dtype=np.int64)
        out = set()
        for va, vs, ro, rs, x in self.secs:
            if not x:
                continue
            b = np.asarray(self.data[ro:ro + rs]).astype(np.int64)
            pos = np.nonzero(b[:-4] == 0xE8)[0]
            d = b[pos + 1] | (b[pos + 2] << 8) | (b[pos + 3] << 16) | (b[pos + 4] << 24)
            d = np.where(d >= 1 << 31, d - (1 << 32), d)
            tgt = va + pos + 5 + d
            for p in pos[np.isin(tgt, want)]:
                f = self.func_of(va + int(p))
                if f:
                    out.add(f[0])
        return out


def names_in(exe, rva):
    from capstone import CS_ARCH_X86, CS_MODE_64, Cs
    from capstone.x86 import X86_OP_IMM, X86_OP_MEM, X86_OP_REG, X86_REG_RIP, X86_REG_RSP
    f = exe.func_of(rva)
    if not f or f[1] - f[0] > 0x60000:
        return set(), set()
    md = Cs(CS_ARCH_X86, CS_MODE_64)
    md.detail = True
    strings, calls, xmm, imm, stack, dirty = set(), set(), {}, {}, {}, set()

    def flush():
        starts = set()
        for o in dirty:
            s = o
            while (s - 1) in stack and 32 <= stack[s - 1] < 127:
                s -= 1
            starts.add(s)
        for s in starts:
            out, p = bytearray(), s
            while p in stack and stack[p] != 0 and len(out) < 256:
                out.append(stack[p])
                p += 1
            if len(out) >= 2 and all(32 <= c < 127 for c in out):
                strings.add(out.decode())
        dirty.clear()

    def put(off, data):
        for i, c in enumerate(data):
            stack[off + i] = c
            dirty.add(off + i)

    for ins in md.disasm(exe.raw(f[0], f[1] - f[0]), BASE + f[0]):
        ops, m = ins.operands, ins.mnemonic
        if m == 'call' or (m.startswith('j') and m != 'jmp'):
            flush()
        if m == 'call' and ops and ops[0].type == X86_OP_IMM:
            calls.add(ops[0].imm - BASE)
        for op in ops:
            if op.type == X86_OP_MEM and op.mem.base == X86_REG_RIP:
                s = exe.cstr(ins.address + ins.size + op.mem.disp - BASE)
                if s and len(s) >= 2:
                    strings.add(s)
        if m in ('vmovaps', 'vmovups', 'movaps', 'movups', 'vmovdqa', 'vmovdqu', 'movdqa', 'movdqu') and len(ops) == 2:
            d, s = ops
            if d.type == X86_OP_REG and s.type == X86_OP_MEM and s.mem.base == X86_REG_RIP:
                xmm[d.reg] = exe.raw(ins.address + ins.size + s.mem.disp - BASE, 16)
            elif d.type == X86_OP_MEM and d.mem.base == X86_REG_RSP and s.type == X86_OP_REG and xmm.get(s.reg):
                put(d.mem.disp, xmm[s.reg])
        elif m == 'movabs' and ops[0].type == X86_OP_REG and ops[1].type == X86_OP_IMM:
            imm[ops[0].reg] = (ops[1].imm & (2 ** 64 - 1)).to_bytes(8, 'little')
        elif m == 'mov' and len(ops) == 2 and ops[0].type == X86_OP_MEM and ops[0].mem.base == X86_REG_RSP:
            d, s = ops
            if s.type == X86_OP_IMM:
                put(d.mem.disp, (s.imm & (2 ** (8 * d.size) - 1)).to_bytes(d.size, 'little'))
            elif s.type == X86_OP_REG and s.reg in imm and d.size == 8:
                put(d.mem.disp, imm[s.reg])
        elif m == 'mov' and len(ops) == 2 and ops[0].type == X86_OP_REG:
            imm.pop(ops[0].reg, None)
    flush()
    return strings, calls


def exe_names(path, depth=2):
    build = BUILDS.get(sha256(path))
    if build is None:
        raise SystemExit(f'{path}: unknown F1_25.exe build; add its game-DB API and reader addresses to BUILDS')
    exe = Exe(path)
    funcs = exe.callers(build['api'])
    frontier = {r - BASE for r in build['readers']}
    seen = set()
    for level in range(depth + 1):
        nxt = set()
        for f in frontier - seen:
            seen.add(f)
            s, c = names_in(exe, f)
            if level < depth:
                nxt |= c
        frontier = nxt
    out = set()
    for f in funcs | seen:
        out |= names_in(exe, f)[0]
    return words(out)


def words(strings):
    out = set()
    for s in strings:
        for part in s.replace('%', ' ').split():
            out.add(part)
            out.update(p for p in part.split('.') if p)
    return sorted(w for w in out if len(w) < 120)


def fnv(s, h=gamedb.FNV_BASIS):
    return gamedb.fnv1a64(s.encode(), h)


def load_db(path):
    return {r.key: r.value.decode('utf-8', 'replace') for r in gamedb.read(open(path, 'rb').read())}


def row_keys(db, row, names, depth=0):
    out = []
    for w in names:
        if fnv('.' + w, row) in db:
            out.append(w)
        c = fnv('._' + w + '_instancecount_', row)
        if c in db and depth < 4:
            out.append(f'_{w}_instancecount_')
            try:
                n = int(db[c])
            except ValueError:
                continue
            for i in range(n):
                out += [f'{w}_{i}.{k}' for k in row_keys(db, fnv(f'.{w}_{i}', row), names, depth + 1)]
    return out


def table_rows(db, base, count, label_field):
    n = int(db.get(count, '0'))
    return {db.get(fnv(f'{i}.{label_field}', base)): fnv(str(i), base) for i in range(n)}


def missing(src_db, src, dst_db, dst, names):
    return [k for k in row_keys(src_db, src, names) if fnv('.' + k, dst) not in dst_db]


def main():
    ap = argparse.ArgumentParser(description='Game-DB names the game reads, and the keys they resolve to on a row')
    sub = ap.add_subparsers(dest='cmd', required=True)
    n = sub.add_parser('names', help='print the names F1_25.exe uses with the game DB, stack-built ones included')
    n.add_argument('exe')
    k = sub.add_parser('keys', help="print the keys a track's rows hold, found with the exe's names")
    k.add_argument('exe')
    k.add_argument('db')
    k.add_argument('track')
    a = ap.parse_args()
    names = exe_names(a.exe)
    if a.cmd == 'names':
        print('\n'.join(names))
        return
    db = load_db(a.db)
    reg = json.load(open(REGISTRY))
    tracks = table_rows(db, fnv('tracks.Track_'), fnv('tracks._Track_instancecount_'), 'instanceName')
    print(json.dumps({'tracks.Track': row_keys(db, tracks[a.track], names)}, indent=1))
    for t in reg['per_track']:
        rows = table_rows(db, int(t['base'], 16), int(t['count'], 16), t['labels'][0])
        if a.track in rows:
            print(json.dumps({t['id']: row_keys(db, rows[a.track], names)}, indent=1))


if __name__ == '__main__':
    main()
