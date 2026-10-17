import hashlib, json, re, struct, sys

exe = sys.argv[1]
data = open(exe, 'rb').read()
pe = struct.unpack_from('<I', data, 0x3c)[0]
nsec = struct.unpack_from('<H', data, pe + 6)[0]
optsz = struct.unpack_from('<H', data, pe + 20)[0]
base = struct.unpack_from('<Q', data, pe + 24 + 24)[0]
secs = {}
for i in range(nsec):
    o = pe + 24 + optsz + 40 * i
    name = data[o:o + 8].rstrip(b'\0').decode()
    vsz, va, rsz, roff = struct.unpack_from('<IIII', data, o + 8)
    secs[name] = (va, data[roff:roff + min(vsz, rsz)])
sha = hashlib.sha256(data).hexdigest()
build = {'exe': 'F1_25.exe (fake)', 'sha256': sha, 'size': len(data), 'pe_timestamp': struct.unpack_from('<I', data, pe + 8)[0], 'image_base': hex(base)}


def find(pattern, section='.xtls'):
    va, b = secs[section]
    rx = re.compile(b''.join(b'.' if t == '??' else re.escape(bytes([int(t, 16)])) for t in pattern.split()), re.S)
    hits = [m.start() for m in rx.finditer(b)]
    return [base + va + h for h in hits]


TICK = '53 56 48 83 EC 28 48 89 CB 89 D6 8B 43 10 83 C0 01 89 43 10 0F B7 43 08'
ENTRY = '53 48 83 EC 20 48 8D 04 11 4C 01 C0 4C 01 C8 48 8B 5C 24 50'
sigs = []


def add(sid, pattern, anchor, kind, expect=None, names=()):
    hits = find(pattern)
    assert len(hits) == 1, (sid, hits)
    s = {'id': sid, 'section': '.xtls', 'pattern': pattern, 'match': hex(hits[0]), 'anchor': hex(anchor),
         'resolve': {'kind': 'addr'}, 'target': hex(hits[0] + anchor), 'kind': kind, 'names': list(names),
         'verified': [{'build': sha[:16], 'date': '2026-10-17', 'by': 'trace_plugin/test/mksigs.py', 'how': 'static'}],
         'used_by': ['re/trace_plugin/test/T900.trace.json']}
    if expect:
        s['expect'] = expect
    sigs.append(s)


add('fake.tick_mid', TICK, 0xB, 'mid', '8B 43 10 83 C0', ['site_Fake_TickCounter'])
add('fake.entry', ENTRY, 0, 'func_start', '53 48 83 EC 20', ['Fake_Entry'])
add('fake.tick_head', '53 56 48 83 EC 28 48 89 CB 89 D6', 0xB, 'mid', '8B 43 10', ['site_Fake_TickCounter'])
add('fake.tick_expect_wrong', TICK, 0xB, 'mid', '90 90 90', ['site_Fake_TickCounter'])
add('fake.tick_mid_insn', TICK, 0xC, 'mid', None, ['site_Fake_TickCounter'])
assert len(find('48 31 C0 48 83 C0 2A 48 83 E8 15 48 83 F0 07')) == 2
json.dump({'schema': 1, 'build': build, 'signatures': sigs}, sys.stdout, indent=1)
print()
