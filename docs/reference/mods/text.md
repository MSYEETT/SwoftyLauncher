---
title: In-game text
description: Show your own names and descriptions with stock tags, literal text, and lang operations that add strings while the game boots.
---

# In-game text

To change in-game text, you work with tags such as `lng_team_tor_hashtag`. Database fields such as `name_loc_tag` and `desc_loc_tag` tell the game which tag to show. The text itself lives in the game's language files (`.lng`), one per language.

You have two ways to change that text: point a field at a tag the game already has, or add your own string with a `lang` operation.

## Point a field at a stock tag

If the wording you want is already in the game, set a `_loc_tag` field to its tag. You get that text in every language without adding a string:

```json
{ "op": "set",
  "select": "customisation.custom_se_liveries.se_liveries.se_livery[_metadata_=toro_rosso_rm_alphatauri_2026]",
  "field": "name_loc_tag", "value": "lng_team_tor_hashtag" }
```

To offer a choice of tags, use a `choice` setting. The [livery example](./livery#_1-the-manifest) does this with `picker_name`.

## Add your own string

Put your `lang` operations in an `operations.assets` file. swofty_core adds the strings while the game boots, leaving the game's language files stock.

| Op | Fields | Change |
|---|---|---|
| `lang.set` | `language` (a code, or `*` for every language), `key`, `value` | Set one string |
| `lang.table` | `language`, `source` (a `.lng` file in the mod) | Set every string in a table |

```json [assets/text.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "lang.set", "language": "*", "key": "lng_my_track_title", "value": "Test Ring" },
    { "op": "lang.set", "language": "eng", "key": "lng_my_track_desc", "value": "A private test circuit." }
  ]
}
```

Then list the file in the manifest:

```json [swofty.json]
"operations": {
  "assets": ["assets/text.json"]
}
```

If your key doesn't exist yet, it gets added. Point a `_loc_tag` field at it to show your text. If two mods set the same key, the one lower in the load order wins.

## Track names

To rename a track in menus, you need `TrackDataSet_<n>.TrackData_<m>.TrackNameData_0`: that's where menus read the name, rather than the track row. Set that field to your key — see [Rename a track](./database#_4-rename-a-track) — then give the key a string with `lang.set`.

## Delivery

You'll need the in-game loader for these edits. `lang` is a runtime-only kind: swofty_core adds the strings when the game loads a language file. Without the loader, `swofty build` lists the operations as waiting and changes no text. See [Loading mods at runtime](/reference/addons/runtime-assets#add-or-change-strings) and [deliveries](/reference/addons/asset-api#deliveries).
