#!/usr/bin/env python3
import json
import re
import sys
from pathlib import Path

from season import Season, slots

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parents[1]
DM_SRC = REPO / "addons/driver-manager/launcher/src"

LIST = "entrants.entrant_lists.entrant_list[_metadata_=f1_2026]"
POOL = "entrants.entrant_lists.entrant_list[_metadata_=f1_2026_driver_pool]"
DRIVERS = "entrants.drivers"
DRIVER_FROM = "entrants.drivers.driver[_metadata_=jarno_opmeer]"
LOOKS = "entrants.driver_customisations.folder[_metadata_=ai_driver_customisations]"
LOOK_FROM = LOOKS + ".preloaded_driver_customisation_properties[_metadata_=official_driver_customisation_for_driver_pool]"
STATS_TABLE = "driver_statistics.driver_statistics"
STATS_FROM = STATS_TABLE + ".DriverStatisticsHolder[_metadata_=jarno_opmeer]"
ENTRANTS = "entrants.entrant_lists.entrants"
ENTRANT_FROM = ENTRANTS + ".entrant[_metadata_=jarno_opmeer]"
TEAM_STANDINGS = "entrants.standings.standings[_metadata_=f1_teams_2025]"
DRIVER_STANDINGS = "entrants.standings.standings[_metadata_=f1_drivers_2025]"

STATS = ("experience", "racecraft", "awareness", "pace", "focus")
TRAITS = ("aggression", "control", "mistakes", "reaction_speed", "speed_in", "speed_out")
LOOK_FIELDS = ["_metadata_", "available", "badge", "car_livery", "gloves", "helmet", "linked_driver_stat_id",
               "podium_emote", "pose", "stat_id", "suit", "victory_vo"]
ENTRANT_FIELDS = ["_metadata_", "badge_customisation", "car_number", "driver", "driver_customisation",
                  "entrant_reference", "team", "team_position", "vehicle", "vehicle_customisation"]

STOCK = {
    "ayrton_senna": ("ayrton_senna", "classic-senna01_ayrton_senna"),
    "alain_prost": ("alain_prost", "classic-prost01_alain_prost"),
    "nigel_mansell": ("nigel_mansell", "classic-mansell01_nigel_mansell"),
}
NEW_STATS_HOLDER = {"ayrton_senna"}

DRIVER_IDS = [18, 37, 60, 61, 104, 105, 106, 107, 108, 119, 120, 124] + list(range(253, 236, -1))
FIRST_PRESET = 1990

SUBSTITUTES = ["david_brabham", "emanuele_pirro", "alex_caffi", "roberto_moreno", "johnny_herbert"]

