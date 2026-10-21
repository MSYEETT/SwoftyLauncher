#!/usr/bin/env python3
"""Writes db/rules.json and db/rules_text.json: the f1_1990 setup mode and its rules.

usage: rules.py [stock gamedatabases.2025.bin]
"""

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ADDON = HERE.parent
sys.path.insert(0, str(ADDON.parent.parent / "tools"))
sys.path.insert(0, str(HERE))

import gamedb  # noqa: E402
from career import ROUNDS  # noqa: E402

STOCK_DB = "/mnt/work/.f1-1990/data-c26/fakegame/game_package/gamedatabases/gamedatabases.2025.bin"
FIELDS = json.loads((HERE / "rules_fields.json").read_text())

INT, STRING, ENUM, ID, BOOL, LIST, SUBSET = 0, 2, 3, 4, 5, 13, 18

NAME = "f1_1990"
MODE_STAT_ID = "16"
DATASET_STAT_ID = "20"
CLASS_STAT_ID = "12"
VISUAL_STAT_IDS = {"Supersoft_f1_1990": "28", "Supersoft_f1_1990_medium": "29", "Supersoft_f1_1990_harder": "30"}
ALLOCATION_STAT_ID = "14"
MANDATORY_STAT_ID = "8"
STANDINGS_STAT_ID = "15"
TAG_NAME = "gamesetup_f1_1990"
STANDINGS_NAME = "f1_1990_race"
ALLOCATION_NAME = "f1_1990"
MANDATORY_NAME = "f1_1990_mandatory"
YEAR = "1990"

MODES = "game_setup.game_setup_modes"
MODE = f"{MODES}.game_setup_mode[_metadata_={NAME}]"
WEEKEND = f"{MODE}.weekend_options_0.weekend_structure[_metadata_=default]"
SESSIONS = f"{WEEKEND}.session_collection_0"
GROUPS = f"{WEEKEND}.session_group_collection_0"
DATASETS = "track_data.game_setup"
DATASET = f"{DATASETS}.TrackDataSet[_metadata_={NAME}]"
TAG_FOLDER = "options_system.context.tag_folder[_metadata_=game_setup_mode]"
TAG = f"{TAG_FOLDER}.game_setup_mode_tag[_metadata_={TAG_NAME}].restrictions_0"
LAYERS = "track_data.track_layers.TrackLayer"
PROBES = "tracks.ibl_probes.IblProbes_0"

SESSION_ROWS = [
    ("practice1", "practice_1", "race", ""),
    ("practice2", "practice_2", "race", ""),
    ("short_practice", "practice_s", "race", ""),
    ("short_sprint_shootout", "sprint_shootout_s", "race", ""),
    ("one_shot_sprint_shootout", "sprint_shootout_o", "race", ""),
    ("short_qualifying", "qualifying_s", "race", ""),
    ("one_shot_qualifying", "qualifying_o", "race", ""),
    ("race", "race", "not_set", STANDINGS_NAME),
]
LENGTH_GROUPINGS = [
    ("none", "none", ""),
    ("one_shot_qualifying", "one_shot", "{one_shot}"),
    ("short_qualifying", "short", "{short}"),
    ("full_qualifying", "full", "{short}"),
]
ALLOCATION_SESSIONS = [
    "Practice1", "Practice2", "Practice3", "Qualifying1", "Qualifying2", "Qualifying3",
    "Race", "Race2", "Race3", "SprintShootout1", "SprintShootout2", "SprintShootout3",
]
SOFT_SESSIONS = {"Practice1", "Practice2", "SprintShootout1", "Qualifying1", "Race"}
SHOOTOUT_SESSIONS = {"SprintShootout1", "SprintShootout2", "SprintShootout3"}
NONE = "0,0,0,0,0"
SOFTER = "1,0,0,0,0"
POINTS = [9, 6, 4, 3, 2, 1]
HALF_POINTS = [4, 3, 2, 1, 1, 0]
POINT_SLOTS = 26
SUPERSOFT = {
    "material_context": "SuperSoft",
    "material_anim": "tyre_compound_key_super_soft",
    "audio_id": "SuperSoft",
    "icon_code": ":",
    "display_name": "lng_tyre_compound_supersoft",
    "title": "lng_tyre_compound_supersoft_title",
    "desc": "lng_tyre_compound_supersoft_desc",
}


