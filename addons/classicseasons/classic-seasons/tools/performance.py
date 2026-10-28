#!/usr/bin/env python3
"""Real season pace for data/performance.json, from the Jolpica F1 API (the Ergast successor).

For each season of data/seasons.json the tool reads every qualifying session. A driver's time in a round is his best
lap over Q1, Q2 and Q3. A team's gap in a round is its faster car's gap to pole, in percent, and the team's pace is the
median of its gaps over the season. A driver's pace is the median, over the rounds he and his teammate both set a
time, of his gap to that teammate in percent (negative is faster). Teams match data/seasons.json by constructor
(CONSTRUCTORS), drivers by surname. API responses are cached in build/jolpica/.

Each driver also gets a pace rating per season on F1 25's 0-100 scale, the least-squares fit of: his rating minus his
teammate's equals -K times his gap to him (weight 1), his rating equals his rating the next season (CARRY), and in
the last season he races his rating equals his stock F1 25 pace (data/stock_pace.json, ANCHOR). K, the rating points
per 1 % of lap time, is the value in KS that fits the anchors best.

    python3 tools/performance.py [--refresh]
"""

import argparse
import json
import statistics
import sys
import time
import unicodedata
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "build" / "jolpica"
OUT = ROOT / "data" / "performance.json"
STOCK_PACE = json.loads((ROOT / "data" / "stock_pace.json").read_text())
CARRY, ANCHOR = 0.35, 0.6
KS = range(6, 31)
PACE = (55.0, 97.0)
API = "https://api.jolpi.ca/ergast/f1"
DATA = json.loads((ROOT / "data" / "seasons.json").read_text())
DATA_BY_YEAR = {s["year"]: s for s in DATA["seasons"]}
CONSTRUCTORS = {
    "mercedes": "mercedes", "red_bull": "redbull", "ferrari": "ferrari", "williams": "williams",
    "force_india": "force_india", "racing_point": "force_india", "aston_martin": "force_india",
    "renault": "lotus", "alpine": "lotus", "toro_rosso": "toro_rosso", "alphatauri": "toro_rosso", "rb": "toro_rosso",
    "sauber": "sauber", "alfa": "sauber", "mclaren": "mclaren", "haas": "haas", "manor": "manor",
}


def fetch(year, refresh):
    path = CACHE / f"qualifying-{year}.json"
    if path.exists() and not refresh:
        return json.loads(path.read_text())
    races, offset = {}, 0
    while True:
        url = f"{API}/{year}/qualifying.json?limit=100&offset={offset}"
        with urllib.request.urlopen(url, timeout=60) as r:
            data = json.load(r)["MRData"]
        for race in data["RaceTable"]["Races"]:
            races.setdefault(race["round"], []).extend(race["QualifyingResults"])
        offset += int(data["limit"])
        if offset >= int(data["total"]):
            break
        time.sleep(0.4)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(races))
    return races


def seconds(t):
    if not t:
        return None
    m, _, s = t.rpartition(":")
    return int(m or 0) * 60 + float(s)


def plain(name):
    return unicodedata.normalize("NFKD", name).encode("ascii", "ignore").decode().lower().replace(" ", "_")


def team_keys(season):
    return {t.get("key", t["folder"]): t for t in season["teams"]}


def season_pace(season, races):
    keys = team_keys(season)
    surnames = {}
    for key, team in keys.items():
        for driver, _ in team["drivers"]:
            surnames[(key, driver.split("_", 1)[-1].replace("-jr", ""))] = driver
    gaps, mates = {k: [] for k in keys}, {}
    for results in races.values():
        best = {}
        for q in results:
            key = CONSTRUCTORS.get(q["Constructor"]["constructorId"])
            times = [seconds(q.get(s)) for s in ("Q1", "Q2", "Q3")]
            times = [t for t in times if t]
            if key not in keys or not times:
                continue
            surname = plain(q["Driver"]["familyName"])
            driver = surnames.get((key, surname)) or next(
                (d for (k, s), d in surnames.items() if k == key and (s in surname or surname in s)), None)
            best.setdefault(key, {})[driver or f"?{surname}"] = min(times)
        if not best:
            continue
        pole = min(t for team in best.values() for t in team.values())
        for key, drivers in best.items():
            gaps[key].append((min(drivers.values()) / pole - 1) * 100)
            ours = {d: t for d, t in drivers.items() if not d.startswith("?")}
            if len(ours) == 2:
                (a, ta), (b, tb) = ours.items()
                mates.setdefault(a, []).append((ta / tb - 1) * 100)
                mates.setdefault(b, []).append((tb / ta - 1) * 100)
    teams = {k: round(statistics.median(v), 3) for k, v in gaps.items() if v}
    missing = sorted(set(keys) - set(teams))
    if missing:
        sys.exit(f"{season['year']}: no qualifying times for {', '.join(missing)}")
    drivers = {d: round(statistics.median(v), 3) for d, v in mates.items()}
    return {"teams": dict(sorted(teams.items(), key=lambda kv: kv[1])), "drivers": dict(sorted(drivers.items()))}


def ratings(out, k):
    pairs, seasons = [], {}
    for year, data in out.items():
        for team in DATA_BY_YEAR[int(year)]["teams"]:
            ds = [d for d, _ in team["drivers"]]
            seasons.setdefault(int(year), []).extend(ds)
            if len(ds) == 2 and ds[0] in data["drivers"]:
                pairs.append(((ds[0], int(year)), (ds[1], int(year)), -k * data["drivers"][ds[0]]))
    last = {}
    for year, ds in seasons.items():
        for d in ds:
            last[d] = max(last.get(d, year), year)
    p = {(d, y): STOCK_PACE.get(d, 75.0) for y, ds in seasons.items() for d in ds}
    links = {v: [] for v in p}
    for a, b, diff in pairs:
        links[a].append((b, diff, 1.0))
        links[b].append((a, -diff, 1.0))
    for (d, y) in p:
        if (d, y + 1) in p:
            links[(d, y)].append(((d, y + 1), 0.0, CARRY))
            links[(d, y + 1)].append(((d, y), 0.0, CARRY))
    for _ in range(4000):
        for v in p:
            num = den = 0.0
            for other, diff, w in links[v]:
                num += w * (p[other] + diff)
                den += w
            if v[0] in STOCK_PACE and v[1] == last[v[0]]:
                num += ANCHOR * STOCK_PACE[v[0]]
                den += ANCHOR
            if den:
                p[v] = num / den
    err = sum((p[(d, last[d])] - STOCK_PACE[d]) ** 2 for d in last if d in STOCK_PACE)
    return p, err


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--refresh", action="store_true")
    args = ap.parse_args()
    out = {}
    for season in DATA["seasons"]:
        out[str(season["year"])] = season_pace(season, fetch(season["year"], args.refresh))
        print(season["year"], " ".join(f"{k}={v}" for k, v in out[str(season["year"])]["teams"].items()))
    k, (p, err) = min(((k, ratings(out, k)) for k in KS), key=lambda kv: kv[1][1])
    print(f"K = {k} rating points per 1 % of lap time, anchor error {err ** 0.5:.1f}")
    for year in out:
        out[year]["pace"] = {d: round(min(max(v, PACE[0]), PACE[1]), 1)
                             for (d, y), v in sorted(p.items()) if y == int(year)}
    OUT.write_text(json.dumps(out, indent=1) + "\n")


if __name__ == "__main__":
    main()
