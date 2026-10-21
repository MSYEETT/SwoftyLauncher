# F1 1990 Cars (private)

The car models for F1 1990's `f1_1990` vehicle package. Private test content, not for release: the Codemasters classics (F1 2019/2020, from Jefry), ported with Jefry's F1modding3dtoolV1. Seven teams run a classic body recoloured in flat 1990 colour zones (base colours and the main sponsor blocks, no logos; specs in `/mnt/work/.mp45b/classic/zones/<team>.json`, built by `classic_layout.py --zones`). The archives are built locally and never committed (`erp/` is ignored).

What it adds, all under new paths, nothing replacing a game file:

- `2025_asset_groups/f1_1990_vehicle_package/teams/<folder>/wep/<folder>.erp`: the 13 cars, 13-inch rims, per-driver number plates, and the pod camera (T-cam) just ahead of each body's airbox with a 100° field of view (`POD_CAMERA` and `POD_FOV` in `tools/build_pack.py`; the offset and side pod cameras move with it and keep their own field of view).
- `2025_asset_groups/f1_1990_vehicle_package/teams/common.erp`: the package's shared resources, from the stock 2026 common.erp, with the 2026 tyre meshes reshaped for 13-inch rims (bead 0.175 m, outer radius 0.351 m front and 0.354 m rear, the stock 2026 radius; dry and wet, stock 2026 tyre materials and textures, no deformation data; `/mnt/work/.mp45b/porter_a/tyres_cm13.py`). The tyre set keeps its `f1_2026_vehicle_package` names and is the archive's only tyre set.
- `2025_asset_groups/f1_1990_vehicle_package/tyrecompounds/tyrecompounds.erp`: the 2026 compound file once, as `eaid://f1_1990_vehicle_package/tyrecompounds/f1_1990_compounds.vcmpd`, with its 16 compounds renamed from `F1_2026_*` to `F1_1990_*`. The `fom_car` handling files left in the car archives and in common.erp have `TyrePhysicsGroup` `F1_1990`.
- `2025_asset_groups/f1_1990_vehicle_b_specs/teams/<folder>/wep/<folder>.erp`: the 13 B-spec cars (the A body with gold rims, `gridc13b_*`). Their own resources carry the B package path; references to the shared resources and to the car's handling file point at the A package, so a B group lists the A common.erp and the A handling archive.
- Runtime asset groups (`assets/cars.json`): `asset_groups/f1_1990_vehicle_package/teams/<folder>/wep/<folder>.assetgroup` listing common.erp, the car archive and `<folder>_handling.erp` (shipped by `swofty.f1-1990`), and the tyre-compound group. Each takes its flags from the matching 2026 group.
- `videos/f1_1990_intro.bik`: the intro video that opens a new 1990 Driver Career (`native/f1_1990_intro.dll` in `swofty.f1-1990`, option `intro_video_1990`), from `video/f1_1990_intro.bik` (ignored, like `erp/`). Bring your own video: the build fails without that file.

| Team | Folder | Model | Source build |
|---|---|---|---|
| McLaren | mclaren | CM c_mclaren_90 | gridc13_mclaren |
| Ferrari | ferrari | CM c_ferrari_90 | gridc13_ferrari |
| Williams | williams | CM c_williams_92 | gridc13_williams |
| Benetton | lotus | CM c_benetton_94 | gridc13_lotus |
| Tyrrell | mercedes | CM c_jordan_91, Tyrrell colours | gridc13_mercedes |
| Lotus | redbull | CM c_mclaren_91, Lotus colours | gridc13_redbull |
| Leyton House | force_india | CM c_jordan_91, Leyton House colours | gridc13_march |
| Larrousse | myteam_ferrari | CM c_ferrari_90, Larrousse colours, on the Williams donor, stem `larrouss` | gridc13_larrousse |
| Ligier | sauber | CM c_mclaren_90, Ligier colours | gridc13_ligier_prost |
| Minardi | toro_rosso | CM c_jordan_91, Minardi colours | gridc13_minardi |
| Brabham | gm_cadillac | CM c_mclaren_91, Brabham colours | gridc13_brabham |
| Dallara | myteam_honda | CM c_williams_92, Dallara colours, on the Haas donor, stem `dall` | gridc13_dallara |
| Arrows | haas | CM c_ferrari_90, Arrows colours | gridc13_arrows |

## Building

```sh
python3 addons/f1-1990-cars/tools/build_pack.py            # all cars, common.erp, tyre compounds, assets/cars.json
python3 addons/f1-1990-cars/tools/build_pack.py --ops-only # only assets/cars.json and db/test.json
python3 addons/f1-1990-cars/tools/build_pack.py --bspec-only # only the B-spec archives
python3 addons/f1-1990-cars/tools/build_pack.py --common-only # only common.erp and the tyre compounds
```

`build_pack.py` renames each porter output from `eaid://f1_2026_vehicle_package/` to `eaid://f1_1990_vehicle_package/` byte for byte (same length), leaves the bare `.mipmaps` sidecar paths on the stock 2026 files, drops the car's own handling file (the f1-1990 addon ships it per option), and sets the AudioF1Car `switchName` in each carmodel layer to the car's 1990 engine audio label (`addons/f1-1990/tools/audio.py`: the donor's label in other letter case, such as `F1_M_mclaren_26`, which keeps the donor's Wwise switch state and names the car's own `audio.audio_car_data` row). Larrousse and Dallara sit on My Team folders for their AI data but were built on team donors: their resources carry a stem of the donor's length, and their World, event graph and layers are also stored under the folder name. The build fails if any `eaid://f1_2026_vehicle_package/` reference survives. Sources default to `/mnt/work/.mp45b/porter_a/tries` (`--tries`, `--common`, `--game`).

The intro video:

```sh
nix shell nixpkgs#ffmpeg nixpkgs#wine64 nixpkgs#xvfb-run nixpkgs#p7zip -c \
    addons/f1-1990-cars/tools/intro_video.sh <input video>   # writes video/f1_1990_intro.bik
```

`intro_video.sh` scales the input to 1280x720 (letterboxed; `WIDTH`, `HEIGHT`), resamples it to 29.97 fps and 48 kHz stereo, and encodes it as Bink 1 at `RATE` bytes per second (1500000) with `radvideo64.exe binkc` and `binkmix` under Wine. The game plays a `.bik` as Bink 1; Bink 2 encoding needs a licence. `RADVIDEO` points at an installed `radvideo64.exe`; otherwise the script downloads the free RAD Video Tools from radgametools.com into `~/.cache/f1-1990-intro`. RAD Video Tools are for non-commercial use only.

## Test option

`test_grand_prix` (default `off`) puts 1990 cars on the F1 2026 Grand Prix grid for testing before the 1990 career exists: `ferrari` (the two Ferrari seats), `all` (all 22 seats, each on the 1990 car of the same AI folder), `myteam` (Larrousse on the Williams seats, Dallara on the Haas seats) or `bspec` (all 22 seats on the B-spec cars). Off leaves Grand Prix stock.
