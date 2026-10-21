#!/usr/bin/env python3
"""Writes db/career.json and db/career-lang.json: the 1990 career content, next to stock 2025 and 2026.

usage: career.py <stock gamedatabases.2025.bin> <dir of language_<code>.tsv dumps (key<TAB>value, \\n escaped)>

Every new row is a full-key clone of its 2026 source row, with the field list read from the stock database through
the key catalogue (tools/gamedb/catalog/keys.tsv.gz). No 2025 or 2026 row changes; content set 11 gains the 1990
names in two lists.
"""
import gzip
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ADDON = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(ADDON))
sys.path.insert(0, os.path.join(REPO, "tools"))
import gamedb  # noqa: E402

CATALOG = os.path.join(REPO, "tools", "gamedb", "catalog", "keys.tsv.gz")

ROUNDS = [
    ("Phoenix", "las_vegas", "rw_las_vegas_26", 11, 3, "las", "UNITED STATES GRAND PRIX"),
    ("Interlagos", "brazil", "rw_brazil_26", 25, 3, "bra", "GRANDE PRÊMIO DO BRASIL"),
    ("Imola", "imola", "rw_imola", 13, 5, "imo", "GRAN PREMIO DI SAN MARINO"),
    ("Monaco", "monaco", "rw_monaco_26", 27, 5, "mco", "GRAND PRIX DE MONACO"),
    ("Montreal", "montreal", "rw_montreal_26", 10, 6, "can", "GRAND PRIX DU CANADA"),
    ("Mexico City", "mexico", "rw_mexico_26", 24, 6, "mex", "GRAN PREMIO DE MÉXICO"),
    ("Paul Ricard", "austria", "rw_austria_26", 8, 7, "aut", "GRAND PRIX DE FRANCE"),
    ("Silverstone", "silverstone", "rw_silverstone_26", 15, 7, "gbr", "BRITISH GRAND PRIX"),
    ("Hockenheim", "shanghai", "rw_shanghai_26", 29, 7, "chn", "GROSSER PREIS VON DEUTSCHLAND"),
    ("Hungaroring", "hungaroring", "rw_hungaroring_26", 12, 8, "hun", "MAGYAR NAGYDÍJ"),
    ("Spa", "spa_francorchamps", "rw_spa_francorchamps_26", 26, 8, "bel", "GRAND PRIX DE BELGIQUE"),
    ("Monza", "monza", "rw_monza_26", 9, 9, "ita", "GRAN PREMIO D’ITALIA"),
    ("Estoril", "madrid", "rw_madrid_26", 23, 9, "mad", "GRANDE PRÉMIO DE PORTUGAL"),
    ("Jerez", "catalunya", "rw_catalunya_26", 30, 9, "cat", "GRAN PREMIO DE ESPAÑA"),
    ("Suzuka", "suzuka", "rw_suzuka_26", 21, 10, "jpn", "JAPANESE GRAND PRIX"),
    ("Adelaide", "melbourne", "rw_melbourne_26", 4, 11, "aus", "AUSTRALIAN GRAND PRIX"),
]
SHORT_NAMES_ENG = {
    "las_vegas": "UNITED STATES GRAND PRIX", "imola": "SAN MARINO GRAND PRIX", "austria": "FRENCH GRAND PRIX",
    "shanghai": "GERMAN GRAND PRIX", "madrid": "PORTUGUESE GRAND PRIX", "catalunya": "SPANISH GRAND PRIX",
    "brazil": "BRAZILIAN GRAND PRIX", "mexico": "MEXICAN GRAND PRIX", "monza": "ITALIAN GRAND PRIX",
}
STOCK_SHORT_KEY = {"catalunya": "lng_track_cat_gp_title_short_26"}

DATA_GROUP = "f1_career_1990"
DATA_GROUP_SOURCE = "f1_career_2026"
DATA_GROUP_TABLES = [
    ("career.career_setup", "CareerSetupDataGroup"),
    ("accolades", "AccoladeDataGroup"),
    ("career.leverage", "LeverageDataGroup"),
    ("career.my_team_renown_and_form", "MyTeamRenownAndFormDataGroup"),
    ("career.my_team_sponsors", "MyTeamSponsorsDataGroup"),
    ("recognition", "RecognitionDataGroup"),
    ("resource_points", "data_group"),
    ("specialist_manager", "dataGroup"),
]

