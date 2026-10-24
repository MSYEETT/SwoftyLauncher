"""Season drivers' ratings, as a statistics dataset of their own for the seasons' setup mode.

A driver's statistics (DriverStatisticsHolder) hold datasets filtered by game setup mode and season; each driver of a
season gets a dataset DATASET for the host setup mode swofty_f1, a copy of his default dataset whose pace curve is
flat at his pace rating (data/performance.json, tools/performance.py). A driver of several seasons takes the median of
his ratings. The other statistics keep the default dataset's.
"""

import json
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PERFORMANCE = json.loads((ROOT / "data" / "performance.json").read_text())
HOLDERS = "driver_statistics.driver_statistics"
DATASET = "classic_seasons"
HOST_MODE = "swofty_f1"


def curve(rating):
    v = f"{rating / 100:.3f}"
    return f"@v2@minx:0,maxx:10,miny:0,maxy:100,0:{v},0:{v},1:{v},1:{v}"


def season_pace():
    ratings = {}
    for season in PERFORMANCE.values():
        for driver, rating in season["pace"].items():
            ratings.setdefault(driver, []).append(rating)
    return {d: statistics.median(r) for d, r in ratings.items()}


def dataset_ops():
    ops = []
    for driver, rating in sorted(season_pace().items()):
        holder = f"{HOLDERS}.DriverStatisticsHolder[driver={driver}]"
        ops.append({"op": "add-row", "table": holder, "from": f"{holder}.StatisticsDataSet[_metadata_=default]",
                    "set": {"_metadata_": DATASET, "game_setup_modes": HOST_MODE, "seasons": ""}})
        ops.append({"op": "set", "select": f"{holder}.StatisticsDataSet[_metadata_={DATASET}].Statistic[_metadata_=pace]",
                    "field": "evolution", "value": curve(rating)})
    return ops