DRIVER_INFO = {
    "ayrton_senna": ("SEN", 1984, (1960, 3, 21), None, None),
    "alain_prost": ("PRO", 1980, (1955, 2, 24), None, None),
    "nigel_mansell": ("MAN", 1980, (1953, 8, 8), None, None),
    "gerhard_berger": ("BER", 1984, (1959, 8, 27), "male_ls204", "tier03_livery101"),
    "thierry_boutsen": ("BOU", 1983, (1957, 7, 13), "male_ls222", "tier01_livery76"),
    "riccardo_patrese": ("PAT", 1977, (1954, 4, 17), "male_ls240", "tier02_livery95"),
    "alessandro_nannini": ("NAN", 1986, (1959, 7, 7), "male_ls248", "tier02_livery104"),
    "nelson_piquet": ("PIQ", 1978, (1952, 8, 17), "pedro_piquet", "tier01_livery79"),
    "satoru_nakajima": ("NAK", 1987, (1953, 2, 23), "male_ls345", "tier05_livery73"),
    "jean_alesi": ("ALE", 1989, (1964, 6, 11), "giuliano_alesi", "tier04_livery103"),
    "derek_warwick": ("WAR", 1981, (1954, 8, 27), "male_ls356", "tier01_livery75"),
    "martin_donnelly": ("DON", 1989, (1964, 3, 26), "male_ls526", "tier02_livery82"),
    "mauricio_gugelmin": ("GUG", 1988, (1963, 4, 20), "male_ls758", "tier03_livery105"),
    "ivan_capelli": ("CAP", 1985, (1963, 5, 24), "male_ls161", "tier01_livery80"),
    "eric_bernard": ("BRN", 1989, (1964, 8, 24), "male_ls212", "tier01_livery78"),
    "aguri_suzuki": ("SUZ", 1988, (1960, 9, 8), "male_ls197", "tier02_livery103"),
    "nicola_larini": ("LAR", 1987, (1964, 3, 19), "male_ls235", "tier03_livery99"),
    "philippe_alliot": ("ALL", 1984, (1954, 7, 27), "male_ls343", "tier02_livery01"),
    "pierluigi_martini": ("MAR", 1984, (1961, 4, 23), "male_ls807", "tier01_livery52"),
    "paolo_barilla": ("BAR", 1989, (1961, 4, 20), "male_ls206", "tier01_livery88"),
    "gregor_foitek": ("FOI", 1989, (1965, 3, 27), "male_ls247", "tier01_livery77"),
    "stefano_modena": ("MOD", 1987, (1963, 5, 12), "male_ls265", "tier03_livery107"),
    "gianni_morbidelli": ("MOR", 1989, (1968, 1, 13), "male_ls553", "tier01_livery86"),
    "andrea_de_cesaris": ("DEC", 1980, (1959, 5, 31), "male_ls683", "tier02_livery94"),
    "michele_alboreto": ("ALB", 1981, (1956, 12, 23), "male_ls766", "tier04_livery105"),
    "bernd_schneider": ("SCH", 1988, (1964, 7, 20), "male_ls790", "tier04_livery20"),
    "david_brabham": ("BRA", 1990, (1965, 9, 5), "male_ls752", "tier04_livery94"),
    "emanuele_pirro": ("PIR", 1989, (1962, 1, 12), "male_ls533", "tier03_livery106"),
    "alex_caffi": ("CAF", 1986, (1964, 3, 18), "male_ls515", "tier05_livery74"),
    "roberto_moreno": ("MRN", 1982, (1959, 2, 11), "male_ls487", "tier02_livery84"),
    "johnny_herbert": ("HER", 1989, (1964, 6, 25), "male_ls355", "tier04_livery99"),
}
VOICES = {"johnny_herbert": "Name175"}
SUB_SUITS = {"david_brabham": "brabham", "emanuele_pirro": "dallara", "alex_caffi": "arrows",
             "roberto_moreno": "benetton", "johnny_herbert": "lotus"}

