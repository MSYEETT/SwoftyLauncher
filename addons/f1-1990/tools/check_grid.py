#!/usr/bin/env python3
import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT.parents[1] / "tools"))
import gamedb as g


class Db:
    def __init__(self, path):
        self.v = {r.key: r.value.decode("utf-8", "replace") for r in g.read(Path(path).read_bytes())}

    def get(self, path):
        return self.v.get(g.fnv1a64(path))

    def has(self, path):
        return g.fnv1a64(path) in self.v

    def rows(self, table, word):
        n = int(self.get(f"{table}._{word}_instancecount_") or 0)
        return [(f"{table}.{word}_{i}", self.get(f"{table}.{word}_{i}._metadata_")) for i in range(n)]

    def find(self, table, word, label):
        hits = [p for p, m in self.rows(table, word) if m == label]
        return hits[0] if len(hits) == 1 else None


def main(argv):
    db = Db(argv[1])
    grid = json.loads((ROOT / "db/grid.json").read_text(encoding="utf-8"))
    problems = []
    lists = dict((m, p) for p, m in db.rows("entrants.entrant_lists", "entrant_list"))
    entrants = "entrants.entrant_lists.entrants"
    folders = dict((m, p) for p, m in db.rows("entrants.driver_customisations", "folder"))
    looks = {}
    for f in folders.values():
        for p, m in db.rows(f, "preloaded_driver_customisation_properties"):
            looks[m] = p
    seats = db.rows(lists["f1_2026"], "entrant")
    print(f"f1_2026: {len(seats)} entrants")
    numbers = Counter()
    drivers_seen = Counter()
    for p, m in seats:
        ref = db.get(p + ".entrant_reference")
        team = db.find("entrants.teams", "team", db.get(p + ".team"))
        vehicle = db.find("entrants.vehicles", "vehicle", db.get(p + ".vehicle"))
        e = db.find(entrants, "entrant", ref)
        if not (team and vehicle and e):
            problems.append(f"{m}: team/vehicle/entrant unresolved ({ref})")
            continue
        dlabel = db.get(e + ".driver")
        d = db.find("entrants.drivers", "driver", dlabel)
        look = db.get(e + ".driver_customisation")
        if not d or look not in looks:
            problems.append(f"{m}: driver {dlabel} or look {look} unresolved")
            continue
        num = db.get(p + ".car_number")
        numbers[num] += 1
        drivers_seen[dlabel] += 1
        print(f"  {m:22} #{num:>3} pos {db.get(p + '.team_position')}  {db.get(team + '.display_name_code')} "
              f"{dlabel:28} stat {db.get(d + '.stat_id'):>3} {db.get(d + '.initials')} {db.get(d + '.nationality')} "
              f"head {db.get(d + '.head')}")
    problems += [f"car number {n} used {c} times" for n, c in numbers.items() if c > 1]
    problems += [f"driver {d} seated {c} times" for d, c in drivers_seen.items() if c > 1]
    ids = Counter(db.get(p + ".stat_id") for p, _ in db.rows("entrants.drivers", "driver"))
    problems += [f"driver stat_id {i} used {c} times" for i, c in ids.items() if c > 1]
    if any(int(i) > 253 for i in ids if i and i != "254"):
        problems.append("driver stat_id above 253")
    presets = Counter(db.get(p + ".stat_id") for p in looks.values())
    problems += [f"look stat_id {i} used {c} times" for i, c in presets.items() if c > 1]
    labels = Counter(m for _, m in db.rows("entrants.drivers", "driver"))
    problems += [f"driver label {l} used {c} times" for l, c in labels.items() if c > 1]
    pool = [db.get(p + ".entrant_reference") for p, _ in db.rows(lists["f1_2026_driver_pool"], "entrant")]
    for ref in pool:
        e = db.find(entrants, "entrant", ref)
        if e and db.get(e + ".driver") in drivers_seen:
            problems.append(f"pool entrant {ref} is also seated")
    print(f"f1_2026_driver_pool: {', '.join(pool)}")
    holders = db.rows("driver_statistics.driver_statistics", "DriverStatisticsHolder")
    if len(holders) > 128:
        problems.append(f"{len(holders)} driver statistics holders; a race start aborts above 128")
    by_driver = Counter(db.get(p + ".driver") for p, _ in holders)
    order = ("experience", "racecraft", "awareness", "pace", "focus")
    for p, _ in holders:
        if db.get(p + ".driver") in drivers_seen:
            names = tuple(db.get(f"{p}.StatisticsDataSet_0.Statistic_{i}._metadata_") for i in range(5))
            if names != order:
                problems.append(f"{p}: statistics in order {names}")
    for d in drivers_seen:
        if by_driver[d] != 1:
            problems.append(f"driver {d} has {by_driver[d]} statistics holders")
    print(f"{len(holders)} driver statistics holders")
    for op in grid["ops"]:
        if op["op"] != "add-row":
            continue
        table = op["table"]
        word = op["from"].rsplit(".", 1)[1].split("[")[0]
        fields = grid["fields"][f"{table}.{word}"]
        parent = table
        if "[" in table:
            name, label = table.rsplit("[_metadata_=", 1)
            base, w = name.rsplit(".", 1)
            parent = db.find(base, w, label.rstrip("]"))
        src_label = op["from"].rsplit("[_metadata_=", 1)[1].rstrip("]")
        src = db.find(parent, word, src_label)
        new = db.find(parent, word, op["set"]["_metadata_"])
        if not (src and new):
            problems.append(f"{op['set']['_metadata_']}: row not found in {parent}")
            continue
        missing = [f for f in fields if db.has(f"{src}.{f}") and not db.has(f"{new}.{f}")]
        extra = [f for f in fields if db.has(f"{new}.{f}") and not db.has(f"{src}.{f}")]
        if missing or extra:
            problems.append(f"{op['set']['_metadata_']}: missing {missing} extra {extra}")
    print(f"{sum(1 for o in grid['ops'] if o['op'] == 'add-row')} added rows checked")
    for p in problems:
        print("PROBLEM:", p)
    print("ok" if not problems else f"{len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
