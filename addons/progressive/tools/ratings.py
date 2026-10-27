"""Per-year driver ratings for data/years.json (`ratings`): every driver of a season, the five statistics the game
keeps per driver (experience, racecraft, awareness, pace, focus) and the overall rating they make.

2025 and 2026 are the stock game's: the value at the start of a career (x = 0) of the driver's `default` dataset
(F1 25's 2025 ratings) and of his `2026` dataset (the 2026 season update; `default` when he has none).

2016-2024 start from Classic Seasons' per-season pace rating (classic-seasons/data/performance.json, the qualifying
gap fit anchored on the stock 2025 pace, tools/performance.py there):
    pace         the season's Classic Seasons pace (a driver without one that season: his nearest season's, else
                 his stock 2025 pace, else the season's slowest)
    racecraft,   his stock 2025 value moved by the same amount as his pace (season pace - stock 2025 pace)
    awareness
    experience   his stock 2025 value moved along the stock game's experience scale by real race starts:
                 + f(starts before the season) - f(starts before 2025), f the monotone fit of the stock 2025
                 experience of the 2025 grid over their starts before 2025 (pool-adjacent-violators, linear in
                 between); starts from data/rounds.json (2016-2025 race entries) and PRE_2016_STARTS
    focus        his stock 2025 value
A driver without stock statistics of his own (Classic Seasons adds a copy of de Vries' for him) gets racecraft,
awareness and focus from his pace with the median offsets of the stock drivers, and experience f(starts).
overall is the game's weighted mean (driver_statistics_info weighting_factor: experience 0.25, racecraft 1,
awareness 0.25, pace 2, focus 0).
"""
import re
import statistics

HOLDERS = "driver_statistics.driver_statistics"
INFO = "driver_statistics.driver_statistics.driver_statistics_info"
STATS = ("experience", "racecraft", "awareness", "pace", "focus")
LIMITS = (0.0, 99.0)
STOCK_YEARS = (2025, 2026)
PRE_2016_STARTS = {
    "lewis_hamilton": 167, "sebastian_vettel": 158, "fernando_alonso": 252, "kimi_raikkonen": 229,
    "felipe_massa": 229, "jenson_button": 284, "nico_rosberg": 185, "daniel_ricciardo": 88, "sergio_perez": 94,
    "nico_hulkenberg": 96, "romain_grosjean": 83, "valtteri_bottas": 56, "daniil_kvyat": 37, "kevin_magnussen": 19,
    "marcus_ericsson": 35, "esteban_gutierrez": 38, "carlos_sainz-jr": 19, "max_verstappen": 19, "felipe_nasr": 19,
    "robert_kubica": 76,
}


def curve_start(text):
    """The game's curve evaluator (Career_Eval_Curve) at x = 0: the y of the last point with x <= 0, scaled."""
    body = text.split("@", 2)[-1]
    head, points = {}, []
    for part in body.split(","):
        k, v = part.split(":")
        if k in ("minx", "maxx", "miny", "maxy"):
            head[k] = float(v)
        else:
            points.append((float(k), float(v)))
    y = [p[1] for p in points if p[0] <= 0][-1]
    return y * (head["maxy"] - head["miny"]) + head["miny"]


def stock_stats(db):
    out = {}
    for h in db.rows(HOLDERS, "DriverStatisticsHolder"):
        driver = db.get(f"{h}.driver")
        sets = {}
        for ds in db.rows(h, "StatisticsDataSet"):
            vals = {}
            for st in db.rows(ds, "Statistic"):
                curve = db.get(f"{st}.evolution")
                if curve and re.match(r"@v2@", curve):
                    vals[db.get(f"{st}._metadata_")] = round(curve_start(curve), 1)
            if all(s in vals for s in STATS):
                sets[db.get(f"{ds}._metadata_")] = vals
        if driver and "default" in sets:
            out[driver] = sets
    return out


def stat_infos(db):
    infos = []
    for r in db.rows(INFO, "StatisticInfo"):
        infos.append({"name": db.get(f"{r}._metadata_"), "stat_id": int(db.get(f"{r}.stat_id")),
                      "weight": float(db.get(f"{r}.weighting_factor"))})
    names = [i["name"] for i in infos]
    if sorted(names) != sorted(STATS):
        raise SystemExit(f"driver_statistics_info lists {names}, expected {list(STATS)}")
    return sorted(infos, key=lambda i: STATS.index(i["name"]))


def overall(values, infos):
    total = sum(i["weight"] for i in infos)
    return round(sum(values[i["name"]] * i["weight"] for i in infos) / total, 1)


