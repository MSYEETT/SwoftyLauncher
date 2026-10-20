#!/usr/bin/env python3
"""Writes silent copies of the agent's speech banks.

usage: mute_agent.py <F1 25 folder> <out dir> [language ...]   (default language: en)

Every Wwise Opus packet in <lang>_agent.nfs and <lang>_agent_cutscene.nfs becomes a silent CELT frame of the same
length, so entry sizes, seek tables and line durations stay stock. The NeFS block checksums and header hash are
rewritten.
"""
import hashlib
import os
import struct
import sys
import zlib


def u16(b, o):
    return struct.unpack_from("<H", b, o)[0]


def u32(b, o):
    return struct.unpack_from("<I", b, o)[0]


def crc16(b):
    c = zlib.crc32(b)
    return ((c & 0xFFFF) + (c >> 16)) & 0xFFFF


class Nefs:
    def __init__(self, data):
        assert data[:4] == b"NeFS"
        self.hsize = u32(data, 100)
        self.bsz = u16(data, 132) << 15
        self.at = struct.unpack_from("<8I", data, 136)
        e_at, _, s_at, _, n_at, b_at, v_at, _ = self.at
        self.raw = [struct.unpack_from("<5I", data, e_at + 20 * i) for i in range((s_at - e_at) // 20)]
        self.shared = [struct.unpack_from("<5I", data, s_at + 20 * i) for i in range((n_at - s_at) // 20)]
        self.names = data[n_at:b_at]
        self.blocks = [struct.unpack_from("<IHH", data, b_at + 8 * i) for i in range((v_at - b_at) // 8)]

    def name(self, off):
        return self.names[off:self.names.index(b"\0", off)].decode()

    def entries(self):
        for lo, hi, s, fb, _ in self.raw:
            yield self.name(self.shared[s][2]), hi << 32 | lo, self.shared[s][3], fb


def chunks(w):
    assert w[:4] == b"RIFF" and w[8:12] == b"WAVE"
    pos, out = 12, {}
    while pos + 8 <= len(w):
        cid, size = w[pos:pos + 4], u32(w, pos + 4)
        out[cid] = (pos + 8, size)
        pos += 8 + size + (size & 1)
    return out


def silent_packet(toc, length):
    assert toc >> 3 >= 16, "not a CELT-only packet"
    base = toc & 0xFC
    if length == 3:
        return bytes([base, 0xFF, 0xFE])
    if length == 4:
        return bytes([base | 3, 0x01, 0xFF, 0xFE])
    a = 0
    while not 254 * a <= length - 5 - a <= 254 * a + 254:
        a += 1
    pad = length - 5 - a
    pkt = bytes([base | 3, 0x41]) + b"\xff" * a + bytes([pad - 254 * a]) + b"\xff\xfe" + b"\0" * pad
    assert len(pkt) == length
    return pkt


def silence_wem(w):
    c = chunks(w)
    assert u16(w, c[b"fmt "][0]) == 0x3041, "not Wwise Opus"
    so, ss = c[b"seek"]
    do, ds = c[b"data"]
    sizes = struct.unpack_from("<%dH" % (ss // 2), w, so)
    assert sum(sizes) == ds
    out, off = bytearray(w), do
    for size in sizes:
        out[off:off + size] = silent_packet(w[off], size)
        off += size
    return bytes(out)


def silence_archive(src, dst):
    d = bytearray(open(src, "rb").read())
    n = Nefs(bytes(d))
    assert hashlib.sha256(d[:4] + d[0x24:n.hsize]).digest() == d[4:36], "unknown header hash"
    b_at = n.at[5]
    count = 0
    for name, start, size, fb in n.entries():
        if size == 0 or not name.endswith(".wem"):
            continue
        blocks = n.blocks[fb:fb + -(-size // n.bsz)]
        prev = 0
        for end, transform, ck in blocks:
            assert transform == 0, "transformed block"
            assert crc16(d[start + prev:start + end]) == ck, "unknown block checksum"
            prev = end
        d[start:start + size] = silence_wem(bytes(d[start:start + size]))
        prev = 0
        for k, (end, _, _) in enumerate(blocks):
            struct.pack_into("<H", d, b_at + 8 * (fb + k) + 6, crc16(d[start + prev:start + end]))
            prev = end
        count += 1
    d[4:36] = hashlib.sha256(d[:4] + d[0x24:n.hsize]).digest()
    with open(dst, "wb") as f:
        f.write(d)
    return count


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    game, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    for lang in sys.argv[3:] or ["en"]:
        for bank in (f"{lang}_agent.nfs", f"{lang}_agent_cutscene.nfs"):
            n = silence_archive(os.path.join(game, "audio", "2025", "speech", bank), os.path.join(out, bank))
            print(f"{bank}: {n} lines silenced")


if __name__ == "__main__":
    main()
