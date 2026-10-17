#!/usr/bin/env python3
import argparse
import collections
import gzip
import json
import os
import re
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..'))
import gamedb

P = np.uint64(gamedb.FNV_PRIME)
PINV = np.uint64(gamedb.FNV_PRIME_INV)
BASIS = gamedb.FNV_BASIS
DB_REL = os.path.join('game_package', 'gamedatabases', 'gamedatabases.2025.bin')
TYPES = gamedb.TYPE_NAMES
ROW_INDEX = re.compile(r'_\d+$')
NESTED = re.compile(r'_\d+\.')
NAME_OK = re.compile(r'^[A-Za-z0-9_][A-Za-z0-9_ .\-/\[\]&()+:]*$')


def fwd(h, s):
    h = h.copy()
    for c in s:
        h ^= np.uint64(c)
        h *= P
    return h


def back(h, s):
    h = h.copy()
    for c in reversed(s):
        h *= PINV
        h ^= np.uint64(c)
    return h


def fnv(s, h=BASIS):
    return gamedb.fnv1a64(s, h)


class KeySet:
    def __init__(self, keys):
        self.sorted = np.sort(np.fromiter(keys, dtype=np.uint64))
        self.bloom = np.zeros(1 << 24, dtype=bool)
        self.bloom[self.sorted >> np.uint64(40)] = True

    def has(self, h):
        out = np.zeros(len(h), dtype=bool)
        m = self.bloom[h >> np.uint64(40)]
        if m.any():
            c = h[m]
            i = np.searchsorted(self.sorted, c)
            i[i >= len(self.sorted)] = 0
            out[m] = self.sorted[i] == c
        return out


def probe(nodes, strings, keys):
    trie = {}
    for s in strings:
        t = trie
        for c in s.encode():
            t = t.setdefault(c, {})
        t[None] = s
    hits = {}
    stack = [(trie, nodes)]
    while stack:
        t, h = stack.pop()
        s = t.get(None)
        if s is not None:
            m = keys.has(h)
            if m.any():
                hits[s] = np.nonzero(m)[0]
        for c, sub in t.items():
            if c is not None:
                stack.append((sub, (h ^ np.uint64(c)) * P))
    return hits


def exe_names(exe, cache):
    if cache and os.path.exists(cache):
        return [l.rstrip('\n') for l in open(cache, encoding='utf-8') if l.strip()]
    import track_keys
    names = track_keys.exe_names(exe)
    if cache:
        with open(cache, 'w', encoding='utf-8') as f:
            f.write('\n'.join(names) + '\n')
    return names


def exe_immediates(exe, keys):
    import pefile
    pe = pefile.PE(exe, fast_load=True)
    data = np.memmap(exe, dtype=np.uint8, mode='r')
    found = collections.Counter()
    for s in pe.sections:
        if not s.Characteristics & 0x20000000:
            continue
        b = np.asarray(data[s.PointerToRawData:s.PointerToRawData + s.SizeOfRawData])
        pos = np.nonzero(((b[:-10] & 0xFE) == 0x48) & ((b[1:-9] & 0xF8) == 0xB8))[0] + 2
        if not len(pos):
            continue
        imm = np.zeros(len(pos), dtype=np.uint64)
        for i in range(8):
            imm |= b[pos + i].astype(np.uint64) << np.uint64(8 * i)
        for k in imm[keys.has(imm)]:
            found[int(k)] += 1
    return found


def words_of(names):
    out = set()
    for n in names:
        for part in n.split('.'):
            w = ROW_INDEX.sub('', part)
            if w and NAME_OK.match(w):
                out.add(w)
    return out


CAMEL = re.compile(r'(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])')
UNITS = [
    (re.compile(r'(^|_)(metres|meters|metre|meter)$|_m$'), 'm'),
    (re.compile(r'(kilometers|kilometres|_km)$'), 'km'),
    (re.compile(r'(_deg|degrees|angle|latitude|longitude)$'), 'degrees'),
    (re.compile(r'(kph|kmh)$'), 'km/h'),
    (re.compile(r'mph$'), 'mph'),
    (re.compile(r'(seconds|_secs|_sec|_s|duration|delay)$'), 's'),
    (re.compile(r'(percent|percentage|_pct)$'), '%'),
    (re.compile(r'(ratio|scale|scalar|multiplier|multi|factor|modifier|mult)$'), 'multiplier'),
    (re.compile(r'(^num_|count$|^number)'), 'count'),
    (re.compile(r'laps$'), 'laps'),
]