SETUP_MODE = "f1_1990"
MODE = "driver_career_1990"
MODE_STAT_ID = 81
CHAMPIONSHIP = "F1_driver_career_1990"
CHAMPIONSHIP_STAT_ID = 35
ENTRANT_LIST = "f1_career_1990"
TRACK_LIST = "F1_1990"
EVENT_LIST = "F1_career_1990"
TRACK_SELECTION = "F1_1990"
CONTENT_SET = "2026"
LENGTHS = "schedule.dynamic_event_data.default_ui_track_list_data"

SEASON_EVENTS = [
    ("start_season_ref", "start_season_2026"),
    ("end_season_ref", "end_season_2026"),
    ("season_break_ref", "season_break_2026"),
    ("car_reveal_ref", "car_reveal_2026"),
    ("sliding_scale_refresh_ref", "sliding_scale_refresh_26"),
]

AGENT = "entrants.characters.character[_metadata_=elena]"
AGENT_NAME = {"jap": "フラビオ", "zhs": "弗拉维奥"}
AGENT_FULL_NAME = {"jap": "フラビオ・ブリアトーレ", "zhs": "弗拉维奥·布里亚托雷"}
STOCK_AGENT_NAME = {"jap": "エレナ", "zhs": "埃琳娜"}
INTRO_KEYS = re.compile(r"^lng_subtitle_agt_intro_")

SEASON_STRINGS = {
    "lng_f1990_career_content": "lng_career_dlc_content",
    "lng_f1990_career_content_description": "lng_career_dlc_content_description",
    "lng_f1990_season": "lng_f1_26",
    "lng_f1990_championship_season": "lng_f1_2026_championship_season",
    "lng_f1990_championship_season_loading_screen": "lng_f1_championship_season_26_loading_screen",
    "lng_f1990_trophy_drivers_champion": "lng_trophies_championship_champion_f1_2026",
}
SEASON_STRINGS_ENG = {
    "lng_f1990_career_content_description": "Play the 1990 season: 1990 teams, drivers, calendar and rules.",
}



def gp_keys(code):
    return f"lng_f1990_gp_{code}", f"lng_f1990_gp_{code}_short"


TRACK_NAMES = {track: gp_keys(code) for _, track, _, _, _, code, _ in ROUNDS}
TRACK_DATES = {track: (day, month) for _, track, _, day, month, _, _ in ROUNDS}


class Db:
    def __init__(self, path):
        self.d = {r.key: (r.type, r.value.decode("utf-8", "replace"))
                  for r in gamedb.read(open(path, "rb").read())}
        with gzip.open(CATALOG, "rt", encoding="utf-8") as f:
            self.cat = [line.split("\t", 1)[0] for line in f][1:]

    def get(self, path):
        v = self.d.get(gamedb.fnv1a64(path.encode()))
        return v[1] if v else None

    def count(self, container, word):
        v = self.get(f"{container}._{word}_instancecount_")
        return int(v) if v is not None else None

    def row_index(self, container, word, label):
        for i in range(self.count(container, word) or 0):
            if self.get(f"{container}.{word}_{i}._metadata_") == label:
                return i
        raise KeyError(f"{container}.{word}[_metadata_={label}]")

    def fields(self, container, word, label):
        row = f"{container}.{word}_{self.row_index(container, word, label)}"
        template = f"{container}.{word}_N."
        out = set()
        for key in self.cat:
            if key.startswith(template):
                out.update(self.under(row, key[len(template):]))
        return sorted(out)

    def under(self, row, rest):
        segs = rest.split(".")
        i = next((k for k, seg in enumerate(segs) if seg.endswith("_N")), None)
        if i is None:
            return [rest] if self.get(f"{row}.{rest}") is not None else []
        pre, word, tail = ".".join(segs[:i]), segs[i][:-2], ".".join(segs[i + 1:])
        parent = f"{row}.{pre}" if pre else row
        n = self.count(parent, word)
        found = []
        for k in range(n if n is not None else 64):
            node = f"{pre}.{word}_{k}" if pre else f"{word}_{k}"
            if not tail:
                if self.get(f"{row}.{node}") is not None:
                    found.append(node)
                continue
            found += [f"{node}.{sub}" for sub in self.under(f"{row}.{node}", tail)]
        return found


