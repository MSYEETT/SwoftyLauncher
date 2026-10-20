#!/usr/bin/env python3

import json
from pathlib import Path

ADDON = Path(__file__).resolve().parent.parent

INT, STRING, ENUM, ID, BOOL, LIST, SUBSET = 0, 2, 3, 4, 5, 13, 18

MODE = "game_setup.game_setup_modes.game_setup_mode[_metadata_=f1_26]"
WEEKEND = f"{MODE}.weekend_options_0.weekend_structure[_metadata_=default]"
SESSIONS = f"{WEEKEND}.session_collection_0"
GROUPS = f"{WEEKEND}.session_group_collection_0"
DATASET = "track_data.game_setup.TrackDataSet[_metadata_=f1_2026]"
TAG = ("options_system.context.tag_folder[_metadata_=game_setup_mode]"
       ".game_setup_mode_tag[_metadata_=gamesetup_f1].restrictions_0")
OPTIONS = "options_system.options.game.options_folder[_metadata_=difficulty_options]"

CALENDAR = [
    "las_vegas", "brazil", "imola", "monaco", "montreal", "mexico", "austria", "silverstone",
    "catalunya", "hungaroring", "spa_francorchamps", "monza", "madrid", "suzuka", "melbourne",
]
TRACKS = CALENDAR + [
    "shanghai", "bahrain", "jeddah", "miami", "zandvoort", "baku", "singapore", "texas", "losail",
    "abu_dhabi", "silverstone_reverse", "austria_reverse", "zandvoort_reverse", "portimao",
]
SPRINT_ROWS = ["montreal", "silverstone", "zandvoort", "singapore"]
INHERITED_SPRINT_ROWS = ["shanghai", "miami"]

SESSION_ROWS = [
    ("practice1", "practice_1", "race", ""),
    ("practice2", "practice_2", "race", ""),
    ("short_practice", "practice_s", "race", ""),
    ("short_sprint_shootout", "sprint_shootout_s", "race", ""),
    ("one_shot_sprint_shootout", "sprint_shootout_o", "race", ""),
    ("short_qualifying", "qualifying_s", "race", ""),
    ("one_shot_qualifying", "qualifying_o", "race", ""),
    ("race", "race", "not_set", "standard_race"),
]

LENGTH_GROUPINGS = [
    ("none", "none", ""),
    ("one_shot_qualifying", "one_shot", "{one_shot}"),
    ("short_qualifying", "short", "{short}"),
    ("full_qualifying", "full", "{short}"),
]

ALLOCATION_PRESETS = [
    "softer", "balanced", "harder", "softer_sprint", "balanced_sprint", "harder_sprint",
    "softer_monaco", "balanced_monaco", "harder_monaco", "softer_sprint_monaco",
    "balanced_sprint_monaco", "harder_sprint_monaco",
]
ALLOCATION_SESSIONS = [
    "Practice1", "Practice2", "Practice3", "Qualifying1", "Qualifying2", "Qualifying3",
    "Race", "Race2", "Race3", "SprintShootout1", "SprintShootout2", "SprintShootout3",
]
SOFT_SESSIONS = {"Practice1", "Practice2", "SprintShootout1", "Qualifying1", "Race"}
NONE = "0,0,0,0,0"


def set_(select, field, value, type_=None):
    op = {"op": "set", "select": select, "field": field, "value": value}
    if type_ is not None:
        op["create"] = True
        op["type"] = type_
    return op


def weekend():
    ops = []
    for i, (label, session_id, penalties, standings) in enumerate(SESSION_ROWS):
        row = f"{SESSIONS}.session_{i}"
        ops += [
            set_(row, "_metadata_", label),
            set_(row, "session_id", session_id),
            set_(row, "session_to_apply_penalties_to", penalties),
            set_(row, "standings", standings),
        ]
    ops.append(set_(SESSIONS, "_session_instancecount_", str(len(SESSION_ROWS))))

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
        set_(saturday, "_qualifying_session_length_grouping_instancecount_", str(len(LENGTH_GROUPINGS)), INT),
    ]
    for i, (label, available_in, sessions) in enumerate(LENGTH_GROUPINGS):
        row = f"{saturday}.qualifying_session_length_grouping_{i}"
        ops += [
            set_(row, "_metadata_", label, STRING),
            set_(row, "available_in", available_in, LIST),
            set_(row, "sessions", sessions.format(one_shot="qualifying_o", short="qualifying_s"), LIST),
        ]
    ops.append(set_(GROUPS, "_qualifying_session_group_instancecount_", "2"))

    for track in SPRINT_ROWS:
        ops.append(set_(f"{DATASET}.TrackData[_metadata_={track}].TrackAuthenticWeekendData_0",
                        "weekend_format", "default"))
    for track in INHERITED_SPRINT_ROWS:
        row = f"{DATASET}.TrackData[_metadata_={track}]"
        ops += [
            set_(row, "_TrackAuthenticWeekendData_instancecount_", "1", INT),
            set_(f"{row}.TrackAuthenticWeekendData_0", "_metadata_", "TrackAuthenticWeekendData", STRING),
            set_(f"{row}.TrackAuthenticWeekendData_0", "weekend_format", "default", ID),
        ]
    return ops


