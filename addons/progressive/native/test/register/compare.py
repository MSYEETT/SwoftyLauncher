#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Compares what register.cpp registered (fake_core's capture) with data/years.json: every typed field
# of every career.season and career.driver_ratings record against the value the rules give from
# years.json, then rebuilds years.json from the records alone (typed fields plus extra.progressive) and
# requires it to equal the file. Writes each record to <records>/<registry>/<key>.json and the seasons
# as one registry data file, for tools/std/validate.py.
#
#   compare.py <captured.json> <years.json> <records dir> [SDK registries dir]

import copy
import json
import math
import re
import sys
from pathlib import Path

ADDON = "swofty.progressive"
SEASON = "career.season"
RATINGS = "career.driver_ratings"
SEASON_EXTRA = ("teams", "changes", "pool", "ratings_source", "addon")

failures = []


def fail(where, message):
    failures.append(f"{where}: {message}")


def diff(where, got, want, limit=20):
    out = []

    def walk(path, a, b):
        if len(out) >= limit:
            return
        if isinstance(a, dict) and isinstance(b, dict):
            for k in sorted(set(a) | set(b), key=str):
                if k not in a:
                    out.append(f"{path}/{k}: missing")
                elif k not in b:
                    out.append(f"{path}/{k}: unexpected {json.dumps(a[k])[:120]}")
                else:
                    walk(f"{path}/{k}", a[k], b[k])
        elif isinstance(a, list) and isinstance(b, list):
            if len(a) != len(b):
                out.append(f"{path}: {len(a)} elements, want {len(b)}")
            for i, (x, y) in enumerate(zip(a, b)):
                walk(f"{path}/{i}", x, y)
        elif type(a) is not type(b) or a != b:
            out.append(f"{path}: {json.dumps(a)[:120]} != {json.dumps(b)[:120]}")

    walk("", got, want)
    for line in out:
        fail(where, line)
    return not out


def slug(name):
    return re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")


def rounded(x):
    return int(math.floor(min(max(x, 0.0), 100.0) + 0.5))


class Rules:
    def __init__(self, years):
        self.years = years
        self.stock = {p["driver"] for p in years["pool"] if p["source"] == "stock"}

    def driver(self, label):
        if not label:
            return None
        return ("game/" if label in self.stock else "person/") + label

    def label(self, ref):
        if ref is None:
            return None
        for prefix in ("game/", "person/"):
            if ref.startswith(prefix):
                return ref[len(prefix):]
        fail("driver ref", f"{ref!r} is neither game/ nor person/")
        return ref

    def team(self, t):
        return f"constructor/{slug(t['name'])}" if t["kind"] == "real" else None

    def box(self, year, season):
        own = f"classic-seasons-{year}"
        if any(t["vehicle"]["source"] == own for t in season["teams"]):
            return f"swofty.classic-seasons-{year}/f1_{year}|game/f1_2025"
        return f"game/f1_{year}"

    def season(self, key, s):
        teams = []
        for t in s["teams"]:
            seats = [None, None]
            for d in t["drivers"]:
                seats[d["position"]] = self.driver(d["driver"])
            teams.append({"slot": t["slot"], "team": self.team(t), "seats": seats})
        changes = []
        for c in s.get("changes", []):
            temporary = "until_round" in c
            changes.append({
                "round": c["round"], "slot": c["slot"], "seat": c["seat"],
                "out": self.driver(c["out"]), "in": self.driver(c["in"]),
                "kind": "stand_in" if temporary else "swap",
                "until_round": c["until_round"] if temporary else None,
            })
        return {
            "data_group": self.years["data_group"],
            "year": int(key),
            "box": self.box(int(key), s),
            "calendar": None,
            "teams": teams,
            "changes": changes,
        }

    def rating(self, year, label, v):
        return {
            "season": f"{ADDON}/{year}",
            "driver": self.driver(label),
            "experience": rounded(v["experience"]),
            "racecraft": rounded(v["racecraft"]),
            "awareness": rounded(v["awareness"]),
            "pace": rounded(v["pace"]),
        }


