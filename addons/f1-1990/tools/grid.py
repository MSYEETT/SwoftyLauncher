#!/usr/bin/env python3
import json
import sys
from pathlib import Path

from season import Season, slots

ROOT = Path(__file__).resolve().parents[1]

LIST = "entrants.entrant_lists.entrant_list[_metadata_=f1_2026]"
POOL = "entrants.entrant_lists.entrant_list[_metadata_=f1_2026_driver_pool]"
LOOKS = "entrants.driver_customisations.folder[_metadata_=official_driver_customisations]"
SPARE_LOOKS = [
    "2026-f2-aixracing_cian_shields", "2026-f2-artgrandprix_kush_maini", "2026-f2-camposracing_nikola_tsolov",
    "2026-f2-dams_dino_beganovic", "2026-f2-hitech_ritomo_miyata", "2026-f2-invictaracing_joshua_duerksen",
    "2026-f2-mpmotorsport_gabriele_mini", "2026-f2-mpmotorsport_oliver_goethe", "2026-f2-prema_sebastian_montoya",
    "2026-f2-tridentmotorsport_john_bennett",
]
PACKAGES = {
    "mclaren_26": ["lando_norris", "oscar_piastri"],
    "mercedes_26": ["george_russell", "andrea-kimi_antonelli"],
    "redbull_26": ["max_verstappen", "isack_hadjar"],
    "ferrari_26": ["charles_leclerc", "lewis_hamilton"],
    "williams_26": ["alex_albon", "carlos_sainz-jr"],
    "tororosso_26": ["liam_lawson", "arvid_lindblad"],
    "forceindia_26": ["fernando_alonso", "lance_stroll"],
    "haas_26": ["oliver_bearman", "esteban_ocon"],
    "sauber_26": ["nico_hulkenberg", "gabriel_bortoleto"],
    "renault_26": ["pierre_gasly", "franco_colapinto"],
    "gm_cadillac_26": ["sergio_perez", "valtteri_bottas"],
}
LOOK_TEAM = {"gm_cadillac_26": "cadillac"}
PACKAGE_INFO = {
    d: (f"2026-{LOOK_TEAM.get(row, row[:-3])}_{d}", "alexander_albon" if d == "alex_albon" else d, 2)
    for row, ds in PACKAGES.items() for d in ds
}
OTHER_ROWS = ["nyck_de-vries", "marino_sato", "marcus_armstrong", "juri_vips", "felipe_drugovich", "olli_caldwell"]
MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October",
          "November", "December"]
STATS_TABLE = "driver_statistics.driver_statistics"
ENTRANTS = "entrants.entrant_lists.entrants"
ENTRANT_FROM = ENTRANTS + ".entrant[_metadata_=jarno_opmeer]"
TEAM_STANDINGS = "entrants.standings.standings[_metadata_=f1_teams_2025]"
DRIVER_STANDINGS = "entrants.standings.standings[_metadata_=f1_drivers_2025]"

STATS = ("experience", "racecraft", "awareness", "pace", "focus")
TRAITS = ("aggression", "control", "mistakes", "reaction_speed", "speed_in", "speed_out")
ENTRANT_FIELDS = ["_metadata_", "badge_customisation", "car_number", "driver", "driver_customisation",
                  "entrant_reference", "team", "team_position", "vehicle", "vehicle_customisation"]

STOCK = {
    "ayrton_senna": ("ayrton_senna", "classic-senna01_ayrton_senna"),
    "alain_prost": ("alain_prost", "classic-prost01_alain_prost"),
    "nigel_mansell": ("nigel_mansell", "classic-mansell01_nigel_mansell"),
}
OWN_HOLDERS = {"ayrton_senna": "aryton_senna", "alain_prost": "alain_prost", "nigel_mansell": "nigel_mansell"}



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
                "F190", "Ford V8", None, "Giuseppe Lucchini"),
    "arrows": ("Arrows Ford", "Arrows", "ARR", ("#FFFFFF", "#C8102E", "#1A3C8C"), "Milton Keynes, UK", "britain",
               "A11B", "Ford V8", "haas", "Jackie Oliver"),
}
UI_COLOUR = {"arrows": "#C8102E"}
STORY = {
    "ferrari": ("Prost joins Mansell at Maranello, with the 641 and its semi-automatic gearbox.", "#Ferrari"),
    "williams": ("Renault V10 power and the FW13B for Boutsen and Patrese.", "#Williams"),
    "tyrrell": ("Alesi and Nakajima drive; Harvey Postlethwaite's raised-nose 019 arrives at Imola.", "#Tyrrell"),
    "lotus": ("Lamborghini V12 power for Warwick and Donnelly in the 102.", "#Lotus"),
    "march": ("Adrian Newey's CG901 with Judd power for Gugelmin and Capelli.", "#LeytonHouse"),
    "larrousse": ("The Lola-built LC90 with the Lamborghini V12 for Bernard and Suzuki.", "#Larrousse"),
    "ligier_prost": ("The JS33B with Ford power for Larini and Alliot.", "#Ligier"),
    "minardi": ("The Faenza team runs the M190 with Ford power for Martini and Barilla.", "#Minardi"),
    "brabham": ("The BT59 with Judd power for Modena and David Brabham.", "#Brabham"),
    "dallara": ("BMS Scuderia Italia runs the Dallara F190 for de Cesaris and Pirro.", "#ScuderiaItalia"),
    "arrows": ("Arrows runs the A11B with Ford power for Alboreto and Caffi.", "#Arrows"),
}
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
BOSS_ROWS = {"mclaren_26": "andrea_stella", "mercedes_26": "toto_wollf", "redbull_26": "laurent_mekies",
             "ferrari_26": "frederic_vasseur", "williams_26": "james_vowles", "tororosso_26": "alan_permane",
             "forceindia_26": "adrian_newey", "haas_26": "ayao_komatsu", "sauber_26": "mattia_binotto",
             "renault_26": "steve_nielsen", "gm_cadillac_26": "graeme_lowdon", "team12_26": "andreas_seidl",
             "team13_26": "franz_tost"}
