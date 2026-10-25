"""Overtake mode for the season cars.

In the 2026 cars, each handling file's EnergyRecoverySystem holds one TrackSettings entry per track: the lap
fractions where overtake mode is detected and where it can be used (OverrideDetectionLapProgress,
OverrideActivationLapProgress), with the track's deploy and harvest settings. The season cars' handling files come
from the 2025 cars, whose EnergyRecoverySystem has no TrackSettings, so overtake mode never opens for them.
`apply` gives every handling file of a season archive without TrackSettings the stock 2026 entries of its team, or
McLaren's for a team the 2026 cars don't have. `stock_settings` reads those entries from the game folder, which has
to hold the stock 2026 car archives. build_cars.py applies it on every build; run on its own, the tool applies it to
built cars.

    python3 tools/overtake.py [--years 2016-2024] [--game <F1 25 folder>]
"""

import argparse
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import erpw
import vtf

TEAMS = Path("2025_asset_groups/f1_2026_vehicle_package/teams")
FALLBACK = "mclaren"
ERS = b"EnergyRecoverySystem"
SETTINGS = b"TrackSettings"
COUNT = b"numTrackSettingsTypes"


def _tops(root):
    return [root] if root.name == b"VTF" else root.find(b"VTF")


def _settings(root):
    for top in _tops(root):
        for ers in top.find(ERS):
            if ers.find(SETTINGS):
                return ers.find(SETTINGS)
    return []


def stock_settings(game):
    out = {}
    for path in sorted((Path(game) / TEAMS).glob("*/wep/*.erp")):
        team = path.parent.parent.name
        if path.stem != team or team in out:
            continue
        e = erpw.Erp(str(path))
        for r in e.res:
            if r.name.endswith(f"/teams/{team}/handling/{team}.vtf"):
                entries = _settings(vtf.parse(e.frag_data(r, 0)))
                if entries:
                    out[team] = entries
    if FALLBACK not in out:
        raise SystemExit(f"{Path(game) / TEAMS}: no stock 2026 {FALLBACK} handling file with TrackSettings")
    return out


def _team(name):
    folder = name.split("/teams/", 1)[-1].split("/", 1)[0]
    for prefix in ("fom_car_", "myteam_"):
        folder = folder.removeprefix(prefix)
    return folder


def apply(e, settings):
    done = 0
    for r in e.res:
        if not r.name.endswith(".vtf"):
            continue
        root = vtf.parse(e.frag_data(r, 0))
        entries = settings.get(_team(r.name), settings[FALLBACK])
        added = False
        for top in _tops(root):
            for ers in top.find(ERS):
                if ers.find(SETTINGS):
                    continue
                ers.children.extend(n.clone() for n in entries)
                count = str(len(entries)).encode()
                if ers.attr(COUNT) is None:
                    ers.attrs.append((COUNT, count))
                else:
                    ers.set_attr(COUNT, count)
                added = True
        if not added:
            continue
        e.set_frag(r, 0, vtf.write(root))
        if len(r.frags) > 1:
            old = e.frag_data(r, 1)
            version = struct.unpack_from("<I", old, 4)[0] if len(old) >= 8 else 1
            e.set_frag(r, 1, vtf.tsmd(root, version))
        done += 1
    return done


def main():
    import pace
    from layout import cars_dir

    ap = argparse.ArgumentParser()
    ap.add_argument("--years", default=f"{min(pace.SEASONS)}-{max(pace.SEASONS)}")
    ap.add_argument("--game", default="/mnt/gaming/SteamLibrary/steamapps/common/F1 25")
    args = ap.parse_args()
    settings = stock_settings(args.game)
    for year in pace.parse_years(args.years):
        main_teams, extra = pace.teams(year)
        paths = [cars_dir(year) / "teams" / f / "wep" / f"{f}.erp" for f in main_teams]
        if extra:
            paths.append(cars_dir(year) / "extra" / "teams" / extra["folder"] / "wep" / f"{extra['folder']}.erp")
        for path in paths:
            e = erpw.Erp(str(path))
            done = apply(e, settings)
            if done:
                e.to_file(str(path))
            print(f"{year} {path.parent.parent.name}: {done} handling file(s) given overtake settings", flush=True)


if __name__ == "__main__":
    main()
