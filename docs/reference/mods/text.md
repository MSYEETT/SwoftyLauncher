---
title: Changing in-game text
description: Show your own names and descriptions in menus with stock text tags, literal text and lang operations.
---

# Changing in-game text

The game shows text by tag, such as `lng_team_tor_hashtag`. Database fields such as `name_loc_tag` and `desc_loc_tag` name the tag to show. The strings for each tag are in the game's language files (`.lng`), one per language.

## Use a stock tag

Set a `_loc_tag` field to a tag the game already ships. It shows in every language.

```json
{ "op": "set", "select": "customisation.custom_se_liveries.se_liveries.se_livery[_metadata_=toro_rosso_rm_alphatauri_2026]",
  "field": "name_loc_tag", "value": "lng_team_tor_hashtag" }
```

A choice setting can offer several tags, as the [livery example](./livery#_1-the-manifest) does with `picker_name`.

## Track names

Menus read a track's name from `TrackDataSet_<n>.TrackData_<m>.TrackNameData_0` in each dataset, not from the track row. See [Rename a track](./database#rename-a-track).

## Add or change strings

`operations.assets` files carry two text ops. swofty_core adds the strings while the game boots, and the game's language files stay stock. Without the in-game loader, `swofty build` lists them as waiting.

| Op | Fields | Change |
|---|---|---|
| `lang.set` | `language` (a language code, or `*` for all), `key`, `value` | Set one string |
| `lang.table` | `language`, `source` | Load a whole `.lng` file from the addon |

```json [assets/text.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "lang.set", "language": "*", "key": "lng_team_tor_hashtag", "value": "AlphaTauri" }
  ]
}
```

A key the game does not have is added, and a `_loc_tag` field can name it. Where two addons set the same key, the later one in the load order wins. See [Loading mods at runtime](/reference/addons/runtime-assets#add-or-change-strings).

## Language files

`swofty-formats` reads, writes and diffs `.lng` files (`swofty_formats::lng`), and extracts the stock language files from the archive indexed in `F1_25.exe` (`swofty_formats::nefs`). `Lng::get`, `Lng::set`, `Lng::remove` and `Lng::diff` work on one file.
