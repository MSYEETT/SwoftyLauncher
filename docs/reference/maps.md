# How maps are installed

This page is for contributors. Players should read [Importing maps](/using/importing-maps).

`swofty apply` turns every map enabled in the profile into a playable track. The code is in `packages/core/src/activation/`.

## The track archive

The game loads an asset group only when its asset manifest (`fileList.manifest`) lists it. A listed group `asset_groups/<path>.assetgroup` is the file `2025_asset_groups/<path>.erp`. A new track's own paths, such as `environment_package/tracks/okayama/wep/okayama_common.assetgroup`, are not listed.

So the package's `<code>.erp` goes whole into an existing invitational event slot, `2025_asset_groups/ui_package/texture_arrays/invitational_events/event_images/invitational_image_<n>.erp`, together with the slot's own pictures. The maps of a profile take `invitational_image_4`, `_10`, `_13` to `_18` in order.

The archive also gets:

- a stand-in for each variant world the package lacks (`classic`, `f2_previous`, ...), copied from the nearest world it has;
- texture mip paths pointed at `environment_package/tracks/<code>/mipmaps/<file>` for the files in the package's `mipmaps/`. Those files are added there;
- room for the F1 2026 grid when the built database's `f1_2026` grid list holds more cars than the `f1_26_myteam` world (with the common world) has grid slots, garages or pit boxes. The layer that holds the last grid slot gets copies of the last slot, garage, in-garage spot and pit box, each placed one row or bay on from the two before it. If the world has no two slots, garages or boxes to space from, the build warns that a Grand Prix there with that many cars crashes.

## The database

| Table | Change |
|---|---|
| `tracks.Track_<n>` | `copy_slot` clones the slot track's row for a new code, with a new `statId`. A code that already has a row (`bahrain24`) keeps it. `track_overrides` are set here. |
| `tracks.track_resource.TrackResource_<n>` | A row `swofty_<code>` cloned from the track's resource row, with every asset group set to the map's slot. The track row's `track_resource` names it. |
| Per-track tables: per-race DNFs, pit lane splines and slide settings, practice programmes (2025 and 2026), tyre wear and tyre wear scales, specialist objectives, track trivia, replay hero shots | A code without a row gets one cloned from the slot track, or from the base track when the slot has none. The game reads the per-race DNF row without checking it exists, so a new code always needs it. |
| `<datasets>.TrackDataSet_<i>.TrackData_<j>` | `copy_from_track` clones the base track's row, with its sub-tables, in every dataset that has it. `overrides` are set here, limited to their `datasets`. |
| Content sets' `available_tracks` | The code is added wherever the base track is listed. This is the Grand Prix track list. |
| `track_data.sorted_track_order` | A `TrackRef` row for the code. |

Every cloned row copies all keys of the source row listed in `track_tables.json`, plus the keys the manifest's overrides name, and the table's `_<row>_instancecount_` grows by one.

Some sub-table names exist in `F1_25.exe` only as strings built on the stack. `tools/track_keys.py` collects every name the game-DB readers use, stack-built ones included.

## Menus

- Flags, previews and splines from `ui/images.json` are appended as slices to the texture arrays named in `overrides.uidb`. Every UI archive that carries an array gets the same slices. The track's `TrackTextures` rows then name `overrides.<list>.<code>`.
- The three track-select scenes and the session picture go into `invitational_image_2`, and `ui_dynamic_texture_loading.uidta` names them.
- `ui/minimap.json` becomes an entry in `minimap.uidb` (in `databases.erp` and `flow_race.erp`) whose textures load from the map's slot.

## ERS

The manifest's `ers` values are per-track `TrackSettings` entries in the car handling files, not in the database. Every 2026 handling file gets an entry for the code, cloned from the base track's, with the manifest's values. A track without an entry still loads; the cars use their default ERS settings.

## Checking a build

```sh
swofty build
python3 tools/map_check.py <overlay folder> --game <game folder> --maps <maps folder>
```

With `F1_25.exe` in the game folder (or `--exe`) and `numpy`, `pefile` and `capstone` installed, `map_check.py` also checks that every key the game reads on a source row (the slot track's Track row, its track resource, per-track rows and TrackData rows) exists on the map's row.

`cargo test -p swofty-core --test real_maps` activates the Sepang and Okayama packages in the database from `F1_25_DIR` and checks the row counts and cloned rows.