def humanize(name):
    words = []
    for part in name.split('.'):
        part = re.sub(r'_(\d+|N)$', '', part)
        words += [w for w in re.split(r'[_\s]+', CAMEL.sub('_', part)) if w]
    return ' '.join(w.lower() if not w.isupper() or len(w) > 4 else w for w in words)


def unit_of(field, types):
    if 'time' in types:
        return 'HH,MM'
    if 't11' in types and re.search(r'colou?r', field, re.I):
        return 'ARGB'
    leaf = field.split('.')[-1].lower()
    for rx, u in UNITS:
        if rx.search(leaf):
            return u
    return ''


def guess_key(field, types):
    leaf = field.split('.')[-1]
    if len(ROW_INDEX.sub('', leaf)) < 2 and '.' not in field:
        return ''
    h = humanize(field)
    t = types[0] if types else ''
    if t == 'bool':
        return 'Flag: ' + h
    if t == 'curve':
        return 'Curve of ' + h
    if t == 'list':
        return 'List of ' + h
    if t == 'vector':
        return 'Vector: ' + h
    return h[:1].upper() + h[1:]


class Meanings:
    def __init__(self, path):
        self.rules = {'table': [], 'key': []}
        if not path or not os.path.exists(path):
            return
        with open(path, encoding='utf-8') as f:
            next(f)
            for line in f:
                if not line.strip():
                    continue
                kind, pat, meaning, unit, evidence, source = (line.rstrip('\n').split('\t') + [''] * 6)[:6]
                rx = re.compile('^' + '.*'.join(re.escape(p) for p in pat.split('*')) + '$')
                self.rules[kind].append((len(pat.replace('*', '')), rx, meaning, unit, evidence, source))
        for k in self.rules:
            self.rules[k].sort(key=lambda r: -r[0])

    def find(self, kind, *names):
        for _, rx, meaning, unit, evidence, source in self.rules[kind]:
            if any(rx.match(n) for n in names if n):
                return meaning, unit, evidence, source, rx.pattern.endswith('.*$')
        return None