class Stock:
    def __init__(self, path):
        self.recs = {r.key: r for r in gamedb.read(Path(path).read_bytes())}

    def get(self, key):
        r = self.recs.get(gamedb.fnv1a64(key))
        return None if r is None else (r.type, r.value.decode("utf-8"))

    def text(self, key):
        found = self.get(key)
        if found is None:
            raise SystemExit(f"stock database has no {key}")
        return found[1]

    def row(self, table, label):
        i = 0
        while (found := self.get(f"{table}_{i}._metadata_")) is not None:
            if found[1] == label:
                return f"{table}_{i}"
            i += 1
        raise SystemExit(f"stock database has no {table} row {label}")


def set_(select, field, value, type_=None, when=None):
    op = {"op": "set", "select": select, "field": field, "value": str(value)}
    if type_ is not None:
        op["create"] = True
        op["type"] = type_
    if when:
        op["when"] = when
    return op


def add_row(table, source, values):
    return {"op": "add-row", "table": table, "from": source, "set": {k: str(v) for k, v in values.items()}}


def points(values):
    return ",".join(str(v) for v in values + [0] * (POINT_SLOTS - len(values)))


def presets(fields):
    allocation = {"_metadata_": ALLOCATION_NAME, "stat_id": ALLOCATION_STAT_ID}
    mandatory = {"_metadata_": MANDATORY_NAME, "stat_id": MANDATORY_STAT_ID}
    for session in ALLOCATION_SESSIONS:
        allocation[session] = "{{soft_sets}},0,0,0,0" if session in SOFT_SESSIONS else NONE
        mandatory[session] = SOFTER if session in SHOOTOUT_SESSIONS else NONE
    fields["tyres.allocation_tyre_presets.TyresPerSessionPreset"] = FIELDS["allocation"]
    fields["tyres.mandatory_tyre_presets.TyresPerSessionPreset"] = FIELDS["mandatory"]
    return [
        add_row("tyres.allocation_tyre_presets",
                "tyres.allocation_tyre_presets.TyresPerSessionPreset[_metadata_=softer]", allocation),
        add_row("tyres.mandatory_tyre_presets",
                "tyres.mandatory_tyre_presets.TyresPerSessionPreset[_metadata_=TyresPerSessionPreset]", mandatory),
    ]


def compounds(fields):
    fields["tyres.visual_compounds.VisualCompound"] = FIELDS["visual"]
    fields["tyres.compound_classes.CompoundClass"] = FIELDS["compound_class"]
    ops = [
        add_row("tyres.visual_compounds", "tyres.visual_compounds.VisualCompound[_metadata_=Soft_f1_26]",
                {"_metadata_": name, "stat_id": stat_id, **SUPERSOFT})
        for name, stat_id in VISUAL_STAT_IDS.items()
    ]
    softer, medium, harder = VISUAL_STAT_IDS
    ops.append(add_row("tyres.compound_classes", "tyres.compound_classes.CompoundClass[_metadata_=f1_26]", {
        "_metadata_": NAME,
        "game_setup_modes": NAME,
        "weekend_formats": "default",
        "stat_id": CLASS_STAT_ID,
        "CompoundOptions_0.visual_compounds": softer,
        "CompoundOptions_1.visual_compounds": medium,
        "CompoundOptions_2.visual_compounds": harder,
    }))
    ops.append(set_(f"tyres.compound_classes.CompoundClass[_metadata_={NAME}]", "show_allocation_screen", "False",
                    when={"no_tyre_allocation": True}))
    return ops