def tyres():
    ops = []
    for preset in ALLOCATION_PRESETS:
        row = f"tyres.allocation_tyre_presets.TyresPerSessionPreset[_metadata_={preset}]"
        for session in ALLOCATION_SESSIONS:
            value = "{{soft_sets}},0,0,0,0" if session in SOFT_SESSIONS else NONE
            ops.append(set_(row, session, value))
    for preset in ["TyresPerSessionPreset", "TyresPerSessionSprintPreset"]:
        row = f"tyres.mandatory_tyre_presets.TyresPerSessionPreset[_metadata_={preset}]"
        ops += [set_(row, session, NONE) for session in ALLOCATION_SESSIONS]
    for track in TRACKS:
        tyre = f"{DATASET}.TrackData[_metadata_={track}].TyreData_0"
        ops += [set_(tyre, f"compound_{c}", "C4") for c in ("softer", "medium", "harder")]
        ops += [
            set_(tyre, "_MandatoryPitstopsOverride_instancecount_", "1", INT),
            set_(f"{tyre}.MandatoryPitstopsOverride_0", "_metadata_", "MandatoryPitstopsOverride", STRING),
            set_(f"{tyre}.MandatoryPitstopsOverride_0", "required_pitstops", "0", INT),
            set_(f"{tyre}.MandatoryPitstopsOverride_0", "session", "race", ID),
        ]
    for name in ("Soft_f1_26", "Medium_f1_26", "Hard_f1_26"):
        visual = f"tyres.visual_compounds.VisualCompound[_metadata_={name}]"
        for field, value in [
            ("material_context", "SuperSoft"),
            ("material_anim", "tyre_compound_key_super_soft"),
            ("audio_id", "SuperSoft"),
            ("icon_code", ":"),
            ("colour", "-1762525"),
            ("texture_name", "Soft_f1_26"),
            ("display_name", "lng_tyre_compound_supersoft"),
            ("title", "lng_tyre_compound_supersoft_title"),
            ("desc", "lng_tyre_compound_supersoft_desc"),
        ]:
            ops.append(set_(visual, field, value))
    ops.append(set_(MODE, "has_tyre_warmers", "False"))
    return ops


def race_control():
    ops = [
        set_(f"{TAG}.value_subset[_metadata_=qualifying_length]", "values",
             "qualifying_length,short_esports,full_f2,full"),
        set_(f"{TAG}.value_subset[_metadata_=weekend_structure]", "values", "weekend_structure,authentic,sprint"),
        set_(f"{TAG}.value_subset_15", "_metadata_", "safety_car_frequency", STRING),
        set_(f"{TAG}.value_subset_15", "values", "safety_car_frequency,reduced,standard,increased", SUBSET),
        set_(f"{TAG}.value_subset_16", "_metadata_", "parc_ferme_rules", STRING),
        set_(f"{TAG}.value_subset_16", "values", "parc_ferme_rules,on", SUBSET),
        set_(TAG, "_value_subset_instancecount_", "17"),
    ]
    for option, default, others in [
        ("safety_car_frequency", "off", ["reduced", "standard", "increased"]),
        ("parc_ferme_rules", "off", ["on"]),
    ]:
        row = f"{OPTIONS}.option[_metadata_={option}]"
        ops.append(set_(f"{row}.value[_metadata_={default}]", "is_default", "True"))
        ops += [set_(f"{row}.value[_metadata_={v}]", "is_default", "False") for v in others]
    ops += [
        set_("options_system.restrictions.restrictions_folder[_metadata_=session_lengths]"
             ".restrictions[_metadata_=short_race_f1].value_subset[_metadata_=red_flag_frequency]",
             "values", "red_flag_frequency"),
        set_("options_system.restrictions.inter_value_restriction[_metadata_=rules_and_flags]"
             ".restrictions.value_subset[_metadata_=red_flag_frequency]", "values", "red_flag_frequency"),
    ]
    grid_cutscene = set_(MODE, "show_grid_cutscene", "False")
    grid_cutscene["when"] = {"grid_cutscene": False}
    ops.append(grid_cutscene)

    group = "drs.drs_allowed_lap.drs_allowed_lap_group"
    ops.append({
        "op": "add-row", "table": "drs.drs_allowed_lap",
        "from": f"{group}[_metadata_=f1_2024]",
        "set": {
            "_metadata_": "f1_1990",
            "drs_allowed_lap_0.lap_number": "200",
            "drs_allowed_lap_1.lap_number": "200",
            "drs_allowed_lap_2.lap_number": "200",
        },
    })
    ops.append(set_(MODE, "drs_allowed_lap_group", "f1_1990"))
    return ops


def text():
    strings = {
        "lng_sprint_shootout": "Friday Qualifying",
        "lng_sprintshootout": "Friday Qualifying",
        "lng_sprintshootout_single": "Friday Qualifying",
        "lng_sprintshootout_single_short": "FQ",
        "lng_sprintshootout_oneshot": "One-Shot Friday Qualifying",
        "lng_sprintshootout_oneshot_short": "OSFQ",
        "lng_sprintshootout_results": "Friday Qualifying Results",
        "lng_qualifying_results": "Saturday Qualifying Results",
        "lng_qualifying_length_short": "Saturday Qualifying",
        "lng_qualifying_length_one_shot": "One-Shot Saturday Qualifying",
    }
    return [{"op": "lang.set", "language": "*", "key": k, "value": v} for k, v in strings.items()]


def main():
    fields = {
        "drs.drs_allowed_lap.drs_allowed_lap_group": [
            "_metadata_", "_drs_allowed_lap_instancecount_",
            "drs_allowed_lap_0._metadata_", "drs_allowed_lap_0.lap_number",
            "drs_allowed_lap_1._metadata_", "drs_allowed_lap_1.lap_number",
            "drs_allowed_lap_2._metadata_", "drs_allowed_lap_2.lap_number",
        ],
    }
    rules = {"schema_version": 1, "fields": fields, "ops": weekend() + tyres() + race_control()}
    (ADDON / "db" / "rules.json").write_text(json.dumps(rules, indent=1) + "\n")
    (ADDON / "db" / "rules_text.json").write_text(
        json.dumps({"schema_version": 1, "ops": text()}, indent=1) + "\n")


if __name__ == "__main__":
    main()