TEAM_INFO = {
    "mclaren": ("McLaren Honda", "McLaren", "MCL", ("#E2001A", "#FFFFFF", "#1A1A1A"), "Woking, UK", "britain",
                "MP4/5B", "Honda V10", "mclaren", "Ron Dennis"),
    "ferrari": ("Ferrari", "Ferrari", "FER", ("#A71F22", "#FFFFFF", "#1A1A1A"), "Maranello, Italy", "italy",
                "641", "Ferrari V12", "ferrari", "Cesare Fiorio"),
    "williams": ("Williams Renault", "Williams", "WIL", ("#16427C", "#FFFFFF", "#F2C200"), "Didcot, UK", "britain",
                 "FW13B", "Renault V10", "williams", "Frank Williams"),
    "benetton": ("Benetton Ford", "Benetton", "BEN", ("#1F9A48", "#FFFFFF", "#1E4FA0"), "Witney, UK", "britain",
                 "B190", "Ford V8", "renault", "Flavio Briatore"),
    "tyrrell": ("Tyrrell Ford", "Tyrrell", "TYR", ("#1F2F6B", "#FFFFFF", "#8AB4E8"), "Ockham, UK", "britain",
                "019", "Ford V8", "mercedes", "Ken Tyrrell"),
    "lotus": ("Lotus Lamborghini", "Lotus", "LOT", ("#F2C500", "#1F4E9C", "#FFFFFF"), "Hethel, UK", "britain",
              "102", "Lamborghini V12", "red_bull", "Tony Rudd"),
    "march": ("Leyton House Judd", "Leyton House", "LEY", ("#5CDEE5", "#1B2A5C", "#FFFFFF"), "Bicester, UK",
              "britain", "CG901", "Judd V8", "racing_point", "Ian Phillips"),
    "larrousse": ("Larrousse Lamborghini", "Larrousse", "LAR", ("#5FAA68", "#FFFFFF", "#1F3C88"), "Antony, France",
                  "france", "LC90", "Lamborghini V12", None, "Gérard Larrousse"),
    "ligier_prost": ("Ligier Ford", "Ligier", "LIG", ("#0F5FDD", "#FFFFFF", "#6EB4FF"), "Magny-Cours, France",
                     "france", "JS33B", "Ford V8", "sauber", "Guy Ligier"),
    "minardi": ("Minardi Ford", "Minardi", "MIN", ("#F7AC15", "#FFFFFF", "#1A1A1A"), "Faenza, Italy", "italy",
                "M190", "Ford V8", "toro_rosso", "Giancarlo Minardi"),
    "brabham": ("Brabham Judd", "Brabham", "BRA", ("#2A3052", "#FFFFFF", "#C8102E"), "Chessington, UK", "britain",
                "BT59", "Judd V8", "gm_cadillac", "Herbie Blash"),
    "dallara": ("Dallara Ford", "Dallara", "DAL", ("#960D04", "#FFFFFF", "#1A1A1A"), "Lumezzane, Italy", "italy",
                "F190", "Ford V8", None, "Giampaolo Dallara"),
    "arrows": ("Arrows Ford", "Arrows", "ARR", ("#FFFFFF", "#C8102E", "#1A3C8C"), "Milton Keynes, UK", "britain",
               "A11B", "Ford V8", "haas", "Jackie Oliver"),
}
UI_COLOUR = {"arrows": "#C8102E"}
SUITS = {"mclaren_26": "2026-mclaren", "ferrari_26": "2026-ferrari", "williams_26": "2026-williams",
         "renault_26": "2026-renault", "mercedes_26": "2026-mercedes", "redbull_26": "2026-redbull",
         "forceindia_26": "2026-forceindia", "team12_26": "2026-williams", "sauber_26": "2026-sauber",
         "tororosso_26": "2026-tororosso", "gm_cadillac_26": "2026-cadillac", "team13_26": "2026-haas",
         "haas_26": "2026-haas"}
STOCK_SEATS = {
    "mclaren_26": ("mclaren_driver_01", "mclaren_driver_02"),
    "mercedes_26": ("mercedes_driver_01", "mercedes_driver_02"),
    "redbull_26": ("redbull_driver_01", "redbull_driver_02"),
    "ferrari_26": ("ferrari_driver_01", "ferrari_driver_02"),
    "williams_26": ("williams_driver_01", "williams_driver_02"),
    "tororosso_26": ("vcarb_driver_01", "vcarb_driver_02"),
    "forceindia_26": ("astonmartin_driver_01", "astonmartin_driver_02"),
    "haas_26": ("haas_driver_01", "haas_driver_02"),
    "sauber_26": ("kick_driver_01", "kick_driver_02"),
    "renault_26": ("alpine_driver_01", "alpine_driver_02"),
    "gm_cadillac_26": ("gm_cadillac_01", "gm_cadillac_02"),
    "team12_26": ("team12_driver_01", "team12_driver_02"),
    "team13_26": ("team13_driver_01", "team13_driver_02"),
}
STANDINGS_1989 = ["mclaren", "williams", "ferrari", "benetton", "tyrrell", "lotus", "arrows", "brabham", "minardi",
                  "march", "ligier_prost"]
DRIVERS_1989 = ["alain_prost", "ayrton_senna", "riccardo_patrese", "nigel_mansell", "thierry_boutsen",
                "alessandro_nannini", "gerhard_berger", "nelson_piquet", "jean_alesi", "derek_warwick",
                "michele_alboreto", "stefano_modena", "pierluigi_martini", "andrea_de_cesaris", "mauricio_gugelmin",
                "satoru_nakajima", "philippe_alliot", "ivan_capelli", "nicola_larini", "eric_bernard",
                "aguri_suzuki", "martin_donnelly"]


def argb(hex_colour):
    v = int(hex_colour.lstrip("#"), 16) | 0xFF000000
    return str(v - (1 << 32))