def dataset(fields, stock):
    tracks = FIELDS["dataset_tracks"]
    row_fields = []
    values = {"_metadata_": NAME, "game_setup_modes": NAME, "parent_data_set": "f1_2026",
              "weekend_formats": "default", "stat_id": DATASET_STAT_ID}
    for i, _ in enumerate(tracks):
        row = f"TrackData_{i}"
        row_fields += [f"{row}._metadata_", f"{row}.track", f"{row}._TyreData_instancecount_"]
        row_fields += [f"{row}.TyreData_0.{f}" for f in FIELDS["tyre_data"]]
        for compound in ("softer", "medium", "harder"):
            values[f"{row}.TyreData_0.compound_{compound}"] = "C4"
        for preset in ("softer_preset", "balanced_preset", "harder_preset"):
            values[f"{row}.TyreData_0.{preset}"] = ALLOCATION_NAME
        values[f"{row}.TyreData_0.mandatory_tyres"] = MANDATORY_NAME
    fields[f"{DATASETS}.TrackDataSet"] = FIELDS["dataset_top"] + row_fields
    ops = [add_row(DATASETS, f"{DATASETS}.TrackDataSet[_metadata_=f1_2026]", values)]

    stock_2026 = stock.row(f"{DATASETS}.TrackDataSet", "f1_2026")
    stock_base = stock.row(f"{DATASETS}.TrackDataSet", "f1")
    dates = {track: (day, month) for _, track, _, day, month in ROUNDS}
    for i, track in enumerate(tracks):
        row = f"{DATASET}.TrackData[_metadata_={track}]"
        tyre = f"{row}.TyreData_0"
        ops += [
            set_(tyre, "_MandatoryPitstopsOverride_instancecount_", 1, INT),
            set_(f"{tyre}.MandatoryPitstopsOverride_0", "_metadata_", "MandatoryPitstopsOverride", STRING),
            set_(f"{tyre}.MandatoryPitstopsOverride_0", "required_pitstops", 0, INT),
            set_(f"{tyre}.MandatoryPitstopsOverride_0", "session", "race", ID),
            set_(row, "_TrackAuthenticWeekendData_instancecount_", 1, INT),
            set_(f"{row}.TrackAuthenticWeekendData_0", "_metadata_", "TrackAuthenticWeekendData", STRING),
            set_(f"{row}.TrackAuthenticWeekendData_0", "weekend_format", "default", ID),
        ]
        if track not in dates:
            continue
        source = f"{stock_2026}.TrackData_{i}"
        if stock.get(f"{source}._TrackGeneralData_instancecount_") is None:
            source = stock.row(f"{stock_base}.TrackData", track)
        day, month = dates[track]
        general = {"weekend_day": day, "weekend_month": month, "weekend_year": YEAR}
        ops.append(set_(row, "_TrackGeneralData_instancecount_", 1, INT))
        for field in FIELDS["general_data"]:
            type_, value = stock.get(f"{source}.TrackGeneralData_0.{field}")
            ops.append(set_(f"{row}.TrackGeneralData_0", field, general.get(field, value), type_))
    return ops


def drs(fields):
    fields["drs.drs_allowed_lap.drs_allowed_lap_group"] = FIELDS["drs"]
    return [add_row("drs.drs_allowed_lap", "drs.drs_allowed_lap.drs_allowed_lap_group[_metadata_=f1_2024]", {
        "_metadata_": NAME,
        "drs_allowed_lap_0.lap_number": 200,
        "drs_allowed_lap_1.lap_number": 200,
        "drs_allowed_lap_2.lap_number": 200,
    })]


def standings(fields):
    fields["game_setup.session_standings.session_standing_preset"] = FIELDS["standings"]
    values = {"_metadata_": STANDINGS_NAME, "stat_id": STANDINGS_STAT_ID, "points": points(POINTS),
              "fastest_lap_bonus": 0, "fastest_lap_rule": "None"}
    for k in range(3):
        values.update({f"reduced_points_data_{k}.points": points(HALF_POINTS),
                       f"reduced_points_data_{k}.fastest_lap_bonus": 0,
                       f"reduced_points_data_{k}.fastest_lap_rule": "None"})
    return [add_row("game_setup.session_standings",
                    "game_setup.session_standings.session_standing_preset[_metadata_=standard_race]", values)]


def option_tag(fields, stock):
    fields[f"{TAG_FOLDER}.game_setup_mode_tag"] = FIELDS["tag"]
    ops = [
        add_row(TAG_FOLDER, f"{TAG_FOLDER}.game_setup_mode_tag[_metadata_=gamesetup_f1]", {"_metadata_": TAG_NAME}),
        set_(f"{TAG}.value_subset[_metadata_=qualifying_length]", "values",
             "qualifying_length,short_esports,full_f2,full"),
        set_(f"{TAG}.value_subset[_metadata_=weekend_structure]", "values", "weekend_structure,authentic,sprint"),
        set_(f"{TAG}.value_subset_15", "_metadata_", "safety_car_frequency", STRING),
        set_(f"{TAG}.value_subset_15", "values", "safety_car_frequency,reduced,standard,increased", SUBSET),
        set_(f"{TAG}.value_subset_16", "_metadata_", "parc_ferme_rules", STRING),
        set_(f"{TAG}.value_subset_16", "values", "parc_ferme_rules,on", SUBSET),
        set_(TAG, "_value_subset_instancecount_", 17),
    ]
    for key in FIELDS["tag_lists"]:
        select, field = key.rsplit(".", 1)
        ops.append(set_(select, field, f"{stock.text(key)},{TAG_NAME}"))
    return ops