OWN_BOSS = {"team12_26", "team13_26"}
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


def build(season):
    teams = [s.team for s in slots()]
    regulars = season.regulars(teams)
    stand_ins = season.stand_ins(teams)
    free = [d for d, _ in stand_ins]
    lineup = []
    for s in slots():
        for pos, seat in enumerate(regulars[s.team]):
            lineup.append((s, pos, seat))
    seated = [seat.driver for _, _, seat in lineup]
    team_of = {seat.driver: s for s, _, seat in lineup}
    team_by_id = {s.team: s for s in slots()}
    for d, tid in stand_ins:
        team_of[d] = team_by_id[tid]
    number_of = {seat.driver: seat.number for _, _, seat in lineup}
    for d, tid in stand_ins:
        for r in season.races:
            seats = season.lineup(r.round, [tid])[tid]
            hit = next((x.number for x in seats if x.driver == d), None)
            if hit is not None:
                number_of.setdefault(d, hit)

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
        boss_first, _, boss_last = boss.partition(" ")
        boss_row = BOSS_ROWS[s.team_row]
        if s.team_row in OWN_BOSS:
            setop(team, "team_boss", boss_row)
        for part, value in (("full", boss), ("first", boss_first), ("last", boss_last)):
            L(f"lng_f1990_boss_{s.team}_{part}", value)
            setop(f"entrants.characters.character[_metadata_={boss_row}]", f"{part}_name",
                  f"lng_f1990_boss_{s.team}_{part}")
        p, sec, ter = (argb(c) for c in colours)
        for fld, value in [
            ("display_name", key), ("display_name_short", key + "_short"), ("display_name_super_short", key + "_short"),
            ("display_name_code", code), ("primary_colour", p), ("secondary_colour", sec), ("tertiary_colour", ter),
            ("ui_team_colour", argb(UI_COLOUR.get(s.team, colours[0]))), ("team_wear_primary_colour", p),
            ("team_wear_secondary_colour", sec), ("team_wear_tertiary_colour", ter),
            ("team_base", f"lng_f1990_base_{s.team}"), ("nationality", nat),
        ]:
            setop(team, fld, value)
        if s.team in STORY:
            L(f"lng_f1990_desc_{s.team}", STORY[s.team][0])
            L(f"lng_f1990_tag_{s.team}", STORY[s.team][1])
            setop(team, "story_info_0.description", f"lng_f1990_desc_{s.team}")
            setop(team, "story_info_0.hashtag", f"lng_f1990_tag_{s.team}")
        vehicle = f"entrants.vehicles.vehicle[_metadata_={s.vehicle}]"
        setop(vehicle, "display_name", key)
        setop(vehicle, "chassis_name", f"lng_f1990_chassis_{s.team}")
        setop(vehicle, "engine_name_override", f"lng_f1990_engine_{s.team}")

    driver_labels, entrant_of = {}, {}
    packages = {row: list(drivers) for row, drivers in PACKAGES.items()}
    others = iter(OTHER_ROWS)
    reuse = {}
    for s, pos, seat in lineup:
        if seat.driver not in STOCK:
            team_packages = packages.get(s.team_row)
            reuse[seat.driver] = team_packages.pop(0) if team_packages else next(others)
    leftover = [d for row in PACKAGES for d in packages[row]]
    for did in free:
        reuse[did] = leftover.pop(0) if leftover else next(others)
    f2_looks = iter(SPARE_LOOKS)
    for did in seated + free:
        d = season.drivers[did]
        code, debut, (by, bm, bd), head, helmet = DRIVER_INFO[did]
        number = number_of[did]
        stats, traits = ratings(season, did)
        suit = SUITS[team_of[did].team_row]
        if did in STOCK:
            label, holder_label, datasets, look, entrant = STOCK[did][0], OWN_HOLDERS[did], 1, None, None
            helmet = STOCK[did][1]
            row = f"entrants.drivers.driver[_metadata_={label}]"
            setop(row, "car_number", str(number))
            setop(row, "f1_seasons_completed", str(1990 - debut))
            for t in TRAITS:
                setop(row, f"driver_characteristics.{t}", num(traits[t], 2))
        else:
            label = reuse[did]
            if label in PACKAGE_INFO:
                look, holder_label, datasets = PACKAGE_INFO[label]
                entrant = f"f1_2026-{label}"
            else:
                look, holder_label, datasets, entrant = next(f2_looks), label, 1, None
            first, last = d.first, d.last
            L(f"lng_f1990_{did}_first", first)
            L(f"lng_f1990_{did}_last", last.upper())
            L(f"lng_f1990_{did}_full", f"{first} {last.upper()}")
            row = f"entrants.drivers.driver[_metadata_={label}]"
            values = {
                "audio_name": VOICES.get(did, "DefaultName"), "available": "True", "car_number": str(number),
                "date_of_birth.year": str(by), "date_of_birth.month": MONTHS[bm - 1], "date_of_birth.day": str(bd),
                "f1_seasons_completed": str(1990 - debut), "first_name": f"lng_f1990_{did}_first",
                "full_name": f"lng_f1990_{did}_full", "garage_slot_preference": "0", "head": head, "initials": code,
                "is_name_reversed": "False", "last_name": f"lng_f1990_{did}_last", "nationality": d.nationality,
            }
            for t in TRAITS:
                values[f"driver_characteristics.{t}"] = num(traits[t], 2)
            for fld, value in values.items():
                setop(row, fld, value)
        driver_labels[did] = label
        holder = f"{STATS_TABLE}.DriverStatisticsHolder[_metadata_={holder_label}]"
        for ds in range(datasets):
            for i, k in enumerate(STATS):
                setop(holder, f"StatisticsDataSet_{ds}.Statistic_{i}.evolution", curve(stats[k]))
        if look is None:
            look = next(f2_looks)
        look_row = f"{LOOKS}.preloaded_driver_customisation_properties[_metadata_={look}]"
        for fld, value in (("helmet", helmet), ("suit", suit), ("gloves", suit), ("linked_driver_stat_id", label)):
            setop(look_row, fld, value)
        if entrant:
            erow = f"{ENTRANTS}.entrant[_metadata_={entrant}]"
            setop(erow, "car_number", str(number))
            setop(erow, "driver_customisation", look)
            entrant_of[did] = entrant
        else:
            entrant = f"f1990-{did}"
            entrant_of[did] = entrant
            add(ENTRANTS, ENTRANT_FROM, {
                "_metadata_": entrant, "badge_customisation": "", "car_number": str(number), "driver": label,
                "driver_customisation": look, "entrant_reference": "", "team": "", "team_position": "-1",
                "vehicle": "", "vehicle_customisation": "",
            })

    for s, pos, seat in lineup:
        row = f"{LIST}.entrant[_metadata_={STOCK_SEATS[s.team_row][pos]}]"
        setop(row, "entrant_reference", entrant_of[seat.driver])
        setop(row, "car_number", str(seat.number))
        setop(row, "team_position", str(pos))

    setop(f"{POOL}.entrant[_metadata_=senna]", "entrant_reference", entrant_of[free[0]])
    for did in free[1:]:
        add(POOL, f"{POOL}.entrant[_metadata_=opmeer]", {
            "_metadata_": f"f1990_{did}", "badge_customisation": "", "car_number": "255", "driver": "",
            "driver_customisation": "", "entrant_reference": entrant_of[did], "team": "", "team_position": "-1",
            "vehicle": "", "vehicle_customisation": "",
        })

    rows = {s.team: s.team_row for s in slots()}
    for i, tid in enumerate(STANDINGS_1989):
        setop(f"{TEAM_STANDINGS}.team_reference_{i}", "team", rows[tid])
    for i, did in enumerate(DRIVERS_1989):
        setop(f"{DRIVER_STANDINGS}.driver_reference_{i}", "driver", driver_labels[did])

    fields = {
        f"{ENTRANTS}.entrant": ENTRANT_FIELDS,
        f"{POOL}.entrant": ENTRANT_FIELDS,
    }
    report = {"rows": driver_labels,
              "ratings": {d: ratings(season, d) for d in seated + free}}
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
        for d, r in report["rows"].items():
            print(f"{d:22} row {r}")
        for d, (st, tr) in report["ratings"].items():
            print(f"{d:22} " + " ".join(f"{k[:4]} {v*100:4.0f}" for k, v in st.items()) + " | "
                  + " ".join(f"{k[:5]} {v:.2f}" for k, v in tr.items()))


if __name__ == "__main__":
    main(sys.argv)