def rebuild(rules, seasons, ratings):
    addons = [r["progressive"]["addon"] for r in seasons]
    for i, a in enumerate(addons[1:], 1):
        if a != addons[0]:
            fail(f"{SEASON} {seasons[i]['year']}", "extra.progressive.addon differs from the first season's")
    groups = {r["data_group"] for r in seasons}
    if len(groups) != 1:
        fail(SEASON, f"data groups differ: {sorted(groups)}")
    out = copy.deepcopy(addons[0])
    out["data_group"] = seasons[0]["data_group"]
    pool = [dict(p, years={}, looks={}, numbers={}) for p in out["pool"]]
    by_driver = {p["driver"]: p for p in pool}
    out["seasons"] = {}
    sources = {}
    for r in seasons:
        y = str(r["year"])
        p = r["progressive"]
        s = {k: copy.deepcopy(v) for k, v in p.items() if k not in SEASON_EXTRA}
        teams = []
        for t in r["teams"]:
            e = copy.deepcopy(p["teams"][f"slot={t['slot']}"])
            e["slot"] = t["slot"]
            e["drivers"] = [d if "driver" in d else dict(d, driver=rules.label(t["seats"][d["position"]]))
                            for d in e["drivers"]]
            teams.append(e)
        s["teams"] = teams
        if "changes" in p:
            changes = []
            for c in r["changes"]:
                e = copy.deepcopy(p["changes"][f"round={c['round']},slot={c['slot']},seat={c['seat']}"])
                e.update(round=c["round"], slot=c["slot"], seat=c["seat"],
                         out=rules.label(c["out"]), **{"in": rules.label(c["in"])})
                if c["until_round"] is not None:
                    e["until_round"] = c["until_round"]
                changes.append(e)
            s["changes"] = changes
        for label, slice_ in p["pool"].items():
            entry = by_driver[label]
            if "slot" in slice_:
                entry["years"][y] = slice_["slot"]
            if "look" in slice_:
                entry["looks"][y] = slice_["look"]
            if "number" in slice_:
                entry["numbers"][y] = slice_["number"]
        if "ratings_source" in p:
            sources[y] = p["ratings_source"]
        out["seasons"][y] = s
    out["pool"] = pool
    out["ratings"]["sources"] = sources
    out["ratings"]["years"] = {str(r["year"]): {} for r in seasons}
    for r in ratings:
        y = r["season"][len(ADDON) + 1:]
        out["ratings"]["years"].setdefault(y, {})[rules.label(r["driver"])] = r["progressive"]
    return out


