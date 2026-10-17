import sys, os, struct
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gamedb

M64 = (1 << 64) - 1
MULT0 = 0xc4ceb9fe1a85ec53
RESEED = 0xc4ceb9fe1a85ec54
HALF = bytes(i >> 1 for i in range(256))


def max_allowed(n):
    return n * 80 // 100 if n <= M64 // 100 else (n // 100) * 80


def with_buffer(n):
    return n + min(max_allowed(n), 0xff)


class RobinHood:
    def __init__(self):
        self.mult = MULT0
        self.mask = 0
        self.size = 0

    def init_data(self, n):
        self.mask = n - 1
        self.size = 0
        self.max_allowed = max_allowed(n)
        nb = with_buffer(n)
        self.info = bytearray(nb + 8)
        self.info[nb] = 1
        self.keys = [0] * nb
        self.inc = 0x20
        self.shift = 0

    def rehash(self, n):
        old_info, old_keys = (self.info, self.keys) if self.mask else (None, None)
        old_nb = with_buffer(self.mask + 1) if self.mask else 1
        self.init_data(n)
        if old_nb > 1:
            for i in range(old_nb):
                if old_info[i]:
                    self.insert_move(old_keys[i])

    def key_to_idx(self, key):
        h = (key * self.mult) & M64
        h ^= h >> 33
        return (h >> 5) & self.mask, self.inc + ((h & 0x1f) >> self.shift)

    def try_increase_info(self):
        if self.inc <= 2:
            return False
        self.inc >>= 1
        self.shift += 1
        nb = with_buffer(self.mask + 1)
        words = (nb + 7) // 8
        self.info[:words * 8] = self.info[:words * 8].translate(HALF)
        self.info[nb] = 1
        self.max_allowed = max_allowed(self.mask + 1)
        return True

    def increase_size(self):
        if self.mask == 0:
            self.init_data(8)
            return
        ma = max_allowed(self.mask + 1)
        if self.size < ma and self.try_increase_info():
            return
        if self.size * 2 < max_allowed(self.mask + 1):
            self.mult = (self.mult + RESEED) & M64
            self.rehash(self.mask + 1)
        else:
            self.rehash((self.mask + 1) * 2)

    def shift_up(self, start, ins):
        info, keys, inc = self.info, self.keys, self.inc
        idx = start
        while idx != ins:
            keys[idx] = keys[idx - 1]
            info[idx] = (info[idx - 1] + inc) & 0xff
            if info[idx] + inc > 0xff:
                self.max_allowed = 0
            idx -= 1

    def place(self, idx, info_v, key):
        info = self.info
        ins_idx, ins_info = idx, info_v
        if ins_info + self.inc > 0xff:
            self.max_allowed = 0
        while info[idx]:
            idx += 1
        if idx != ins_idx:
            self.shift_up(idx, ins_idx)
        info[ins_idx] = ins_info
        self.keys[ins_idx] = key
        self.size += 1

    def insert_move(self, key):
        if self.max_allowed == 0 and not self.try_increase_info():
            raise OverflowError
        idx, iv = self.key_to_idx(key)
        info = self.info
        while iv <= info[idx]:
            idx += 1
            iv += self.inc
        self.place(idx, iv, key)

    def insert(self, key):
        for _ in range(256):
            idx, iv = self.key_to_idx(key)
            info, inc = self.info, self.inc
            while iv < info[idx]:
                idx += 1
                iv += inc
            while iv == info[idx]:
                if self.keys[idx] == key:
                    return
                idx += 1
                iv += inc
            if self.size >= self.max_allowed:
                self.increase_size()
                continue
            self.place(idx, iv, key)
            return
        raise OverflowError

    def reserve(self, count):
        n = 8
        while max_allowed(n) < max(count, self.size):
            n *= 2
        if self.mask + 1 < n:
            self.rehash(n)

    def not_found_sentinel(self):
        nb = with_buffer(self.mask + 1)
        return struct.unpack_from('<Q', self.info, 8)[0], nb

    def residents_of(self, lo, hi):
        return [(i, self.info[i], self.keys[i]) for i in range(lo, hi) if self.info[i]]


def build(path_or_data):
    data = open(path_or_data, 'rb').read() if isinstance(path_or_data, str) else path_or_data
    recs = gamedb.read(data)
    t = RobinHood()
    t.reserve(len(recs))
    for r in recs:
        t.insert(r.key)
    return t, recs


def main():
    for p in sys.argv[1:]:
        t, recs = build(p)
        s, nb = t.not_found_sentinel()
        verdict = 'OK (null)' if s == 0 else 'BAD (garbage record pointer)'
        print(f'{p}\n  records={len(recs)} mask=0x{t.mask:x} slots={nb:#x} inc={t.inc} shift={t.shift} '
              f'mult=0x{t.mult:016x}\n  not-found sentinel *(u64*)&info[8] = 0x{s:016x}  -> {verdict}')
        if s:
            for i, iv, k in t.residents_of(8, 16):
                print(f'    slot {i}: info=0x{iv:02x} key=0x{k:016x}')


if __name__ == '__main__':
    main()
