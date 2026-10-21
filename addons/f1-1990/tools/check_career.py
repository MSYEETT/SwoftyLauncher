#!/usr/bin/env python3
"""Checks a built database against the stock one: the 2025 and 2026 career rows must be unchanged.

usage: check_career.py <stock gamedatabases.2025.bin> <built gamedatabases.2025.bin>

Protected: every stock row of the tables the 1990 career clones from (mode rows, championships, event lists, track
lists, season lengths, track selections, the eight career data-group tables), the points presets, the 2025/2026 track
datasets, the content sets and the customisation car groups. Allowed changes: the row counts of the tables that gain
a 1990 row, content set 11's two lists (stock value plus the 1990 name) and the agent's two name fields. Exit code 1
when a protected key changed or went missing, or a 1990 row is missing.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import career  # noqa: E402

TABLES = [(c, w) for c, w in career.DATA_GROUP_TABLES] + [
    ("game_modes", "ModeChampionship"),
    ("championships", "Championship"),
    ("schedule.event_lists", "event_list"),
    ("schedule.track_lists", "track_event_list"),
    (career.LENGTHS, "custom_dynamic_available_track_list_data"),
    (f"{career.LENGTHS}.track_selections", "tracks_selection"),
    ("game_setup.session_standings", "session_standing_preset"),
    ("content_sets", "content_set"),
    ("customisation.customisation_cars", "customisation_car_group"),
]
DATASETS = ("f1", "f1_2025", "f1_2026", "sprint_f1_2026")
NEW_ROWS = [(c, w, career.DATA_GROUP) for c, w in career.DATA_GROUP_TABLES] + [
    ("game_modes", "ModeChampionship", career.MODE),
    ("championships", "Championship", career.CHAMPIONSHIP),
    ("schedule.event_lists", "event_list", career.EVENT_LIST),
    ("schedule.track_lists", "track_event_list", career.TRACK_LIST),
    (career.LENGTHS, "custom_dynamic_available_track_list_data", "F1_1990_Long"),
    (career.LENGTHS, "custom_dynamic_available_track_list_data", "F1_1990_Medium"),
    (career.LENGTHS, "custom_dynamic_available_track_list_data", "F1_1990_Short"),
    (f"{career.LENGTHS}.track_selections", "tracks_selection", career.TRACK_SELECTION),
]


def h(path):
    return career.gamedb.fnv1a64(path.encode())


def protected(db):
    out = {}
    for container, word in TABLES:
        for i in range(db.count(container, word) or 0):
            row = f"{container}.{word}_{i}"
            label = db.get(f"{row}._metadata_")
            for field in db.fields(container, word, label):
                out[h(f"{row}.{field}")] = f"{row}.{field} ({label})"
    ts = "track_data.game_setup"
    for i in range(db.count(ts, "TrackDataSet") or 0):
        label = db.get(f"{ts}.TrackDataSet_{i}._metadata_")
        if label in DATASETS:
            for field in db.fields(ts, "TrackDataSet", label):
                out[h(f"{ts}.TrackDataSet_{i}.{field}")] = f"{ts}.TrackDataSet_{i}.{field} ({label})"
    return out


def allowed(stock):
    ok = {}
    for container, word in [(c, w) for c, w, _ in NEW_ROWS]:
        ok[h(f"{container}._{word}_instancecount_")] = None
    ci = stock.row_index("content_sets", "content_set", career.CONTENT_SET)
    for field, name in (("available_game_modes_0.game_mode_championships", career.MODE),
                        ("available_game_setup_modes_0.setup_modes", career.SETUP_MODE)):
        path = f"content_sets.content_set_{ci}.{field}"
        ok[h(path)] = f"{stock.get(path)},{name}"
    ai = stock.row_index("entrants.characters", "character", "elena")
    for field in ("full_name", "last_name"):
        ok[h(f"entrants.characters.character_{ai}.{field}")] = None
    return ok


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    stock, built = career.Db(sys.argv[1]), career.Db(sys.argv[2])
    guard, ok = protected(stock), allowed(stock)
    bad, other = [], 0
    for key, (tag, value) in stock.d.items():
        new = built.d.get(key)
        if new == (tag, value):
            continue
        if key in ok and new is not None:
            want = ok[key]
            if want is not None and new[1] != want:
                bad.append(f"{guard.get(key, hex(key))}: {value!r} -> {new[1]!r}, want {want!r}")
            continue
        if key in guard:
            bad.append(f"{guard[key]}: {value!r} -> {new[1] if new else 'MISSING'!r}")
        else:
            other += 1
    for container, word, label in NEW_ROWS:
        try:
            built.row_index(container, word, label)
        except KeyError:
            bad.append(f"missing 1990 row {container}.{word}[_metadata_={label}]")
    added = sum(1 for k in built.d if k not in stock.d)
    print(f"protected keys: {len(guard)}, added keys: {added}, other changed stock keys (other lanes): {other}")
    if bad:
        print(f"FAIL: {len(bad)} protected change(s)")
        for line in bad[:50]:
            print("  " + line)
        sys.exit(1)
    print("ok: 2025 and 2026 career rows unchanged")


if __name__ == "__main__":
    main()
