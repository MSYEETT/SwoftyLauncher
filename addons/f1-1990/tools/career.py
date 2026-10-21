#!/usr/bin/env python3
"""Writes db/career.json and db/career-lang.json.

usage: career.py <dir of language_<code>.tsv dumps (key<TAB>value, \\n escaped)>
"""
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ADDON = os.path.dirname(HERE)

ROUNDS = [
    ("Phoenix", "las_vegas", "rw_las_vegas_26", 11, 3),
    ("Interlagos", "brazil", "rw_brazil_26", 25, 3),
    ("Imola", "imola", "rw_imola", 13, 5),
    ("Monaco", "monaco", "rw_monaco_26", 27, 5),
    ("Montreal", "montreal", "rw_montreal_26", 10, 6),
    ("Mexico City", "mexico", "rw_mexico_26", 24, 6),
    ("Paul Ricard", "austria", "rw_austria_26", 8, 7),
    ("Silverstone", "silverstone", "rw_silverstone_26", 15, 7),
    ("Hockenheim", "shanghai", "rw_shanghai_26", 29, 7),
    ("Hungaroring", "hungaroring", "rw_hungaroring_26", 12, 8),
    ("Spa", "spa_francorchamps", "rw_spa_francorchamps_26", 26, 8),
    ("Monza", "monza", "rw_monza_26", 9, 9),
    ("Estoril", "madrid", "rw_madrid_26", 23, 9),
    ("Jerez", "catalunya", "rw_catalunya_26", 30, 9),
    ("Suzuka", "suzuka", "rw_suzuka_26", 21, 10),
    ("Adelaide", "melbourne", "rw_melbourne_26", 4, 11),
]

BASE_DATASET_TRACKS = {"imola"}

POINTS = [9, 6, 4, 3, 2, 1]
HALF_POINTS = [4, 3, 2, 1, 1, 0]
POINT_SLOTS = 26

MODE = "game_modes.ModeChampionship[_metadata_=driver_career_26]"
CHAMPIONSHIP = "championships.Championship[_metadata_=F1_driver_career_26]"
LENGTHS = "schedule.dynamic_event_data.default_ui_track_list_data"
STANDINGS = "game_setup.session_standings.session_standing_preset[_metadata_=standard_race]"
AGENT = "entrants.characters.character[_metadata_=elena]"

TRACK_LIST = "F1_1990"
EVENT_LIST = "F1_career_1990"
SAVE_NAME = "driver_career_1990"

SEASON_EVENTS = [
    ("start_season_ref", "start_season_2026"),
    ("end_season_ref", "end_season_2026"),
    ("season_break_ref", "season_break_2026"),
    ("car_reveal_ref", "car_reveal_2026"),
    ("sliding_scale_refresh_ref", "sliding_scale_refresh_26"),
]

AGENT_NAME = {"jap": "フラビオ", "zhs": "弗拉维奥"}
AGENT_FULL_NAME = {"jap": "フラビオ・ブリアトーレ", "zhs": "弗拉维奥·布里亚托雷"}
STOCK_NAME = {"jap": "エレナ", "zhs": "埃琳娜"}
INTRO_KEYS = re.compile(r"^lng_subtitle_agt_intro_")
CONTENT_TILE_KEYS = ("lng_career_dlc_content", "lng_career_dlc_content_description")
CONTENT_TILE_ENG = {"lng_career_dlc_content_description": "Play the 1990 season: 1990 teams, drivers, calendar and rules."}


def setop(select, field, value, **extra):
    op = {"op": "set", "select": select, "field": field, "value": str(value)}
    op.update(extra)
    return op


def points(values):
    return ",".join(str(v) for v in values + [0] * (POINT_SLOTS - len(values)))


def track_list_ops():
    fields = ["_metadata_", "sort_by_date", "_fixed_event_reference_instancecount_"]
    for k in range(len(ROUNDS)):
        fields += [f"fixed_event_reference_{k}._metadata_", f"fixed_event_reference_{k}.event_name"]
    row = {"_metadata_": TRACK_LIST, "sort_by_date": "False"}
    for k, (_, track, event, _, _) in enumerate(ROUNDS):
        row[f"fixed_event_reference_{k}._metadata_"] = f"fixed_event_reference_rw_{track}"
        row[f"fixed_event_reference_{k}.event_name"] = event
    op = {
        "op": "add-row",
        "table": "schedule.track_lists",
        "from": "schedule.track_lists.track_event_list[_metadata_=F1_2026_16]",
        "set": row,
    }
    return fields, op