class Catalog:
    def __init__(self, db_path, names, exe_set, imm):
        self.recs = gamedb.read(open(db_path, 'rb').read())
        self.rec = {r.key: r for r in self.recs}
        self.keys = KeySet(self.rec.keys())
        self.exe = exe_set
        self.imm = imm
        self.names = names
        self.node_hash = []
        self.node_name = []
        self.node_info = []
        self.by_hash = {}
        self.tables = {}

    def add_node(self, h, name, info):
        if h in self.by_hash:
            return self.by_hash[h]
        i = len(self.node_hash)
        self.node_hash.append(h)
        self.node_name.append(name)
        self.node_info.append(info)
        self.by_hash[h] = i
        return i

    def text(self, key):
        r = self.rec.get(key)
        return None if r is None else r.value.decode('utf-8', 'replace')

    def run(self):
        fields = sorted(n for n in self.names if not NESTED.search(n) and not n.startswith('#'))
        single = sorted(words_of(self.names))
        self.count_probes = ['._%s_instancecount_' % w for w in single]
        self.field_probes = ['.' + f for f in fields]
        containers = set()
        for n in fields:
            parts = n.split('.')
            for i in range(1, len(parts) + 1):
                containers.add('.'.join(parts[:i]))
        for c in sorted(containers):
            self.add_node(fnv(c), c, ('container', None))
        self.key_owner = {}
        self.done = 0
        self.bottom_up()
        self.probe_new()
        self.attach_orphans(single)
        self.probe_new()
        self.claim_metadata()

    def probe_new(self):
        while self.done < len(self.node_hash):
            todo = list(range(self.done, len(self.node_hash)))
            self.done = len(self.node_hash)
            h = np.array([self.node_hash[i] for i in todo], dtype=np.uint64)
            for s, idx in probe(h, self.count_probes, self.keys).items():
                w = s[2:-len('_instancecount_')]
                for j in idx:
                    self.add_table(todo[j], w)
            for s, idx in probe(h, self.field_probes, self.keys).items():
                for j in idx:
                    node = todo[j]
                    self.claim(fnv(s, self.node_hash[node]), node, s[1:])

    def bottom_up(self):
        k = self.keys.sorted
        r = back(k, b'._metadata_')
        b0 = back(r, b'0')
        b1 = back(r, b'1')
        bases = np.intersect1d(b0, b1)
        bogus = np.zeros(len(bases), dtype=bool)
        for d in b'0123456789':
            bogus |= np.isin(back(bases, bytes([d])), bases)
        groups = {}
        for base in bases[~bogus].tolist():
            n = 0
            while self.keys.has(np.array([fnv('%d._metadata_' % n, base)], dtype=np.uint64))[0]:
                n += 1
            groups.setdefault(base >> 4, []).append((base, n))
        self.orphan_groups = groups
        self.orphan_bases = {b: n for members in groups.values() for b, n in members}

    def depth(self, node):
        kind, info = self.node_info[node]
        if kind == 'row':
            return 1000 + info[1]
        name = self.node_name[node]
        return name.count('.') if name else 0

    def claim(self, key, node, rel):
        cur = self.key_owner.get(key)
        if cur is None or self.depth(node) > self.depth(cur[0]):
            self.key_owner[key] = (node, rel)

    def table_id(self, parent, word, base):
        t = self.tables.get(base)
        if t is None:
            t = {'parent': parent, 'word': word, 'base': base, 'rows': [], 'count': None}
            self.tables[base] = t
        return t

    def add_table(self, parent, word):
        ph = self.node_hash[parent]
        base = fnv('.%s_' % word, ph)
        t = self.table_id(parent, word, base)
        if t['parent'] is None:
            t['parent'], t['word'] = parent, word
        count_key = fnv('._%s_instancecount_' % word, ph)
        t['count_key'] = count_key
        try:
            t['count'] = int(self.text(count_key).strip())
        except (ValueError, AttributeError):
            t['count'] = 0
        n = max(t['count'], self.orphan_bases.pop(base, 0))
        self.fill_rows(t, n)
        self.claim(count_key, parent, '_%s_instancecount_' % word)

    def fill_rows(self, t, n):
        pdepth = 0
        if t['parent'] is not None and self.node_info[t['parent']][0] == 'row':
            pdepth = self.node_info[t['parent']][1][1]
        for i in range(len(t['rows']), n):
            h = fnv(str(i), t['base'])
            node = self.add_node(h, None, ('row', (t['base'], pdepth + 1, i)))
            if self.node_info[node][0] != 'row':
                self.node_info[node] = ('row', (t['base'], pdepth + 1, i))
            if self.node_info[node][1][0] == t['base']:
                t['rows'].append(node)

    def attach_orphans(self, single):
        bases = sorted(self.orphan_bases)
        if bases:
            arr = np.array(bases, dtype=np.uint64)
            for w in single:
                c = back(arr, ('.%s_' % w).encode())
                m = self.keys.has(fwd(c, ('._%s_instancecount_' % w).encode()))
                for j in np.nonzero(m)[0]:
                    if bases[j] not in self.orphan_bases:
                        continue
                    ch = int(c[j])
                    parent = self.by_hash.get(ch)
                    if parent is None:
                        parent = self.add_node(ch, None, ('object', None))
                    self.add_table(parent, w)
        for members in self.orphan_groups.values():
            left = [(b, n) for b, n in members if b in self.orphan_bases]
            if len(left) < len(members):
                keep = []
            else:
                keep = [min(left, key=lambda m: (-m[1], m[0]))]
            for b, _ in left:
                self.orphan_bases.pop(b)
            for b, n in keep:
                self.fill_rows(self.table_id(None, None, b), n)
        self.orphan_bases.clear()
        objects = [i for i, (k, v) in enumerate(self.node_info) if k == 'object' and v is None]
        if objects:
            arr = np.array([self.node_hash[i] for i in objects], dtype=np.uint64)
            known = np.array(sorted(h for h, i in self.by_hash.items() if self.node_info[i][0] != 'object'),
                             dtype=np.uint64)
            for s in self.field_probes:
                up = back(arr, s.encode())
                for j in np.nonzero(np.isin(up, known))[0]:
                    o = objects[j]
                    if self.node_info[o][1] is None:
                        self.node_info[o] = ('object', (self.by_hash[int(up[j])], s[1:]))

    def claim_metadata(self):
        for t in self.tables.values():
            for r in t['rows']:
                k = fnv('._metadata_', self.node_hash[r])
                if k in self.rec:
                    self.claim(k, r, '_metadata_')

    def name(self, node, family=False, memo=None):
        memo = {} if memo is None else memo
        key = (node, family)
        if key in memo:
            return memo[key]
        kind, info = self.node_info[node]
        if kind == 'container':
            out = self.node_name[node]
        elif kind == 'object':
            if info is None:
                out = '{%016x}' % self.node_hash[node]
            else:
                out = self.name(info[0], family, memo) + '.' + info[1]
        else:
            base, _, i = info
            t = self.tables[base]
            idx = 'N' if family else str(i)
            if t['word'] is None:
                out = '{%016x}%s' % (base, idx)
            else:
                out = '%s.%s_%s' % (self.name(t['parent'], family, memo), t['word'], idx)
        memo[key] = out
        return out