def mode(fields):
    fields[f"{MODES}.game_setup_mode"] = FIELDS["mode"]
    ops = [add_row(MODES, f"{MODES}.game_setup_mode[_metadata_=f1_26]", {
        "_metadata_": NAME,
        "stat_id": MODE_STAT_ID,
        "display_name": "lng_f1_1990",
        "options_tag": TAG_NAME,
        "has_tyre_warmers": "False",
        "drs_allowed_lap_group": NAME,
        "tyre_regulations.allow_strategy_mandatory_stop_check": "False",
    })]
    ops += [
        set_(MODE, "show_grid_cutscene", "False", when={"grid_cutscene": False}),
        set_(MODE, "grid_rundown_enabled", "False", when={"grid_rundown": False}),
    ]
    for i, (label, session_id, penalties, standing) in enumerate(SESSION_ROWS):
        row = f"{SESSIONS}.session_{i}"
        ops += [
            set_(row, "_metadata_", label),
            set_(row, "session_id", session_id),
            set_(row, "session_to_apply_penalties_to", penalties),
            set_(row, "standings", standing),
        ]
    ops.append(set_(SESSIONS, "_session_instancecount_", len(SESSION_ROWS)))
    practice = f"{GROUPS}.practice_session_group_0.practice_session_length_grouping[_metadata_=full_practice]"
    ops.append(set_(practice, "sessions", "practice_1,practice_2"))
    friday = f"{GROUPS}.qualifying_session_group_0"
    ops += [set_(friday, "_metadata_", "sprint_shootout"), set_(friday, "group", "SprintShootout")]
    for i, (label, available_in, sessions) in enumerate(LENGTH_GROUPINGS):
        row = f"{friday}.qualifying_session_length_grouping_{i}"
        ops += [
            set_(row, "_metadata_", label),
            set_(row, "available_in", available_in),
            set_(row, "sessions", sessions.format(one_shot="sprint_shootout_o", short="sprint_shootout_s")),
        ]
    saturday = f"{GROUPS}.qualifying_session_group_1"
    ops += [
        set_(saturday, "_metadata_", "qualifying_session_group", STRING),
        set_(saturday, "group", "Qualifying", ENUM),
        set_(saturday, "_qualifying_session_length_grouping_instancecount_", len(LENGTH_GROUPINGS), INT),
    ]
    for i, (label, available_in, sessions) in enumerate(LENGTH_GROUPINGS):
        row = f"{saturday}.qualifying_session_length_grouping_{i}"
        ops += [
            set_(row, "_metadata_", label, STRING),
            set_(row, "available_in", available_in, LIST),
            set_(row, "sessions", sessions.format(one_shot="qualifying_o", short="qualifying_s"), LIST),
        ]
    ops.append(set_(GROUPS, "_qualifying_session_group_instancecount_", 2))
    return ops


def appends(stock):
    layer = stock.row(LAYERS, "f1_26")
    return [
        set_(f"{LAYERS}[_metadata_=f1_26]", "game_setup_modes", f"{stock.text(f'{layer}.game_setup_modes')},{NAME}"),
        set_(PROBES, "game_setup_modes", f"{stock.text(f'{PROBES}.game_setup_modes')},{NAME}"),
    ]


def main():
    stock = Stock(sys.argv[1] if len(sys.argv) > 1 else STOCK_DB)
    fields = {}
    ops = (drs(fields) + standings(fields) + presets(fields) + compounds(fields) + dataset(fields, stock)
           + option_tag(fields, stock) + mode(fields) + appends(stock))
    rules = {"schema_version": 1, "fields": fields, "ops": ops}
    (ADDON / "db" / "rules.json").write_text(json.dumps(rules, indent=1, ensure_ascii=False) + "\n")
    text = {"schema_version": 1, "ops": [{"op": "lang.set", "language": "*", "key": "lng_f1_1990", "value": "F1 1990"}]}
    (ADDON / "db" / "rules_text.json").write_text(json.dumps(text, indent=1) + "\n")


if __name__ == "__main__":
    main()
