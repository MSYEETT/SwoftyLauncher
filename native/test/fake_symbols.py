import struct, sys

d = open(sys.argv[1], 'rb').read()
pe = struct.unpack_from('<I', d, 0x3c)[0]
nsec, stamp, opt = struct.unpack_from('<H', d, pe + 6)[0], struct.unpack_from('<I', d, pe + 8)[0], struct.unpack_from('<H', d, pe + 20)[0]
secs = [struct.unpack_from('<8xIIII', d, pe + 24 + opt + 40 * i) for i in range(nsec)]
off = lambda r: next(r - va + rp for vs, va, rs, rp in secs if va <= r < va + max(vs, rs))
e = off(struct.unpack_from('<I', d, pe + 24 + 112)[0])
count, funcs, names, ords = struct.unpack_from('<IIII', d, e + 24)
for i in range(count):
    if d[off(struct.unpack_from('<I', d, off(names) + 4 * i)[0]):].split(b'\0')[0] == b'fake_crash':
        rva = struct.unpack_from('<I', d, off(funcs) + 4 * struct.unpack_from('<H', d, off(ords) + 2 * i)[0])[0]
open(sys.argv[2], 'w').write(f'# F1_25.exe fake {stamp}\n{rva:x}\tFake_Crash_Here\tL0\n')