def starts_by_year(rounds):
    out = {}
    for year, rs in rounds["years"].items():
        for r in rs:
            for entries in r["entries"].values():
                for driver, _ in entries:
                    if not driver.startswith("missing:"):
                        out.setdefault(driver, {}).setdefault(int(year), 0)
                        out[driver][int(year)] += 1
    return out


def starts_before(starts, driver, year):
    return PRE_2016_STARTS.get(driver, 0) + sum(n for y, n in starts.get(driver, {}).items() if y < year)


def monotone_fit(points):
    """Pool-adjacent-violators over (x, y) sorted by x: a non-decreasing step fit, as (x, y) knots."""
    blocks = []
    for x, y in sorted(points):
        blocks.append([x, x, y, 1])
        while len(blocks) > 1 and blocks[-2][2] > blocks[-1][2]:
            b = blocks.pop()
            a = blocks[-1]
            a[1] = b[1]
            a[2] = (a[2] * a[3] + b[2] * b[3]) / (a[3] + b[3])
            a[3] += b[3]
    return [((a + b) / 2, y) for a, b, y, _ in blocks]


def interp(knots, x):
    if x <= knots[0][0]:
        return knots[0][1]
    for (x0, y0), (x1, y1) in zip(knots, knots[1:]):
        if x <= x1:
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0) if x1 > x0 else y1
    return knots[-1][1]


def clamp(v):
    return round(min(LIMITS[1], max(LIMITS[0], v)), 1)


def season_drivers(y):
    out = []
    for t in y["teams"]:
        for d in t["drivers"]:
            out.append(d["driver"])
    for s in y.get("stand_ins", []):
        if s["driver"]:
            out.append(s["driver"])
    for c in y.get("changes", []):
        out += [c["out"], c["in"]]
    return list(dict.fromkeys(out))


def build(db, years, performance, rounds):
    stock = stock_stats(db)
    infos = stat_infos(db)
    starts = starts_by_year(rounds)
    grid_2025 = season_drivers(years["2025"])
    exp_points = [(starts_before(starts, d, 2025), stock[d]["default"]["experience"]) for d in grid_2025 if d in stock]
    knots = monotone_fit(exp_points)
    own = sorted(stock)
    offsets = {s: statistics.median(stock[d]["default"][s] - stock[d]["default"]["pace"] for d in own
                                    if d in grid_2025) for s in ("racecraft", "awareness")}
    generic_focus = statistics.median(stock[d]["default"]["focus"] for d in own if d in grid_2025)
    pace_by_driver = {}
    for y, p in performance.items():
        for d, v in p["pace"].items():
            pace_by_driver.setdefault(d, {})[int(y)] = v

    def season_pace(driver, year):
        mine = pace_by_driver.get(driver, {})
        if year in mine:
            return mine[year], "Classic Seasons pace"
        if mine:
            near = min(mine, key=lambda y: (abs(y - year), y > year))
            return mine[near], f"Classic Seasons pace of {near}"
        if driver in stock:
            return stock[driver]["default"]["pace"], "stock 2025 pace"
        season = performance.get(str(year), {}).get("pace")
        if season:
            return min(season.values()), "no pace data: the season's slowest Classic Seasons pace"
        return None, None

    out = {}
    for key in sorted(years, key=int):
        year = int(key)
        rows = {}
        for driver in season_drivers(years[key]):
            if driver.startswith("missing:"):
                continue
            if year in STOCK_YEARS:
                sets = stock.get(driver)
                if not sets:
                    continue
                v = dict(sets["2026"] if year == 2026 and "2026" in sets else sets["default"])
                source = "stock 2026 dataset" if year == 2026 and "2026" in sets else "stock default (2025) dataset"
            else:
                pace, how = season_pace(driver, year)
                if pace is None:
                    continue
                n = starts_before(starts, driver, year)
                if driver in stock:
                    base = stock[driver]["default"]
                    delta = pace - base["pace"]
                    v = {"experience": base["experience"] + interp(knots, n) - interp(
                             knots, starts_before(starts, driver, 2025)),
                         "racecraft": base["racecraft"] + delta, "awareness": base["awareness"] + delta,
                         "pace": pace, "focus": base["focus"]}
                    source = f"{how}; others from stock 2025 moved by {delta:+.1f} pace and {n} starts"
                else:
                    v = {"experience": interp(knots, n), "racecraft": pace + offsets["racecraft"],
                         "awareness": pace + offsets["awareness"], "pace": pace, "focus": generic_focus}
                    source = f"{how}; no stock statistics: others from pace (median stock offsets) and {n} starts"
            v = {s: clamp(v[s]) for s in STATS}
            rows[driver] = {**v, "overall": overall(v, infos), "source": source}
        out[key] = rows
    return {"stats": infos, "experience_scale": [[round(x, 1), round(y, 2)] for x, y in knots], "years": out}