def scope_sets(cat, rows_by_family):
    def labels(fam, field):
        out = set()
        for r in rows_by_family.get(fam, []):
            v = cat.text(fnv('.' + field, cat.node_hash[r]))
            if v:
                out.add(v)
        return out
    return {
        'track': labels('tracks.Track_N', 'instanceName') | labels('tracks.Track_N', '_metadata_'),
        'team': labels('entrants.teams.team_N', '_metadata_'),
        'driver': labels('entrants.drivers.driver_N', '_metadata_'),
        'vehicle': labels('entrants.vehicles.vehicle_N', '_metadata_'),
        'dataset': labels('{2e47cdd25071dcdd}N', '_metadata_') | labels('track_data_sets', '_metadata_'),
    }


SEASON = re.compile(r'(^|_)(f[12]|season|sprint)_?(20\d\d|\d\d)($|_)')


def classify(values, sets):
    vals = [v for v in values if v]
    if not vals:
        return ''
    best, score = '', 0.0
    for k in ('track', 'team', 'driver', 'vehicle', 'dataset'):
        s = sets[k]
        if not s:
            continue
        f = sum(v in s for v in vals) / len(vals)
        if f > score:
            best, score = k, f
    if score >= 0.6:
        return best
    if sum(bool(SEASON.search(v)) for v in vals) / len(vals) >= 0.6:
        return 'season'
    return ''


def short(v, n=48):
    v = v.replace('\t', ' ').replace('\n', ' ')
    return v if len(v) <= n else v[:n - 1] + '…'