def setop(select, field, value):
    return {"op": "set", "select": select, "field": field, "value": str(value)}


def build(db):
    fields = {}
    ops = []

    def clone(container, word, source, row, keep=None):
        key = f"{container}.{word}"
        f = [x for x in db.fields(container, word, source) if keep is None or keep(x)]
        missing = [k for k in row if k not in f]
        if missing:
            raise SystemExit(f"{key}: fields not on {source}: {missing}")
        if key in fields and fields[key] != f:
            raise SystemExit(f"{key}: two sources with different fields")
        fields[key] = f
        ops.append({"op": "add-row", "table": container, "from": f"{container}.{word}[_metadata_={source}]",
                    "set": row})

    for container, word in DATA_GROUP_TABLES:
        clone(container, word, DATA_GROUP_SOURCE, {"_metadata_": DATA_GROUP})

    events = {"_metadata_": TRACK_LIST, "sort_by_date": "False",
              "_fixed_event_reference_instancecount_": str(len(ROUNDS))}
    for k, (_, track, event, _, _, _, _) in enumerate(ROUNDS):
        events[f"fixed_event_reference_{k}._metadata_"] = f"fixed_event_reference_rw_{track}"
        events[f"fixed_event_reference_{k}.event_name"] = event
    clone("schedule.track_lists", "track_event_list", "F1_2026_16", events)

    s = "season_0"
    season = {
        "_metadata_": EVENT_LIST,
        "_season_instancecount_": "1",
        f"{s}._fixed_event_reference_instancecount_": str(len(SEASON_EVENTS)),
        f"{s}.date_time_0.year": "{{career_year}}",
        f"{s}.date_time_1.year": "{{career_year}}",
    }
    for k, (ref, event) in enumerate(SEASON_EVENTS):
        season[f"{s}.fixed_event_reference_{k}._metadata_"] = ref
        season[f"{s}.fixed_event_reference_{k}.event_name"] = event
    later = re.compile(rf"^season_[1-9]|^{s}\.fixed_event_reference_([{len(SEASON_EVENTS)}-9]|\d\d)\.")
    clone("schedule.event_lists", "event_list", "F1_career_26", season, keep=lambda x: not later.match(x))

    for length in ("Long", "Medium", "Short"):
        clone(LENGTHS, "custom_dynamic_available_track_list_data", f"F1_2026_{length}", {
            "_metadata_": f"F1_1990_{length}",
            "game_setup_modes": SETUP_MODE,
            "max_num_tracks": str(len(ROUNDS)),
            "track_list_name": TRACK_LIST,
            "weekend_to_add_transfer_window": "-1",
            "language_string": "lng_f1990_season_length",
        })
    clone(f"{LENGTHS}.track_selections", "tracks_selection", "F1_2026", {
        "_metadata_": TRACK_SELECTION,
        "game_setup_modes": SETUP_MODE,
        "default": TRACK_LIST,
        "season_0.track_event_list": TRACK_LIST,
    })

    clone("championships", "Championship", "F1_driver_career_26", {
        "_metadata_": CHAMPIONSHIP,
        "statId": str(CHAMPIONSHIP_STAT_ID),
        "event_list": EVENT_LIST,
        "game_setup_modes": SETUP_MODE,
        "ComCareerSetup_0.entrant_list": ENTRANT_LIST,
        "ComCareerSetup_0.setup_data": DATA_GROUP,
    })
    setup = "championship_setups.ChampionshipSetupList_0.ChampionshipSetup_0"
    clone("game_modes", "ModeChampionship", "driver_career_26", {
        "_metadata_": MODE,
        "statId": str(MODE_STAT_ID),
        "save_data.name": MODE,
        "event_list": EVENT_LIST,
        f"{setup}.championship": CHAMPIONSHIP,
        f"{setup}.event_list": EVENT_LIST,
        "championships.ChampionshipReference_0.Championship": CHAMPIONSHIP,
    })

    ci = db.row_index("content_sets", "content_set", CONTENT_SET)
    cs = f"content_sets.content_set[_metadata_={CONTENT_SET}]"
    for field, name in (("available_game_modes_0.game_mode_championships", MODE),
                        ("available_game_setup_modes_0.setup_modes", SETUP_MODE)):
        ops.append(setop(cs, field, f"{db.get(f'content_sets.content_set_{ci}.{field}')},{name}"))

    ops += [
        setop(AGENT, "full_name", "lng_f1990_agent_full_name"),
        setop(AGENT, "last_name", "lng_f1990_agent_last_name"),
    ]
    return {"schema_version": 1, "fields": fields, "ops": ops}


