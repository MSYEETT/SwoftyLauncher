#!/usr/bin/env python3
# Fake in-game bridge for rendering the launcher pages without the game: rewrites snapshot.json every
# 1.5 s and answers inbox batches with acks, as native/src/bridge.cpp does.
# usage: fake_bridge.py <game>/swofty/addons/swofty.career-editor myteam|driver|closed
import json, os, random, sys, time

box, scenario = sys.argv[1], sys.argv[2]
for d in ("inbox", "outbox"):
    os.makedirs(os.path.join(box, d), exist_ok=True)
random.seed(7)
boot = int(time.time() * 1000)

NAMES = [("max_verstappen", "VER", 1), ("yuki_tsunoda", "TSU", 22), ("lando_norris", "NOR", 4), ("oscar_piastri", "PIA", 81),
         ("charles_leclerc", "LEC", 16), ("lewis_hamilton", "HAM", 44), ("george_russell", "RUS", 63), ("kimi_antonelli", "ANT", 12),
         ("fernando_alonso", "ALO", 14), ("lance_stroll", "STR", 18), ("pierre_gasly", "GAS", 10), ("franco_colapinto", "COL", 43),
         ("alexander_albon", "ALB", 23), ("carlos_sainz", "SAI", 55), ("isack_hadjar", "HAD", 6), ("liam_lawson", "LAW", 30),
         ("esteban_ocon", "OCO", 31), ("oliver_bearman", "BEA", 87), ("nico_hulkenberg", "HUL", 27), ("gabriel_bortoleto", "BOR", 5),
         ("sergio_perez", "PER", 11), ("valtteri_bottas", "BOT", 77)]
STATS = [("experience", 0), ("racecraft", 1), ("awareness", 2), ("pace", 3), ("focus", 4)]
my_team = scenario != "driver"

drivers = []
for i, (n, ini, num) in enumerate(NAMES):
    base = 95 - i * 1.2 + random.uniform(-3, 3)
    r = {s: {"infoStatId": sid, "value": int(max(55, min(99, base + random.uniform(-6, 6)))), "base": 0.0, "curve": "base",
             "dynamic": False, "dynamicLevel": 0.0, "boost": 0.0, "min": 0.0, "max": 100.0} for s, sid in STATS}
    drivers.append({"driverStatId": 10 + i, "kind": 0, "index": 10 + i, "name": n, "initials": ini, "number": num,
                    "player": (i == 5) if not my_team else (i == 12), "ratings": r})
if my_team:
    drivers.append({"driverStatId": 16384, "kind": 1, "index": 0, "name": "jacob_nardella", "initials": "NAR", "number": 7,
                    "player": True, "ratings": {s: {"infoStatId": sid, "value": 78 + sid, "base": 0.0, "curve": "base", "dynamic": False,
                                                     "dynamicLevel": 0.0, "boost": 0.0, "min": 0.0, "max": 100.0} for s, sid in STATS}})
    drivers[12]["player"] = False

def overall(d):
    w = {"experience": 1, "racecraft": 1.5, "awareness": 1, "pace": 2, "focus": 1}
    return round(sum(d["ratings"][s]["value"] * w[s] for s in w) / sum(w.values()))

