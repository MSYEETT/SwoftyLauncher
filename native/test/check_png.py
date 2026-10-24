import struct, sys, zlib

data = open(sys.argv[1], 'rb').read()
rgb = open(sys.argv[1] + '.rgb', 'rb').read()
pos, idat, size = 8, b'', None
while pos < len(data):
    n, kind = struct.unpack('>I4s', data[pos:pos + 8])
    body = data[pos + 8:pos + 8 + n]
    crc, = struct.unpack('>I', data[pos + 8 + n:pos + 12 + n])
    assert zlib.crc32(kind + body) == crc, kind
    if kind == b'IHDR':
        size = struct.unpack('>II', body[:8])
    if kind == b'IDAT':
        idat += body
    pos += 12 + n
w, h = size
raw = zlib.decompress(idat)
out = bytearray()
for y in range(h):
    row = raw[y * (w * 3 + 1):(y + 1) * (w * 3 + 1)]
    assert row[0] == 1
    line = bytearray(row[1:])
    for x in range(3, len(line)):
        line[x] = (line[x] + line[x - 3]) & 0xff
    out += line
assert bytes(out) == rgb, 'pixels differ'
print('ok  : the capture PNG inflates with zlib to the source pixels')