def rust_list(path, name):
    text = path.read_text(encoding="utf-8")
    body = re.search(rf"{name}[^=]*=\s*\[(.*?)\];", text, re.S).group(1)
    return re.findall(r'"([^"]+)"', body)


def stats_fields():
    f = ["_metadata_", "driver", "_StatisticsDataSet_instancecount_", "StatisticsDataSet_0._metadata_",
         "StatisticsDataSet_0.game_setup_modes", "StatisticsDataSet_0.is_invalid", "StatisticsDataSet_0.seasons",
         "StatisticsDataSet_0.weekend_formats", "StatisticsDataSet_0._Statistic_instancecount_"]
    for i in range(len(STATS)):
        for part in ("_metadata_", "info", "evolution"):
            f.append(f"StatisticsDataSet_0.Statistic_{i}.{part}")
    return f


def num(v, places=3):
    t = f"{max(0.0, min(1.0, v)):.{places}f}".rstrip("0").rstrip(".")
    return t or "0"


def curve(v):
    v = num(v)
    return f"@v2@minx:0,maxx:10,miny:0,maxy:100,0:{v},0.25:{v},0.5:{v},1:{v}"


def mean(*xs):
    return sum(xs) / len(xs)


def ratings(season, did):
    n = lambda k: season.normalised(did, k)
    seasons = 1990 - DRIVER_INFO[did][1]
    stats = {
        "experience": 0.62 + 0.33 * min(seasons / 10, 1.0),
        "racecraft": 0.70 + 0.27 * mean(n("race_skill"), n("defending"), n("aggression")),
        "awareness": 0.70 + 0.27 * mean(n("avoidance_of_mistakes"), n("avoidance_of_forced_mistakes"),
                                        n("blue_flag_conceding")),
        "pace": 0.70 + 0.27 * mean(n("qualifying_skill"), n("race_skill")),
        "focus": 0.70 + 0.27 * mean(n("stamina"), n("tyre_management"), n("fuel_management"),
                                    n("weather_tyre_changes")),
    }
    traits = {
        "aggression": 0.55 + 0.45 * n("aggression"),
        "control": 0.55 + 0.45 * mean(n("race_skill"), n("wet_skill")),
        "mistakes": 0.55 + 0.45 * mean(n("avoidance_of_mistakes"), n("avoidance_of_forced_mistakes")),
        "reaction_speed": 0.55 + 0.45 * n("start_reactions"),
        "speed_in": 0.55 + 0.45 * n("qualifying_skill"),
        "speed_out": 0.55 + 0.45 * mean(n("qualifying_skill"), n("race_skill")),
    }
    return stats, traits


def driver_label(did):
    return STOCK[did][0] if did in STOCK else f"f1990_{did}"