def build(args):
    db = args.db or os.path.join(args.game, DB_REL)
    exe = args.exe or (os.path.join(args.game, 'F1_25.exe') if args.game else None)
    seed = [l.rstrip('\n') for l in open(args.names, encoding='utf-8') if l.strip()]
    enames = exe_names(exe, args.exe_names) if (exe or args.exe_names) else []
    exe_set = set(enames)
    exe_words = words_of(enames)
    names = set(seed) | {n for n in enames if NAME_OK.match(n) and not n.endswith('.')}
    cat = Catalog(db, names, exe_set, {})
    if exe and not args.no_immediates:
        cat.imm = exe_immediates(exe, cat.keys)
    cat.run()

    memo = {}
    rows_by_family = collections.defaultdict(list)
    for t in cat.tables.values():
        for r in t['rows']:
            rows_by_family[cat.name(r, True, memo)].append(r)
    sets = scope_sets(cat, rows_by_family)

    fam_keys = collections.defaultdict(lambda: collections.defaultdict(lambda: {
        'types': collections.Counter(), 'rows': 0, 'examples': [], 'imm': 0, 'lo': None, 'hi': None,
        'distinct': set(), 'sample': []}))
    for key, (node, rel) in cat.key_owner.items():
        fam = cat.name(node, True, memo)
        e = fam_keys[fam][rel]
        r = cat.rec[key]
        tn = TYPES.get(r.type, 't%d' % r.type)
        e['types'][tn] += 1
        e['rows'] += 1
        e['imm'] += cat.imm.get(key, 0) > 0
        text = r.value.decode('utf-8', 'replace')
        if len(e['examples']) < 3:
            v = short(text)
            if v not in e['examples']:
                e['examples'].append(v)
        if tn in ('int', 'float', 't11', 't12'):
            try:
                x = float(text)
                e['lo'] = x if e['lo'] is None else min(e['lo'], x)
                e['hi'] = x if e['hi'] is None else max(e['hi'], x)
            except ValueError:
                pass
        elif tn in ('bool', 'enum') and len(e['distinct']) <= 8:
            e['distinct'].add(text)
        if text and len(e['sample']) < 40 and tn in ('id', 'string', 'list'):
            e['sample'].append(text)

    label_sets = {}
    for fam, rs in rows_by_family.items():
        if fam.startswith('{') or len(rs) < 2:
            continue
        labs = {cat.text(fnv('._metadata_', cat.node_hash[r])) for r in rs} - {None, ''}
        if fam == 'tracks.Track_N':
            labs |= {cat.text(fnv('.instanceName', cat.node_hash[r])) for r in rs} - {None, ''}
        if len(labs) >= 2:
            label_sets[fam] = labs
    canonical = ['tracks.Track_N', 'entrants.teams.team_N', 'entrants.drivers.driver_N', 'entrants.vehicles.vehicle_N',
                 'entrants.entrant_lists.entrant_list_N', 'entrants.characters.character_N',
                 'entrants.nationality.nationality_N', 'entrants.vehicles.vehicle_packages.vehicle_package_N',
                 'track_data.game_setup.TrackDataSet_N', 'entrants.standings.standings_N']

    def reference(sample):
        vals = [v for x in sample for v in x.split(',') if v]
        if len(vals) < 2:
            return None
        best, score = None, 0.0
        for fam in canonical + sorted(label_sets, key=lambda f: -len(label_sets[f])):
            labs = label_sets.get(fam)
            if not labs:
                continue
            f = sum(v in labs for v in vals) / len(vals)
            need = 0.8 if fam in canonical else 0.95
            if f >= need and f > score:
                best, score = fam, f
                if fam in canonical:
                    break
        return best

    meanings = Meanings(args.meanings)

    def value_range(e):
        if e['lo'] is not None:
            lo, hi = e['lo'], e['hi']
            fmt = lambda x: ('%d' % x) if x == int(x) else ('%g' % x)
            return fmt(lo) if lo == hi else '%s..%s' % (fmt(lo), fmt(hi))
        if e['distinct']:
            vals = sorted(e['distinct'])
            return ', '.join(short(v, 20) for v in vals[:6]) + (', …' if len(vals) > 6 else '')
        return ''

    def key_meaning(fam, rel, e, base_alias):
        types = [k for k, _ in e['types'].most_common()]
        hit = meanings.find('key', '%s.%s' % (fam, rel), base_alias and '%s.%s' % (base_alias, rel))
        if hit:
            m, unit, ev, src, group = hit
            if group:
                m = '%s: %s' % (m, humanize(rel.split('.')[-1]))
            return m, unit or unit_of(rel, types), ev, src
        unit = unit_of(rel, types)
        ref = reference(e['sample'])
        if ref:
            what = 'Labels of rows' if 'list' in types or any(',' in x for x in e['sample']) else 'Label of a row'
            return '%s in %s' % (what, ref), unit, 'inferred', 'values match that table\'s labels'
        g = guess_key(rel, types)
        if g:
            return g, unit, 'inferred', 'name'
        return '', unit, 'unknown', ''

    def exe_flag(path):
        parts = path.split('.')
        for i in range(len(parts) - 1):
            if '.'.join(parts[i:]) in exe_set:
                return 'path'
        if parts[-1] in exe_set or parts[-1] in exe_words:
            return 'leaf'
        return ''

    tables = []
    for t in cat.tables.values():
        if not t['rows']:
            continue
        fam = cat.name(t['rows'][0], True, memo)
        tables.append((fam, t))
    fam_tables = collections.defaultdict(list)
    for fam, t in tables:
        fam_tables[fam].append(t)

    out_tables = []
    base_of = {}
    for fam in sorted(fam_tables):
        ts = fam_tables[fam]
        rows = rows_by_family[fam]
        labels = [cat.text(fnv('._metadata_', cat.node_hash[r])) for r in rows]
        trackf = [cat.text(fnv('.track', cat.node_hash[r])) for r in rows]
        scope = classify(labels, sets) or classify(trackf, sets)
        if fam == 'tracks.Track_N':
            scope = 'track'
        counts = sorted({t['count'] for t in ts if t['count'] is not None})
        sizes = sorted({len(t['rows']) for t in ts})
        w = ts[0]['word']
        fields = fam_keys.get(fam, {})
        base_alias = '{%016x}N' % ts[0]['base']
        hit = meanings.find('table', fam, base_alias)
        if hit:
            tmeaning, _, tev, tsrc, _ = hit
        elif w:
            parent = fam[:-(len(w) + 3)]
            where = humanize(parent.split('.')[-1]) if parent and not parent.endswith('}') else ''
            tmeaning = 'Rows of %s%s' % (humanize(w), ' under ' + where if where and where != humanize(w) else '')
            tev, tsrc = 'inferred', 'name'
        else:
            tmeaning, tev, tsrc = '', 'unknown', ''
        base_of[fam] = base_alias if len(ts) == 1 else ''
        out_tables.append({
            'table': fam,
            'row_word': w or '',
            'instances': len(ts),
            'rows': len(rows),
            'rows_per_instance': '%d' % sizes[0] if len(sizes) == 1 else '%d-%d' % (sizes[0], sizes[-1]),
            'instancecount': ('-' if not counts else
                              '%d' % counts[0] if len(counts) == 1 else '%d-%d' % (counts[0], counts[-1])),
            'fields': len([f for f in fields if not f.startswith('_') or f == '_metadata_']),
            'keys': sum(e['rows'] for e in fields.values()),
            'scope': scope,
            'labels': [short(l, 32) for l in labels if l][:4],
            'exe_fields': sum(1 for f in fields if exe_flag(f) == 'path' or f in exe_set),
            'base': base_alias[1:-2] if len(ts) == 1 else '',
            'meaning': tmeaning,
            'evidence': tev,
            'source': tsrc,
        })

    out_keys = []
    for fam in sorted(fam_keys):
        for rel in sorted(fam_keys[fam]):
            e = fam_keys[fam][rel]
            path = '%s.%s' % (fam, rel)
            nrows = len(rows_by_family.get(fam, [])) or 1
            flag = exe_flag(rel if fam.startswith('{') else path)
            if e['imm']:
                flag = 'hash' if not flag else flag + '+hash'
            meaning, unit, ev, src = key_meaning(fam, rel, e, base_of.get(fam, ''))
            if flag.startswith('path') and ev == 'inferred' and src == 'name':
                src = 'name; read by the exe'
            out_keys.append({
                'key': path,
                'table': fam,
                'field': rel,
                'type': ','.join('%s' % k for k, _ in e['types'].most_common()),
                'records': e['rows'],
                'rows': nrows,
                'in_db': 'yes',
                'exe': flag,
                'example': ' | '.join(e['examples']),
                'range': value_range(e),
                'unit': unit,
                'meaning': meaning,
                'evidence': ev,
                'source': src,
            })

    db_words = set()
    for fam, fs in fam_keys.items():
        for part in fam.split('.'):
            db_words.add(ROW_INDEX.sub('', part))
        for rel in fs:
            db_words.add(rel)
            for part in rel.split('.'):
                db_words.add(ROW_INDEX.sub('', part))
            parts = rel.split('.')
            for i in range(len(parts)):
                db_words.add('.'.join(parts[i:]))
    for t in out_tables:
        db_words.add(t['row_word'])
    full_paths = {k['key'] for k in out_keys}
    out_exe = []
    for n in sorted(exe_set):
        if n in db_words or n in full_paths:
            status = 'yes'
        elif n.rstrip('.') in db_words:
            status = 'yes'
        else:
            status = 'no'
        out_exe.append({'name': n, 'in_db': status})
        if status == 'no' and NAME_OK.match(n) and not n.endswith('.'):
            g = guess_key(n, [])
            out_keys.append({'key': n, 'table': '', 'field': n, 'type': '', 'records': 0, 'rows': 0,
                             'in_db': 'no', 'exe': 'name', 'example': '', 'range': '', 'unit': unit_of(n, []),
                             'meaning': g, 'evidence': 'inferred' if g else 'unknown', 'source': 'name' if g else ''})

    unnamed = collections.Counter()
    for r in cat.recs:
        if r.key not in cat.key_owner:
            unnamed[TYPES.get(r.type, 't%d' % r.type)] += 1
    imm_named = sum(1 for k in cat.imm if k in cat.key_owner)
    summary = {
        'db': os.path.basename(db),
        'db_bytes': os.path.getsize(db),
        'records': len(cat.recs),
        'named_records': len(cat.key_owner),
        'unnamed_records': sum(unnamed.values()),
        'unnamed_by_type': dict(unnamed.most_common()),
        'types': dict(collections.Counter(TYPES.get(r.type, 't%d' % r.type) for r in cat.recs).most_common()),
        'tables': len(out_tables),
        'table_instances': sum(t['instances'] for t in out_tables),
        'rows': sum(t['rows'] for t in out_tables),
        'named_tables': sum(1 for t in out_tables if not t['table'].startswith('{')),
        'schema_keys': sum(1 for k in out_keys if k['in_db'] == 'yes'),
        'schema_keys_exe_path': sum(1 for k in out_keys if k['in_db'] == 'yes' and k['exe'].startswith('path')),
        'schema_keys_exe_leaf': sum(1 for k in out_keys if k['in_db'] == 'yes' and k['exe'].startswith('leaf')),
        'exe_names': len(exe_set),
        'exe_names_in_db': sum(1 for e in out_exe if e['in_db'] == 'yes'),
        'exe_names_not_in_db': sum(1 for e in out_exe if e['in_db'] == 'no'),
        'exe_key_immediates': len(cat.imm),
        'exe_key_immediates_named': imm_named,
        'scopes': dict(collections.Counter(t['scope'] or 'other' for t in out_tables).most_common()),
        'table_evidence': dict(collections.Counter(t['evidence'] for t in out_tables).most_common()),
        'key_evidence': dict(collections.Counter(k['evidence'] for k in out_keys if k['in_db'] == 'yes').most_common()),
    }
    return summary, out_tables, out_keys, out_exe