TEAMN = [(8, "mclaren"), (1, "ferrari"), (2, "redbull"), (0, "mercedes"), (4, "astonmartin"), (5, "alpine"), (7, "haas"), (6, "racingbulls"), (3, "williams"), (9, "sauber"), (10, "cadillac")]
for i, d in enumerate(drivers):
    sid, tn = TEAMN[min(i // 2, 10)]
    d["team"] = {"key": (sid << 16) | 1, "teamStatId": sid, "name": tn}
    d["seat"] = {"vehicle": (sid << 16) | (2 + i % 2), "hasCar": i < 22, "position": i % 2}
    d["onGrid"] = i < 22
if my_team:
    drivers[-1]["team"] = {"key": 0x000a000a, "teamStatId": 10, "name": "user_defined"}
    drivers[-1]["seat"] = {"vehicle": 0x000a0002, "hasCar": True, "position": 0}
    drivers[-1]["onGrid"] = True
for k, (n, ini) in enumerate([("jak_crawford", "CRA"), ("paul_aron", "ARO")]):
    drivers.append({"driverStatId": 60 + k, "kind": 0, "index": 60 + k, "name": n, "initials": ini, "number": None, "player": False,
                    "team": {"key": (k + 2) << 16 | 1, "teamStatId": k + 2, "name": TEAMN[k + 2][1]}, "seat": {"vehicle": 0, "hasCar": False, "position": None},
                    "onGrid": False, "ratings": {s: {"infoStatId": sid, "value": 70 + sid, "base": 0.0, "curve": "base", "dynamic": False,
                                                       "dynamicLevel": 0.0, "boost": 0.0, "min": 0.0, "max": 100.0} for s, sid in STATS}})
player = next(d for d in drivers if d["player"])
contracts = [{"driverStatId": d["driverStatId"], "oldTeamStatId": d["team"]["teamStatId"], "newTeamStatId": d["team"]["teamStatId"],
              "salary": 1_000_000 + 250_000 * (i % 9), "signingBonus": 200_000 * (i % 3), "teamPerfMoney": 0, "teamPerfRaceValue": 0,
              "successChance": 0.75, "negotiationBonus": 0, "isCounteroffer": False, "reason": 0, "weekendIndex": 3, "careerDayIndex": 40,
              "dayContractSent": 38, "dayContractDecided": 40, "accepted": True, "acknowledged": True, "clause": None}
             for i, d in enumerate(drivers[:6])]
moves = [{"driverStatId": 22, "oldTeamStatId": 3, "newTeamStatId": 3, "reason": 3, "weekendIndex": 4}]
adjustments = []
renown = 412.5
sponsor_loyalty = 340
costcap = {"expenses": 96_400_000, "cost_cap": 140_000_000}
fac_levels = [2, 1, 3]
team_key = 0x000a000a if my_team else 0x00030003

DEPT = ["aero", "chassis", "durability", "powertrain"]
PARTS = {"aero": ["front_wing", "rear_wing", "floor", "sidepods", "diffuser", "beam_wing"],
         "chassis": ["suspension", "weight_reduction", "cooling", "brake_ducts", "steering"],
         "durability": ["gearbox_wear", "engine_wear", "ers_wear", "brake_wear"],
         "powertrain": ["ice_power", "ers_deploy", "turbo", "fuel_flow", "mguk"]}
def upgrades(seed):
    rr = random.Random(seed); out = []
    for di, d in enumerate(DEPT):
        for p in PARTS[d]:
            for lvl in (1, 2, 3):
                st = rr.choice([3, 3, 3, 1, 1, 0, 2, 8, 6]) if lvl == 1 else rr.choice([1, 0, 0, 2, 3, 7])
                out.append({"hash": rr.randrange(1 << 31), "name": f"{d}_{p}_{lvl}", "tree": 0, "dept": di, "type": 3, "size": lvl,
                            "supplier": False, "state": st, "state_name": "", "fitted": 1 if st == 3 else 0, "enabled": True,
                            "facility_locked": lvl == 3 and rr.random() < 0.4, "unlock_facility_level": 3 if lvl == 3 else 0, "day": 40,
                            "research_cost": 600 * lvl + rr.randrange(0, 400, 50), "develop_cost": 400 * lvl, "research_days": 6 + 2 * lvl,
                            "develop_days": 3 + lvl, "research_paid": 0, "develop_paid": 0})
    return out
def counts(ups):
    c = {"total": len(ups), "unavailable": 0, "available": 0, "researching": 0, "researched": 0, "developing": 0,
         "pending_fitting": 0, "fitted": 0, "failed": 0, "locked": 0}
    m = {0: "unavailable", 1: "available", 2: "researching", 3: "fitted", 4: "failed", 6: "researched", 7: "developing", 8: "pending_fitting"}
    for u in ups:
        c[m.get(u["state"], "failed")] += 1
        c["locked"] += u["facility_locked"]
    return c
def depts(ups):
    out = []
    for di, d in enumerate(DEPT):
        us = [u for u in ups if u["dept"] == di]
        out.append({"dept": di, "name": d, "total": len(us), "available": sum(u["state"] == 1 for u in us),
                    "in_progress": sum(u["state"] in (2, 7) for u in us), "fitted": sum(u["state"] == 3 for u in us),
                    "locked": sum(u["facility_locked"] for u in us)})
    return out
teams = []
for t in range(10):
    key = team_key if t == 0 else ((t + 1) << 16 | (t + 1))
    ups = upgrades(t)
    teams.append({"team": key, "team_stat_id": key >> 16, "player": t == 0, "mode": 1, "owner": player["driverStatId"],
                  "counts": counts(ups), "departments": depts(ups), "upgrades": ups})

cash = 48_250_000
rp = 3_420
history = [{"type": 27, "type_name": "eMyTeamReward", "amount": 2_000_000, "day": 70}, {"type": 3, "type_name": "eSponsorIncome", "amount": 6_500_000, "day": 77},
           {"type": 5, "type_name": "eFacilityUpkeep", "amount": -1_250_000, "day": 84}, {"type": 9, "type_name": "eDriverSalary", "amount": -3_400_000, "day": 84},
           {"type": 3, "type_name": "eSponsorIncome", "amount": 6_500_000, "day": 91}]
TRACKS = ["rw_australia", "rw_china", "rw_japan", "rw_bahrain", "rw_saudi_arabia", "rw_miami", "rw_imola", "rw_monaco", "rw_spain", "rw_canada",
          "rw_austria", "rw_great_britain", "rw_belgium", "rw_hungary", "rw_netherlands", "rw_italy", "rw_azerbaijan", "rw_singapore",
          "rw_usa", "rw_mexico", "rw_brazil", "rw_las_vegas", "rw_qatar", "rw_abu_dhabi"]
cur = 8
order = list(TRACKS)

def short_u(u):
    o = {"h": u["hash"], "n": u["name"], "d": u["dept"], "s": u["state"], "c": u["research_cost"], "t": u["research_days"], "dc": u["develop_cost"], "dt": u["develop_days"]}
    if u["state"] in (2, 7):
        o["b"], o["e"] = 90, 100
    if u["facility_locked"]:
        o["l"], o["r"], o["ul"] = 1, "facility_locked", 3
    elif u["state"] == 0:
        o["r"] = "prerequisites"
    return o
def short_depts(t):
    return [{"d": d["dept"], "t": d["total"], "f": d["fitted"], "p": d["in_progress"], "a": d["available"], "l": d["locked"]} for d in t["departments"]]
def rnd_full(t):
    return {"team": t["team"], "team_stat_id": t["team_stat_id"], "player": t["player"], "mode": 1, "owner": t["owner"], "counts": t["counts"],
            "depts": short_depts(t), "u": [short_u(u) for u in t["upgrades"]]}
def rnd_summary(t):
    return {"team": t["team"], "team_stat_id": t["team_stat_id"], "player": t["player"], "fitted_ratio": t["counts"]["fitted"] / t["counts"]["total"],
            "counts": t["counts"], "depts": short_depts(t)}

def standings():
    ds = []
    for i, d in enumerate(drivers):
        pts = max(0, int(190 - i * 9 + random.Random(i).uniform(-12, 12)))
        ds.append({"key": d["driverStatId"] << 16, "driver_stat_id": d["driverStatId"], "team_stat_id": i // 2 + 1, "points": pts,
                   "wins": max(0, 3 - i // 2), "podiums": max(0, 6 - i // 2), "results": []})
    ds.sort(key=lambda x: -x["points"])
    for p, d in enumerate(ds):
        d["position"] = p + 1
    ts = {}
    for d in ds:
        t = ts.setdefault(d["team_stat_id"], {"team_stat_id": d["team_stat_id"], "points": 0, "wins": 0})
        t["points"] += d["points"]; t["wins"] += d["wins"]
    tl = sorted(ts.values(), key=lambda t: -t["points"])
    for p, t in enumerate(tl):
        t["position"] = p + 1
    for d in ds:
        d["points"] += sum(a["points"] for a in adjustments if a["driver_stat_id"] == d["driver_stat_id"])
    return {"ready": True, "season_index": 0, "rounds_scored": cur - 1, "drivers": ds, "teams": tl, "adjustments": adjustments,
            "adjustments_pending": False, "career_id": "x"}

def weather():
    sess = {}
    for i, k in enumerate(["p1", "p2", "p3", "q", "ss", "r", "r2", "r3"]):
        active = k in ("p1", "p2", "p3", "q", "r")
        st = [{"state": "light_cloud", "intensity": 0.2, "end": 0.4, "transition": 0.02}, {"state": "overcast", "intensity": 0.4, "end": 1.0, "transition": 0.0}] if active else []
        if k == "r":
            st = [{"state": "overcast", "intensity": 0.3, "end": 0.26, "transition": 0.017}, {"state": "light_rain", "intensity": 0.5, "end": 0.44, "transition": 0.03},
                  {"state": "heavy_rain", "intensity": 0.9, "end": 0.7, "transition": 0.03}, {"state": "overcast", "intensity": 0.3, "end": 1.0, "transition": 0}]
        sess[k] = {"dataset": i, "active": active, "length_s": 7200.0 if k.startswith("r") else 3600.0, "progress": 0.0, "started": k in ("p1", "p2"),
                   "locked": k in ("p1", "p2"), "planned": k in ("r", "q"), "rebuilt": k == "r",
                   "not_applied": "forecast not built yet (weekend hub not entered)" if k == "q" else None, "dynamic": True, "states": st}
    return {"available": True, "weekend": cur - 1, "forecast_weekend": cur - 1, "default_weekend": cur - 1,
            "seed": {"base": 1234567, "forced": 0, "tod": 1234597, "locked": False, "pinned": None}, "temp_supported": False, "active_dataset": 2,
            "types": [], "sessions": sess, "plan": {"weekend": cur - 1, "sessions": {"r": {"laps": 78, "timeline": []}}}, "plans": [cur - 1]}

journal_t = 0
def snapshot():
    for d in drivers:
        d["overall"] = overall(d)
    safe = scenario in ("myteam", "fresh")
    status = {"game": "running", "career": True, "mode": "my_team" if my_team else "driver_career", "career_id": f"mt-{team_key:08x}-{player['driverStatId']:08x}" if my_team else f"dc-{player['driverStatId']:08x}",
              "humans": 1, "phase": "team_hq" if safe else "weekend_hub", "safe": safe, "anytime": True,
              "why": None if safe else "a session is running; edits wait for Team HQ", "why_anytime": None,
              "season": 1, "year": 2026, "round": cur, "rounds": 24, "day": 96, "weekend": order[cur - 1],
              "player": {"driver": player["driverStatId"] << 16, "team": team_key},
              "detect": {"components": "live", "component_mode": "my_team" if my_team else "driver_career", "team_hq_answer": "my_team" if my_team else "driver_career", "flow_career": True},
              "state": "StateTeamHQUpdate" if safe else "SessionStateCareerHubUpdate", "transitions": 812,
              "flow": ([{"state": "StateTeamHQ", "status": "Updating", "since": boot - 84000}, {"state": "StateTeamHQUpdate", "status": "Updating", "since": boot - 5300}] if safe
                       else [{"state": "StateSession", "status": "Updating", "since": boot - 125000}, {"state": "SessionStateCareerHubUpdate", "status": "Updating", "since": boot - 9300}])}
    ops = [("economy.cash.set", False), ("economy.cash.add", False), ("economy.rp.set", False), ("economy.rp.add", False),
           ("drivers.ratings.set", False), ("drivers.contract.set", False), ("drivers.swap", False), ("rnd.complete", False), ("rnd.team_detail", True), ("rnd.unlock", False),
           ("rnd.reset", False), ("rnd.refund", False), ("rnd.fit", False), ("facilities.complete", False), ("facilities.set_level", False),
           ("season.calendar.reorder", False), ("season.calendar.set_round", False), ("season.calendar.add_round", False),
           ("season.calendar.remove_round", False), ("season.calendar.toggle_sprint", False), ("season.points.adjust", False), ("season.points.clear", False),
           ("season.result.set", False), ("economy.renown.set", False), ("economy.renown.add", False), ("economy.sponsor.loyalty.add", False),
           ("economy.costcap.refund", False), ("weather.plan.set", True), ("weather.plan.clear", True), ("weather.seed.lock", True),
           ("weather.seed.reroll", True), ("weather.seed.unlock", True)]
    sections = {
        "economy": {"available": True, "mode": status["mode"], "player": status["player"],
                    "cash": {"balance": cash, "team": team_key, "accounts": [{"category": 1, "balance": cash, "min": 0}], "history": history[-20:]} if my_team else None,
                    "rp": {"wallet": rp, "per": "team" if my_team else "driver", "key": team_key, "unlimited": False},
                    "cost_cap": {"current_season": 0, "seasons": [{"season": 0, "expenses": costcap["expenses"], "cost_cap": costcap["cost_cap"], "modifier": 1.0}]} if my_team else None,
                    "sponsors": {"primary": 3, "list": [{"id": 3, "name": "sponsor_primary_vortex", "active": True, "slot": 0, "loyalty": sponsor_loyalty, "level": 2, "max_loyalty": 1000},
                                                        {"id": 7, "name": "sponsor_secondary_helix", "active": True, "slot": 1, "loyalty": 120, "level": 1, "max_loyalty": 600},
                                                        {"id": 9, "name": "", "active": False, "slot": 2, "loyalty": 0, "level": 0, "max_loyalty": 600}]} if my_team else None,
                    "renown": {"points": renown, "level": sum(renown >= t for t in (100, 250, 500, 1000)), "tiers": [100, 250, 500, 1000]} if my_team else None},
        "drivers": {"available": True, "reason": "", "component": "DriverStatisticsComponent", "progress": 0.31,
                    "patch": {"appliedChecksum": 0, "declinedChecksum": 0, "usesPatchedCurves": False},
                    "stats": [{"infoStatId": sid, "name": s, "weight": 1.0, "default": 50.0} for s, sid in STATS], "drivers": drivers,
                    "contracts": {"pending": [], "decided": contracts, "pendingMoves": [], "lastSeasonSignings": []} if my_team else None,
                    "recordedMoves": moves[-20:], "pendingSwaps": []},
        "rnd": {"ready": True, "day": 96, "player_team": team_key, "lifecycle": "research_develop_fit",
                "player": rnd_full(teams[0]), "teams": [rnd_summary(t) for t in teams]},
        "season": {"ready": True, "season_index": 0, "season_number": 1, "seasons": 1, "name": "2026", "year": 2026, "round": cur, "rounds": 24,
                   "weekend": order[cur - 1], "completion": 0, "season_completion": 0, "current_minutes": 96 * 1440, "day": 96},
        "standings": standings(),
        "calendar": {"ready": True, "season_index": 0, "current_round": cur, "editable_from": cur + 1,
                     "rounds": [{"round": i + 1, "weekend": w, "track": w[3:], "completed": i + 1 < cur, "current": i + 1 == cur, "editable": i + 1 > cur, "state": 0 if i + 1 > cur else 2,
                                 "sprint": (i % 4 == 1) if i + 1 < cur else None, "sessions": []} for i, w in enumerate(order)]},
        "weather": weather(),
    }
    if my_team:
        def fac(ty, level, built, total, building):
            nodes = []
            for n in range(total):
                st = 3 if n < built else (2 if n == built and building else (1 if n < built + 3 else 0))
                node = {"i": ty * 100 + n, "n": f"lng_facility_{ty}_{n}", "s": st, "p": 1 + n * 4 // total, "lv": 1, "mx": 5, "c": 1500000, "dd": 14}
                if st == 2:
                    node["dl"] = 4
                nodes.append(node)
            return {"ty": ty, "lv": level, "hp": level, "wc": 40, "nt": total, "nb": built, "na": 3,
                    "bld": [{"i": x["i"], "dl": x["dl"], "dd": x["dd"]} for x in nodes if x["s"] == 2], "nodes": nodes}
        sections["facilities"] = {"ready": True, "available": True, "team": team_key,
            "f": [fac(0, fac_levels[0], 9, 36, True), fac(1, fac_levels[1], 5, 24, False), fac(2, fac_levels[2], 12, 20, False)],
            "teams": [{"team": team_key, "player": True, "lv": [[i, l] for i, l in enumerate(fac_levels)]}]}
    if scenario == "driver":
        del sections["weather"]
    pending = sorted(int(f[:-5]) for f in os.listdir(os.path.join(box, "inbox")) if f.endswith(".json") and f[:-5].isdigit())
    return {"v": 1, "written": int(time.time() * 1000), "boot": boot, "pid": 4242, "addon": {"id": "swofty.career-editor", "version": "0.1.0", "api": 5},
            "status": status,
            "bridge": {"pending": pending, "blocked": {"seq": pending[0], "why": status["why"]} if pending and not safe else None, "last_ack": None, "applied": 3, "failed": 0,
                       "ops": [{"name": n, "anytime": a, "off": None} for n, a in ops], "sections": [{"name": k, "off": None} for k in sections], "duplicates": []},
            "sections_live": True, "sections_at": int(time.time() * 1000), "sections": sections}

def write(path, obj):
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(obj, f)
    os.replace(tmp, path)

def apply(op, args):
    global cash, rp, renown, sponsor_loyalty
    if op == "economy.cash.set":
        b = {"cash": cash, "team": team_key}; cash = int(args["value"]); history.append({"type": 27, "type_name": "eMyTeamReward", "amount": cash - b["cash"], "day": 96})
        return True, None, b, {"cash": cash, "team": team_key}
    if op == "economy.cash.add":
        b = {"cash": cash, "team": team_key}; cash += int(args["amount"]); history.append({"type": 27, "type_name": "eMyTeamReward", "amount": int(args["amount"]), "day": 96})
        return True, None, b, {"cash": cash, "team": team_key}
    if op in ("economy.rp.set", "economy.rp.add"):
        b = {"rp": rp, "key": team_key, "per": "team"}; rp = int(args["value"]) if op.endswith("set") else rp + int(args["amount"])
        return True, None, b, {"rp": rp, "key": team_key, "per": "team"}
    if op == "drivers.ratings.set":
        d = next(x for x in drivers if x["driverStatId"] == args["driverStatId"])
        b = {"driverStatId": d["driverStatId"], "ratings": {k: {"value": d["ratings"][k]["value"]} for k in args["ratings"]}, "overall": overall(d)}
        for k, v in args["ratings"].items():
            d["ratings"][k]["value"] = int(v)
        return True, None, b, {"driverStatId": d["driverStatId"], "ratings": {k: {"value": d["ratings"][k]["value"]} for k in args["ratings"]}, "overall": overall(d)}
    if op == "season.calendar.reorder":
        b = {"rounds": list(order)}; order[cur:] = args["order"]
        return True, None, b, {"rounds": list(order)}
    if op == "drivers.swap":
        a = next(x for x in drivers if x["driverStatId"] == args["a"]); b = next(x for x in drivers if x["driverStatId"] == args["b"])
        if a["player"] or b["player"]:
            return False, "player_locked: the player's seat can't move", None, None
        if a["team"]["teamStatId"] == b["team"]["teamStatId"] and a["seat"]["hasCar"] == b["seat"]["hasCar"]:
            return False, "same_team: both drive for the same team", None, None
        bf = {"a": {"driverStatId": a["driverStatId"], "teamStatId": a["team"]["teamStatId"], "seat": dict(a["seat"])}, "b": {"driverStatId": b["driverStatId"], "teamStatId": b["team"]["teamStatId"], "seat": dict(b["seat"])}}
        a["team"], b["team"] = b["team"], a["team"]; a["seat"], b["seat"] = b["seat"], a["seat"]; a["onGrid"], b["onGrid"] = b["onGrid"], a["onGrid"]
        moves.append({"driverStatId": a["driverStatId"], "oldTeamStatId": bf["a"]["teamStatId"], "newTeamStatId": a["team"]["teamStatId"], "reason": 0, "weekendIndex": cur - 1})
        moves.append({"driverStatId": b["driverStatId"], "oldTeamStatId": bf["b"]["teamStatId"], "newTeamStatId": b["team"]["teamStatId"], "reason": 0, "weekendIndex": cur - 1})
        return True, None, bf, {"a": {"driverStatId": a["driverStatId"], "teamStatId": a["team"]["teamStatId"], "seat": a["seat"]}, "b": {"driverStatId": b["driverStatId"], "teamStatId": b["team"]["teamStatId"], "seat": b["seat"]}, "appliedNow": True}
    if op == "drivers.contract.set":
        c = next(x for x in contracts if x["driverStatId"] == args["driverStatId"]); bf = dict(c)
        for f in ("salary", "signingBonus", "teamPerfMoney", "teamPerfRaceValue"):
            if f in args:
                c[f] = int(args[f])
        return True, None, bf, dict(c)
    if op in ("economy.renown.set", "economy.renown.add"):
        bf = {"renown": renown, "level": 2, "team": team_key}; renown = float(args["value"]) if op.endswith("set") else renown + float(args["amount"])
        return True, None, bf, {"renown": renown, "level": 3, "team": team_key}
    if op == "economy.sponsor.loyalty.add":
        bf = {"team": team_key, "sponsor": 3, "name": "sponsor_primary_vortex", "loyalty": sponsor_loyalty, "level": 2}; sponsor_loyalty += int(args["amount"])
        return True, None, bf, dict(bf, loyalty=sponsor_loyalty)
    if op == "economy.costcap.refund":
        if args["amount"] > costcap["expenses"]:
            return False, f"bad_args: at most {costcap['expenses']}", None, None
        bf = {"season": 0, "expenses": costcap["expenses"], "cost_cap": costcap["cost_cap"], "cash": cash}
        costcap["expenses"] -= int(args["amount"]); cash += int(args["amount"])
        return True, None, bf, {"season": 0, "expenses": costcap["expenses"], "cost_cap": costcap["cost_cap"], "cash": cash}
    if op == "season.points.adjust":
        d = args["driver"]; prev = next((a for a in adjustments if a["driver_stat_id"] == d), None)
        bf = {"driver_stat_id": d, "key": d << 16, "points": 0, "adjustment": prev["points"] if prev else 0, "pending": False}
        if prev:
            prev["points"] += int(args["delta"])
        else:
            adjustments.append({"driver_stat_id": d, "key": d << 16, "points": int(args["delta"]), "live": True, "pending": False})
        return True, None, bf, dict(bf, adjustment=bf["adjustment"] + int(args["delta"]))
    if op == "season.points.clear":
        bf = {"adjustments": [dict(a) for a in adjustments if "driver" not in args or a["driver_stat_id"] == args["driver"]]}
        adjustments[:] = [a for a in adjustments if "driver" in args and a["driver_stat_id"] != args["driver"]]
        return True, None, bf, {"adjustments": []}
    if op == "season.calendar.set_round":
        bf = {"rounds": list(order)}; order[args["round"] - 1] = args["weekend"]
        return True, None, bf, {"rounds": list(order)}
    if op == "season.calendar.add_round":
        if args["weekend"] in order:
            return False, "duplicate_weekend: already in this season", None, None
        bf = {"rounds": list(order)}; order.insert(args.get("at", len(order) + 1) - 1, args["weekend"])
        return True, None, bf, {"rounds": list(order)}
    if op == "season.calendar.remove_round":
        bf = {"rounds": list(order)}; del order[args["round"] - 1]
        return True, None, bf, {"rounds": list(order)}
    if op in ("season.result.set", "season.calendar.toggle_sprint"):
        return False, "unsupported: not available in this build", None, {"detail": "phase 3"}
    if op == "facilities.set_level":
        bf = {"team": team_key, "f": [{"ty": i, "lv": l} for i, l in enumerate(fac_levels)]}
        fac_levels[int(args["facility"])] = int(args["level"])
        return True, None, bf, {"team": team_key, "f": [{"ty": i, "lv": l} for i, l in enumerate(fac_levels)]}
    if op == "facilities.complete":
        return False, "nothing_building: no build is in progress", None, None
    if op == "weather.plan.set" and "q" in args.get("sessions", {}):
        return True, None, {"weekend": cur - 1, "sessions": {"q": None}}, {"weekend": cur - 1, "sessions": args["sessions"], "not_applied": {"q": "forecast not built yet"}}
    if op.startswith("weather."):
        return True, None, {"weekend": cur - 1, "sessions": {"r": None}}, {"weekend": cur - 1, "sessions": args.get("sessions", {})}
    if op == "rnd.team_detail":
        t = next((x for x in teams if x["team"] == args.get("team")), teams[0])
        return True, None, None, rnd_full(t)
    if op.startswith("rnd."):
        return True, None, {"upgrades": [{"hash": 1, "state": 1}]}, {"upgrades": [{"hash": 1, "state": 3}]}
    return False, f"unknown op '{op}' (not in this build)", None, None

def drain():
    safe = scenario in ("myteam", "fresh")
    for f in sorted(os.listdir(os.path.join(box, "inbox"))):
        if not (f.endswith(".json") and f[:-5].isdigit()):
            continue
        p = os.path.join(box, "inbox", f)
        batch = json.load(open(p))
        anytime = all(o["op"].startswith("weather.") or o["op"] == "rnd.team_detail" for o in batch["ops"])
        if not safe and not anytime:
            continue
        res = []
        for i, o in enumerate(batch["ops"]):
            args = dict(o.get("args", {}))
            ok, err, b, a = apply(o["op"], args)
            res.append({"i": i, "op": o["op"], "ok": ok, "ran": True, "args": args, "error": err, "before": b, "after": a})
            if ok:
                with open(os.path.join(box, "journal.jsonl"), "a") as j:
                    j.write(json.dumps({"t": int(time.time() * 1000), "seq": batch["seq"], "i": i, "op": o["op"], "args": args, "before": b, "after": a}) + "\n")
        write(os.path.join(box, "outbox", f), {"seq": batch["seq"], "boot": boot, "session": batch.get("session"), "t": int(time.time() * 1000),
                                               "mode": "my_team" if my_team else "driver_career", "ok": all(r["ok"] for r in res), "ms": 1, "results": res})
        os.remove(p)

while True:
    if scenario != "closed":
        drain()
        write(os.path.join(box, "snapshot.json"), snapshot())
    time.sleep(1.5)
