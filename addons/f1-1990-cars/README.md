# F1 1990 Cars (private)

The car models for F1 1990's `f1_1990` vehicle package. Private test content, not for release: the Codemasters classics (F1 2019/2020, from Jefry) and the ASR McLaren MP4/5B, ported with Jefry's F1modding3dtoolV1. The archives are built locally and never committed (`erp/` is ignored).

What it adds, all under new paths, nothing replacing a game file:

- `2025_asset_groups/f1_1990_vehicle_package/teams/<folder>/wep/<folder>.erp`: the 13 cars, 13-inch rims, per-driver number plates.
- `2025_asset_groups/f1_1990_vehicle_package/teams/common.erp`: the package's shared resources, with the MP4/5B 13-inch tyre set (dry and wet, Pirelli 2018 textures, no deformation data).
- `2025_asset_groups/f1_1990_vehicle_package/tyrecompounds/tyrecompounds.erp`: a copy of the 2026 compound file.
- `2025_asset_groups/f1_1990_vehicle_b_specs/teams/<folder>/wep/<folder>.erp`: the 13 B-spec cars (McLaren: ASR's late-1990 MP4/5B, `gridn13_mclaren_late`; the others: the A body with gold rims, `grid*13b_*`). Their own resources carry the B package path; references to the shared resources and to the car's handling file point at the A package, so a B group lists the A common.erp and the A handling archive.
- Runtime asset groups (`assets/cars.json`): `asset_groups/f1_1990_vehicle_package/teams/<folder>/wep/<folder>.assetgroup` listing common.erp, the car archive and `<folder>_handling.erp` (shipped by `swofty.f1-1990`), and the tyre-compound group. Each takes its flags from the matching 2026 group.

| Team | Folder | Model | Source build |
|---|---|---|---|
| McLaren | mclaren | ASR MP4/5B | gridn13_mclaren |
| Ferrari | ferrari | CM c_ferrari_90 | gridc13_ferrari |
| Williams | williams | CM c_williams_92 | gridc13_williams |
| Benetton | lotus | CM c_benetton_94 | gridc13_lotus |
| Tyrrell | mercedes | CM c_jordan_91, Tyrrell colours | gridc13_mercedes |
| Lotus | redbull | CM c_mclaren_91, Lotus colours | gridc13_redbull |
| Leyton House | force_india | MP4/5B | gridn13_force_india |
| Larrousse | myteam_ferrari | MP4/5B on the Williams donor, stem `larrouss` | gridn13_larrousse |
| Ligier | sauber | MP4/5B | gridn13_sauber |
| Minardi | toro_rosso | MP4/5B | gridn13_toro_rosso |
| Brabham | gm_cadillac | MP4/5B | gridn13_gm_cadillac |
| Dallara | myteam_honda | MP4/5B on the Haas donor, stem `dall` | gridn13_dallara |
| Arrows | haas | MP4/5B | gridn13_haas |

## Building

```sh
python3 addons/f1-1990-cars/tools/build_pack.py            # all cars, common.erp, tyre compounds, assets/cars.json
python3 addons/f1-1990-cars/tools/build_pack.py --ops-only # only assets/cars.json and db/test.json
python3 addons/f1-1990-cars/tools/build_pack.py --bspec-only # only the B-spec archives
```

`build_pack.py` renames each porter output from `eaid://f1_2026_vehicle_package/` to `eaid://f1_1990_vehicle_package/` byte for byte (same length), leaves the bare `.mipmaps` sidecar paths on the stock 2026 files, drops the car's own handling file (the f1-1990 addon ships it per option), and sets the AudioF1Car `switchName` in each carmodel layer to the car's 1990 engine audio label (`addons/f1-1990/tools/audio.py`, same length as the donor's). Larrousse and Dallara sit on My Team folders for their AI data but were built on team donors: their resources carry a stem of the donor's length, and their World, event graph and layers are also stored under the folder name. The build fails if any `eaid://f1_2026_vehicle_package/` reference survives. Sources default to `/mnt/work/.mp45b/porter_a/tries` (`--tries`, `--common`, `--game`).

## Test option

`test_grand_prix` (default `off`) puts 1990 cars on the F1 2026 Grand Prix grid for testing before the 1990 career exists: `ferrari` (the two Ferrari seats), `all` (all 22 seats, each on the 1990 car of the same AI folder), `myteam` (Larrousse on the Williams seats, Dallara on the Haas seats) or `bspec` (all 22 seats on the B-spec cars). Off leaves Grand Prix stock.