FRONTMATTER = ['---', 'wide: true', '---', '']

SCHEMA_PAGES = [
    ('Tracks', r'^tracks\.'),
    ('Track data sets', r'^track_data\.'),
    ('Entrants', r'^entrants\.'),
    ('Content sets', r'^content_sets\.'),
    ('Pit lane', r'^pit_lane\.'),
    ('Canned events', r'^canned_events\.'),
]


def md(v):
    return str(v).replace('|', '\\|')


def mdt(v):
    return md(v).replace('<', '&lt;').replace('{', '&#123;').replace('}', '&#125;').replace('*', '\\*').replace('_', '\\_')


def meaning_cell(r):
    if not r['meaning']:
        return '*unknown*'
    return '%s *(%s)*' % (mdt(r['meaning']), r['evidence'])


def write_docs(path, summary, tables, keys):
    named = [t for t in tables if not t['table'].startswith('{')]
    hashed = [t for t in tables if t['table'].startswith('{')]
    out = ['<!-- Generated by tools/gamedb/catalog.py. Do not edit by hand. -->', '', '# Table catalogue', '',
           'Every table family found in `gamedatabases.2025.bin`, generated from the database and the names '
           '`F1_25.exe` reads. Row counts, field counts and types come from the data. See '
           '[the overview](/reference/gamedb#table-catalogue) for how to read this page.', '',
           '| | |', '|---|---|']
    for k, label in [('records', 'Records'), ('named_records', 'Records with a known name'),
                     ('tables', 'Table families'), ('named_tables', 'Families with a full path'),
                     ('table_instances', 'Table instances'), ('rows', 'Rows'),
                     ('schema_keys', 'Schema keys (`Table_N.field` form)'),
                     ('exe_names', 'Names the exe reads'), ('exe_names_not_in_db', 'Exe names with no match in the DB'),
                     ('exe_key_immediates', 'Full key hashes held as exe immediates')]:
        out.append('| %s | %s |' % (label, '{:,}'.format(summary[k])))
    out += ['', '## Tables with a full path', '',
            '`scope` is inferred: 60% or more of the rows carry a track, team, driver, vehicle or season label. '
            '`exe` counts fields whose dotted path appears in a game-DB reader. Each meaning is tagged *proven* '
            '(exe use or an in-game test), *inferred* (names or values) or *unknown*; see '
            '[the glossary](/reference/gamedb#glossary).', '',
            '| Table | Meaning | Rows | `_instancecount_` | Fields | Scope | `exe` | Labels |',
            '|---|---|---|---|---|---|---|---|']
    for t in named:
        rows = t['rows_per_instance'] if t['instances'] == 1 else '%s × %d' % (t['rows_per_instance'], t['instances'])
        out.append('| `%s` | %s | %s | %s | %d | %s | %d | %s |' % (
            md(t['table']), meaning_cell(t), rows, t['instancecount'], t['fields'], t['scope'], t['exe_fields'],
            mdt(', '.join(t['labels'][:3]))))
    out += ['', '## Tables with an unresolved container', '',
            '%d families sit under a container whose path is not known. They are listed in '
            '`tools/gamedb/catalog/tables.tsv` as `{hash}` placeholders (see '
            '[Unresolved paths](/reference/gamedb/format#unresolved-paths)). The largest:' % len(hashed), '',
            '| Table | Meaning | Rows | Fields | Labels |', '|---|---|---|---|---|']
    for t in sorted(hashed, key=lambda t: -t['rows'])[:40]:
        out.append('| `%s` | %s | %d | %d | %s |' % (md(t['table']), meaning_cell(t), t['rows'], t['fields'],
                                                   mdt(', '.join(t['labels'][:3]))))
    with open(os.path.join(path, 'catalogue.md'), 'w', encoding='utf-8') as f:
        f.write('\n'.join(FRONTMATTER + out) + '\n')

    by_table = collections.defaultdict(list)
    for k in keys:
        if k['in_db'] == 'yes':
            by_table[k['table']].append(k)
    out = ['<!-- Generated by tools/gamedb/catalog.py. Do not edit by hand. -->', '', '# Key schema', '',
           'Fields of the tables modders edit most, generated from the database. `Rows` is how many rows hold '
           'the field out of the family\'s rows. `exe` is `path` when the dotted path appears in a game-DB '
           'reader, `leaf` when only its last part does, and `hash` when the full key hash is an immediate in '
           'the exe. The full list for every table is `tools/gamedb/catalog/keys.tsv.gz`.', '']
    for title, pat in SCHEMA_PAGES:
        fams = [t for t in named if re.match(pat, t['table'])]
        if not fams:
            continue
        out += ['## %s' % title, '']
        for t in fams:
            ks = by_table.get(t['table'], [])
            if not ks:
                continue
            out += ['### `%s`' % md(t['table']), '',
                    '%s rows, `_instancecount_` %s.' % (t['rows'], t['instancecount']), '',
                    '| Field | Meaning | Type | Unit | Range | Rows | `exe` | Example |',
                    '|---|---|---|---|---|---|---|---|']
            for k in ks:
                out.append('| `%s` | %s | %s | %s | %s | %d/%d | %s | %s |' % (
                    md(k['field']), meaning_cell(k), k['type'], k['unit'], mdt(k['range']), k['records'], k['rows'],
                    k['exe'], mdt(k['example'])))
            out.append('')
    with open(os.path.join(path, 'schema.md'), 'w', encoding='utf-8') as f:
        f.write('\n'.join(FRONTMATTER + out) + '\n')