def build(season):
    lineup = []
    for s in slots():
        seats = season.lineup(1)[s.team]
        for pos, seat in enumerate(seats):
            lineup.append((s, pos, seat))
    seated = [seat.driver for _, _, seat in lineup]
    new = [d for d in seated if d not in STOCK] + SUBSTITUTES
    ids = dict(zip(new, DRIVER_IDS))
    if len(ids) != len(new):
        raise SystemExit("not enough driver ids")
    team_of = {seat.driver: s for s, _, seat in lineup}
    team_by_id = {s.team: s for s in slots()}
    for sub, team in SUB_SUITS.items():
        team_of.setdefault(sub, team_by_id[team])
    number_of = {seat.driver: seat.number for _, _, seat in lineup}
    for a in season.substitutions([s.team for s in slots()]):
        for seat in season.teams[a.team].seats:
            if seat.driver == a.out:
                number_of.setdefault(a.into, seat.number)
    for d in SUBSTITUTES:
        if d not in number_of:
            number_of[d] = next(x.number for x in season.teams[SUB_SUITS[d]].seats if x.driver == d)

    ops, lang = [], []

    def L(key, value):
        lang.append({"op": "lang.set", "language": "*", "key": key, "value": value})

    def setop(select, fld, value):
        ops.append({"op": "set", "select": select, "field": fld, "value": value})

    def add(table, frm, values):
        ops.append({"op": "add-row", "table": table, "from": frm, "set": values})

    for s in slots():
        name, short, code, colours, base, nat, chassis, engine, boss_key, boss = TEAM_INFO[s.team]
        team = f"entrants.teams.team[_metadata_={s.team_row}]"
        key = f"lng_f1990_team_{s.team}"
        L(key, name)
        L(key + "_short", short)
        L(f"lng_f1990_base_{s.team}", base)
        L(f"lng_f1990_chassis_{s.team}", chassis)
        L(f"lng_f1990_engine_{s.team}", engine)
        if boss_key:
            L(f"lng_{boss_key}_boss", boss)
        p, sec, ter = (argb(c) for c in colours)
        for fld, value in [
            ("display_name", key), ("display_name_short", key + "_short"), ("display_name_super_short", key + "_short"),
            ("display_name_code", code), ("primary_colour", p), ("secondary_colour", sec), ("tertiary_colour", ter),
            ("ui_team_colour", argb(UI_COLOUR.get(s.team, colours[0]))), ("team_wear_primary_colour", p),
            ("team_wear_secondary_colour", sec), ("team_wear_tertiary_colour", ter),
            ("team_base", f"lng_f1990_base_{s.team}"), ("nationality", nat),
        ]:
            setop(team, fld, value)
        vehicle = f"entrants.vehicles.vehicle[_metadata_={s.vehicle}]"
        setop(vehicle, "display_name", key)
        setop(vehicle, "chassis_name", f"lng_f1990_chassis_{s.team}")
        setop(vehicle, "engine_name_override", f"lng_f1990_engine_{s.team}")

    preset = FIRST_PRESET
    for did in seated + SUBSTITUTES:
        d = season.drivers[did]
        code, debut, (by, bm, bd), head, helmet = DRIVER_INFO[did]
        label = driver_label(did)
        look = f"f1990_{did}_look"
        entrant = f"f1990-{did}"
        number = number_of[did]
        stats, traits = ratings(season, did)
        s = team_of[did]
        suit = SUITS[s.team_row]
        if did in STOCK:
            row = f"entrants.drivers.driver[_metadata_={label}]"
            setop(row, "car_number", str(number))
            setop(row, "f1_seasons_completed", str(1990 - debut))
            for t in TRAITS:
                setop(row, f"driver_characteristics.{t}", num(traits[t], 2))
            helmet = STOCK[did][1]
        else:
            first, last = d.first, d.last
            L(f"lng_f1990_{did}_first", first)
            L(f"lng_f1990_{did}_last", last.upper())
            L(f"lng_f1990_{did}_full", f"{first} {last.upper()}")
            values = {
                "_metadata_": label, "audio_name": VOICES.get(did, "DefaultName"), "available": "True",
                "car_number": str(number), "date_of_birth.year": str(by),
                "date_of_birth.month": ["January", "February", "March", "April", "May", "June", "July", "August",
                                        "September", "October", "November", "December"][bm - 1],
                "date_of_birth.day": str(bd), "f1_seasons_completed": str(1990 - debut),
                "first_name": f"lng_f1990_{did}_first", "full_name": f"lng_f1990_{did}_full",
                "garage_slot_preference": "0", "head": head, "initials": code, "is_name_reversed": "False",
                "last_name": f"lng_f1990_{did}_last", "nationality": d.nationality, "stat_id": str(ids[did]),
            }
            for t in TRAITS:
                values[f"driver_characteristics.{t}"] = num(traits[t], 2)
            add(DRIVERS, DRIVER_FROM, values)
        add(LOOKS, LOOK_FROM, {
            "_metadata_": look, "available": "False", "badge": "Basic Badge",
            "car_livery": "livery_titlesponsor_livery23", "gloves": suit, "helmet": helmet,
            "linked_driver_stat_id": label, "podium_emote": "Emote01", "pose": "Pose03", "stat_id": str(preset),
            "suit": suit, "victory_vo": "",
        })
        preset += 1
        curves = {f"StatisticsDataSet_0.Statistic_{i}.evolution": curve(stats[k]) for i, k in enumerate(STATS)}
        if did in STOCK and did not in NEW_STATS_HOLDER:
            holder = f"{STATS_TABLE}.DriverStatisticsHolder[_metadata_={label}]"
            for fld, value in curves.items():
                setop(holder, fld, value)
        else:
            values = {
                "_metadata_": label, "driver": label, "_StatisticsDataSet_instancecount_": "1",
                "StatisticsDataSet_0._metadata_": "default", "StatisticsDataSet_0.game_setup_modes": "",
                "StatisticsDataSet_0.is_invalid": "False", "StatisticsDataSet_0.seasons": "",
                "StatisticsDataSet_0.weekend_formats": "", "StatisticsDataSet_0._Statistic_instancecount_": str(len(STATS)),
            }
            for i, k in enumerate(STATS):
                values[f"StatisticsDataSet_0.Statistic_{i}._metadata_"] = k
                values[f"StatisticsDataSet_0.Statistic_{i}.info"] = k
            values.update(curves)
            add(STATS_TABLE, STATS_FROM, values)
        add(ENTRANTS, ENTRANT_FROM, {
            "_metadata_": entrant, "badge_customisation": "", "car_number": str(number), "driver": label,
            "driver_customisation": look, "entrant_reference": "", "team": "", "team_position": "-1",
            "vehicle": "", "vehicle_customisation": "",
        })

    for s, pos, seat in lineup:
        row = f"{LIST}.entrant[_metadata_={STOCK_SEATS[s.team_row][pos]}]"
        setop(row, "entrant_reference", f"f1990-{seat.driver}")
        setop(row, "car_number", str(seat.number))
        setop(row, "team_position", str(pos))

    setop(f"{POOL}.entrant[_metadata_=senna]", "entrant_reference", f"f1990-{SUBSTITUTES[0]}")
    for did in SUBSTITUTES[1:]:
        add(POOL, f"{POOL}.entrant[_metadata_=opmeer]", {
            "_metadata_": f"f1990_{did}", "badge_customisation": "", "car_number": "255", "driver": "",
            "driver_customisation": "", "entrant_reference": f"f1990-{did}", "team": "", "team_position": "-1",
            "vehicle": "", "vehicle_customisation": "",
        })

    rows = {s.team: s.team_row for s in slots()}
    for i, tid in enumerate(STANDINGS_1989):
        setop(f"{TEAM_STANDINGS}.team_reference_{i}", "team", rows[tid])
    for i, did in enumerate(DRIVERS_1989):
        setop(f"{DRIVER_STANDINGS}.driver_reference_{i}", "driver", driver_label(did))

    fields = {
        f"{DRIVERS}.driver": rust_list(DM_SRC / "fields.rs", "DRIVER_FIELDS"),
        f"{LOOKS}.preloaded_driver_customisation_properties": LOOK_FIELDS,
        f"{STATS_TABLE}.DriverStatisticsHolder": stats_fields(),
        f"{ENTRANTS}.entrant": ENTRANT_FIELDS,
        f"{POOL}.entrant": ENTRANT_FIELDS,
    }
    report = {"ids": ids, "presets": {d: FIRST_PRESET + i for i, d in enumerate(seated + SUBSTITUTES)},
              "ratings": {d: ratings(season, d) for d in seated + SUBSTITUTES}}
    return {"schema_version": 1, "fields": fields, "ops": ops}, {"schema_version": 1, "ops": lang}, report


def main(argv):
    season = Season.load()
    db, lang, report = build(season)
    (ROOT / "db").mkdir(exist_ok=True)
    (ROOT / "lang").mkdir(exist_ok=True)
    (ROOT / "db/grid.json").write_text(json.dumps(db, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    (ROOT / "lang/grid.json").write_text(json.dumps(lang, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"db/grid.json: {len(db['ops'])} ops; lang/grid.json: {len(lang['ops'])} strings")
    if "-v" in argv:
        for d, i in report["ids"].items():
            print(f"{d:22} stat_id {i:3}")
        for d, (st, tr) in report["ratings"].items():
            print(f"{d:22} " + " ".join(f"{k[:4]} {v*100:4.0f}" for k, v in st.items()) + " | "
                  + " ".join(f"{k[:5]} {v:.2f}" for k, v in tr.items()))


if __name__ == "__main__":
    main(sys.argv)