def event_list_ops():
    s = "season_0"
    fields = ["_metadata_", "_season_instancecount_", f"{s}._metadata_", f"{s}.track_event_list",
              f"{s}._date_time_instancecount_", f"{s}._fixed_event_reference_instancecount_"]
    for d in range(2):
        fields += [f"{s}.date_time_{d}.{f}" for f in
                   ("_metadata_", "day", "daylight_saving", "hour", "minute", "month", "second", "year")]
    for k in range(len(SEASON_EVENTS)):
        fields += [f"{s}.fixed_event_reference_{k}._metadata_", f"{s}.fixed_event_reference_{k}.event_name"]
    row = {
        "_metadata_": EVENT_LIST,
        "_season_instancecount_": "1",
        f"{s}._fixed_event_reference_instancecount_": str(len(SEASON_EVENTS)),
        f"{s}.date_time_0.year": "{{career_year}}",
        f"{s}.date_time_1.year": "{{career_year}}",
    }
    for k, (ref, event) in enumerate(SEASON_EVENTS):
        row[f"{s}.fixed_event_reference_{k}._metadata_"] = ref
        row[f"{s}.fixed_event_reference_{k}.event_name"] = event
    op = {
        "op": "add-row",
        "table": "schedule.event_lists",
        "from": "schedule.event_lists.event_list[_metadata_=F1_career_26]",
        "set": row,
    }
    return fields, op


def career_ops():
    ops = []
    ops += [
        setop(MODE, "save_data.name", SAVE_NAME),
        setop(MODE, "event_list", EVENT_LIST),
        setop(MODE, "championship_setups.ChampionshipSetupList_0.ChampionshipSetup_0.event_list", EVENT_LIST),
        setop(CHAMPIONSHIP, "event_list", EVENT_LIST),
    ]
    for length in ("F1_2026_Long", "F1_2026_Medium", "F1_2026_Short"):
        row = f"{LENGTHS}.custom_dynamic_available_track_list_data[_metadata_={length}]"
        ops += [
            setop(row, "max_num_tracks", len(ROUNDS)),
            setop(row, "track_list_name", TRACK_LIST),
            setop(row, "weekend_to_add_transfer_window", -1),
            setop(row, "language_string", "lng_f1990_season_length"),
        ]
    selection = f"{LENGTHS}.track_selections.tracks_selection[_metadata_=F1_2026]"
    ops += [
        setop(selection, "default", TRACK_LIST),
        setop(selection, "season_0.track_event_list", TRACK_LIST),
    ]
    for _, track, _, day, month in ROUNDS:
        dataset = "f1" if track in BASE_DATASET_TRACKS else "f1_2026"
        row = (f"track_data.game_setup.TrackDataSet[_metadata_={dataset}]"
               f".TrackData[_metadata_={track}].TrackGeneralData_0")
        ops += [
            setop(row, "weekend_day", day),
            setop(row, "weekend_month", month),
            setop(row, "weekend_year", "{{career_year}}"),
        ]
    ops += [
        setop(STANDINGS, "points", points(POINTS)),
        setop(STANDINGS, "fastest_lap_bonus", 0),
        setop(STANDINGS, "fastest_lap_rule", "None"),
    ]
    for k in range(3):
        row = f"{STANDINGS}.reduced_points_data_{k}"
        ops += [
            setop(row, "points", points(HALF_POINTS)),
            setop(row, "fastest_lap_bonus", 0),
            setop(row, "fastest_lap_rule", "None"),
        ]
    ops += [
        setop(AGENT, "full_name", "lng_f1990_agent_full_name"),
        setop(AGENT, "last_name", "lng_f1990_agent_last_name"),
    ]
    return ops


def write_json(path, doc):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=1, ensure_ascii=False)
        f.write("\n")


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
    codes = sorted(n[len("language_"):-len(".tsv")] for n in os.listdir(lang_dir)
                   if n.startswith("language_") and n.endswith(".tsv"))
    for code in codes:
        table = read_tsv(os.path.join(lang_dir, f"language_{code}.tsv"))
        stock = STOCK_NAME.get(code, "Elena")
        name = AGENT_NAME.get(code, "Flavio")
        full = AGENT_FULL_NAME.get(code, "Flavio Briatore")
        pattern = re.compile(re.escape(stock) if code in STOCK_NAME else r"\bElena\b")
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
        for key in CONTENT_TILE_KEYS:
            if key in table:
                new = CONTENT_TILE_ENG.get(key) if code == "eng" and key in CONTENT_TILE_ENG else table[key].replace("2026", "1990")
                ops.append({"op": "lang.set", "language": code, "key": key, "value": new})
    return ops


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    tl_fields, tl_op = track_list_ops()
    el_fields, el_op = event_list_ops()
    write_json(os.path.join(ADDON, "db", "career.json"), {
        "schema_version": 1,
        "fields": {
            "schedule.track_lists.track_event_list": tl_fields,
            "schedule.event_lists.event_list": el_fields,
        },
        "ops": [tl_op, el_op] + career_ops(),
    })
    write_json(os.path.join(ADDON, "db", "career-lang.json"), {
        "schema_version": 1,
        "ops": lang_ops(sys.argv[1]),
    })


if __name__ == "__main__":
    main()