def write_tsv(path, rows, cols, gz=False):
    op = gzip.open if gz else open
    with op(path, 'wt', encoding='utf-8', newline='') as f:
        f.write('\t'.join(cols) + '\n')
        for r in rows:
            f.write('\t'.join(str(r[c]) if not isinstance(r[c], list) else ', '.join(r[c]) for c in cols) + '\n')


def main():
    ap = argparse.ArgumentParser(description='Catalogue every table and key of the F1 25 game database')
    ap.add_argument('--game', help='F1 25 folder (or a pristine copy) holding F1_25.exe and game_package/')
    ap.add_argument('--db', help='gamedatabases.2025.bin, overriding --game')
    ap.add_argument('--exe', help='F1_25.exe, overriding --game')
    ap.add_argument('--exe-names', help='read (or cache) the exe name list here instead of running track_keys')
    ap.add_argument('--names', default=os.path.join(HERE, 'names.txt'))
    ap.add_argument('--meanings', default=os.path.join(HERE, 'meanings.tsv'))
    ap.add_argument('--no-immediates', action='store_true')
    ap.add_argument('--out', default=os.path.join(HERE, 'catalog'))
    ap.add_argument('--docs', help='also write the generated docs pages into this folder')
    a = ap.parse_args()
    if not (a.game or a.db):
        ap.error('give --game or --db')
    summary, tables, keys, exe = build(a)
    os.makedirs(a.out, exist_ok=True)
    write_tsv(os.path.join(a.out, 'tables.tsv'), tables,
              ['table', 'row_word', 'base', 'instances', 'rows', 'rows_per_instance', 'instancecount', 'fields',
               'keys', 'scope', 'exe_fields', 'labels', 'meaning', 'evidence', 'source'])
    write_tsv(os.path.join(a.out, 'keys.tsv.gz'), keys,
              ['key', 'table', 'field', 'type', 'records', 'rows', 'in_db', 'exe', 'range', 'unit', 'example',
               'meaning', 'evidence', 'source'], gz=True)
    write_tsv(os.path.join(a.out, 'exe_names.tsv'), exe, ['name', 'in_db'])
    with open(os.path.join(a.out, 'summary.json'), 'w', encoding='utf-8') as f:
        json.dump(summary, f, indent=1)
        f.write('\n')
    if a.docs:
        os.makedirs(a.docs, exist_ok=True)
        write_docs(a.docs, summary, tables, keys)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