def read_tsv(path):
    out = {}
    with open(path, encoding="utf-8", newline="\n") as f:
        for line in f:
            key, _, value = line.rstrip("\n").partition("\t")
            out[key] = value.replace("\\n", "\n").replace("\\t", "\t")
    return out


def lang_ops(lang_dir):
    ops = [
        {"op": "lang.set", "language": "*", "key": "lng_f1990_season_length", "value": "1990 Season"},
        {"op": "lang.set", "language": "*", "key": "lng_f1990_agent_full_name", "value": "Flavio Briatore"},
        {"op": "lang.set", "language": "*", "key": "lng_f1990_agent_last_name", "value": "Briatore"},
    ]
    for _, _, _, _, _, code, name in ROUNDS:
        ops.append({"op": "lang.set", "language": "*", "key": gp_keys(code)[0], "value": f"{name} 1990"})
    codes = sorted(n[len("language_"):-len(".tsv")] for n in os.listdir(lang_dir)
                   if n.startswith("language_") and n.endswith(".tsv"))
    for code in codes:
        table = read_tsv(os.path.join(lang_dir, f"language_{code}.tsv"))
        stock = STOCK_AGENT_NAME.get(code, "Elena")
        name = AGENT_NAME.get(code, "Flavio")
        full = AGENT_FULL_NAME.get(code, "Flavio Briatore")
        pattern = re.compile(re.escape(stock) if code in STOCK_AGENT_NAME else r"\bElena\b")
        for key in sorted(table):
            value = table[key]
            if key in ("lng_elena_name", "lng_email_from_agent"):
                new = name
            elif pattern.search(value):
                new = pattern.sub(full if INTRO_KEYS.match(key) else name, value, count=1)
                new = pattern.sub(name, new)
            else:
                continue
            if new != value:
                ops.append({"op": "lang.set", "language": code, "key": key, "value": new})
        for key, source in SEASON_STRINGS.items():
            if source in table:
                value = SEASON_STRINGS_ENG[key] if code == "eng" and key in SEASON_STRINGS_ENG else \
                    table[source].replace("2026", "1990")
                ops.append({"op": "lang.set", "language": code, "key": key, "value": value})
        for _, track, _, _, _, gp, _ in ROUNDS:
            stock_short = table.get(STOCK_SHORT_KEY.get(track, f"lng_track_{gp}_gp_title_short"))
            value = SHORT_NAMES_ENG[track] if code == "eng" and track in SHORT_NAMES_ENG else stock_short
            if value:
                ops.append({"op": "lang.set", "language": code, "key": gp_keys(gp)[1], "value": value})
    return ops


def write_json(path, doc):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=1, ensure_ascii=False)
        f.write("\n")


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    db = Db(sys.argv[1])
    write_json(os.path.join(ADDON, "db", "career.json"), build(db))
    write_json(os.path.join(ADDON, "db", "career-lang.json"), {"schema_version": 1, "ops": lang_ops(sys.argv[2])})


if __name__ == "__main__":
    main()