def main():
    captured = json.loads(Path(sys.argv[1]).read_text())
    years = json.loads(Path(sys.argv[2]).read_text())
    records = Path(sys.argv[3])
    rules = Rules(years)

    if captured.get("returned") != 1:
        fail("SwoftyAddonRegister", f"returned {captured.get('returned')}")
    for u in captured["unexpected"]:
        fail("registry", f"unexpected call {u}")
    declared = [d["registry"] for d in captured["declare"]]
    if sorted(declared) != sorted([SEASON, RATINGS]):
        fail("declare", f"declared {declared}, want each of {SEASON} and {RATINGS} once")
    for d in captured["declare"]:
        if not d["schema_ok"] or d["schema"].get("x-registry") != d["registry"]:
            fail(f"declare {d['registry']}", "schema is not JSON or names another registry")
            continue
        if len(sys.argv) > 4:
            sdk = json.loads((Path(sys.argv[4]) / d["registry"] / "1.0.json").read_text())
            for k in ("x-registry", "x-version", "x-applies", "x-snapshot", "x-snapshot-budget", "additionalProperties"):
                if d["schema"].get(k) != sdk.get(k):
                    fail(f"declare {d['registry']}", f"{k} {d['schema'].get(k)!r}, the SDK schema has {sdk.get(k)!r}")
            names = [k for k in sdk["properties"] if k != "extends"]
            if list(d["schema"].get("properties", {})) != names:
                fail(f"declare {d['registry']}", f"fields {list(d['schema'].get('properties', {}))}, the SDK schema has {names}")

    adds = {SEASON: [], RATINGS: []}
    for a in captured["add"]:
        if a["registry"] not in adds:
            fail("add", f"unexpected registry {a['registry']}")
            continue
        if not a["value_ok"]:
            fail(f"{a['registry']} {a['key']}", "value is not JSON")
            continue
        if a["id"] != f"{ADDON}/{a['key']}":
            fail(f"{a['registry']} {a['key']}", f"id {a['id']}")
        adds[a["registry"]].append(a)
    for name, list_ in adds.items():
        keys = [a["key"] for a in list_]
        if len(keys) != len(set(keys)):
            fail(name, "a key was added twice")

    season_keys = [a["key"] for a in adds[SEASON]]
    if season_keys != list(years["seasons"]):
        fail(SEASON, f"keys {season_keys}, want {list(years['seasons'])}")
    fields = 0
    for a in adds[SEASON]:
        s = years["seasons"].get(a["key"])
        if s is None:
            continue
        want = rules.season(a["key"], s)
        got = {k: a["value"].get(k) for k in want}
        fields += len(want)
        diff(f"{SEASON} {a['key']}", got, want)
        extra = set(a["value"]) - set(want)
        if extra != {"progressive"}:
            fail(f"{SEASON} {a['key']}", f"top-level fields beyond the schema's: {sorted(extra)}")

    want_ratings = {}
    for y, drivers in years["ratings"]["years"].items():
        for label, v in drivers.items():
            want_ratings[f"{y}/{rules.driver(label)}"] = (y, label, v)
    got_keys = [a["key"] for a in adds[RATINGS]]
    if sorted(got_keys) != sorted(want_ratings):
        fail(RATINGS, f"{len(got_keys)} keys, want {len(want_ratings)}; "
                      f"missing {sorted(set(want_ratings) - set(got_keys))[:5]}, "
                      f"extra {sorted(set(got_keys) - set(want_ratings))[:5]}")
    for a in adds[RATINGS]:
        if a["key"] not in want_ratings:
            continue
        y, label, v = want_ratings[a["key"]]
        want = rules.rating(y, label, v)
        got = {k: a["value"].get(k) for k in want}
        fields += len(want)
        diff(f"{RATINGS} {a['key']}", got, want)
        diff(f"{RATINGS} {a['key']} extra", a["value"].get("progressive"), v)
        extra = set(a["value"]) - set(want)
        if extra != {"progressive"}:
            fail(f"{RATINGS} {a['key']}", f"top-level fields beyond the schema's: {sorted(extra)}")

    if not failures:
        rebuilt = rebuild(rules, [a["value"] for a in adds[SEASON]], [a["value"] for a in adds[RATINGS]])
        diff("years.json rebuilt from the records", rebuilt, years, limit=40)

    for name, list_ in adds.items():
        folder = records / name
        folder.mkdir(parents=True, exist_ok=True)
        for a in list_:
            (folder / (a["key"].replace("/", "__") + ".json")).write_text(json.dumps(a["value"], indent=1))
    (records / "seasons.data.json").write_text(
        json.dumps({SEASON: {a["key"]: a["value"] for a in adds[SEASON]}}, indent=1))

    sizes = [a["bytes"] for a in adds[SEASON]]
    print(f"compare: {len(adds[SEASON])} {SEASON}, {len(adds[RATINGS])} {RATINGS}, {fields} typed fields checked; "
          f"season records {min(sizes) if sizes else 0}-{max(sizes) if sizes else 0} bytes")
    if failures:
        for f in failures[:60]:
            print(f"FAIL {f}")
        print(f"compare: {len(failures)} failure(s)")
        return 1
    print("compare: every typed field matches years.json, and years.json rebuilt from the records equals the file")
    return 0


if __name__ == "__main__":
    sys.exit(main())
