---
title: Adding a livery
description: Add a special livery as an extra choice in the livery picker, with the RM Designs Scuderia AlphaTauri ORLEN livery as the worked example.
---

# Adding a livery

A special livery is an extra choice in the livery picker for one car. It is three changes: the car's model in a `wep` archive, a row in the special-livery table, and an unlock row that the launcher reports as owned.

The example is RM Designs' Scuderia AlphaTauri ORLEN livery for the 2026 Racing Bulls car. The stock Racing Bulls liveries stay selectable.

```
rm-alphatauri-livery/
├── swofty.json
├── db/
│   └── se_livery.json
└── erp/
    ├── livery.json
    └── rm_object_opaque.nefx2.sm51.erp
```

`erp/rm_object_opaque.nefx2.sm51.erp` is RM Designs' archive from their download, unmodified.

## 1. The manifest

```json [swofty.json]
{
  "schema_version": 1,
  "id": "rmdesigns.alphatauri-orlen-livery",
  "name": "Scuderia AlphaTauri ORLEN livery",
  "version": "1.0.0",
  "author": "RM Designs",
  "kind": ["database", "erp"],
  "options": [
    {
      "id": "picker_name",
      "type": "choice",
      "label": "Name in the livery picker",
      "binds": "db",
      "default": "lng_team_tor_hashtag",
      "values": [
        { "value": "lng_team_tor_hashtag", "label": "AlphaTauri" },
        { "value": "Scuderia AlphaTauri ORLEN (RM Designs)", "label": "Literal text, no lookup" }
      ]
    }
  ],
  "compat": { "game": "f1_25", "game_builds": [">=25251167"], "seasons": ["f1_2026"] },
  "operations": {
    "database": ["db/se_livery.json"],
    "erp": ["erp/livery.json"]
  }
}
```

`picker_name` chooses the text the picker shows. `lng_team_tor_hashtag` is a stock text tag, so it shows in every language.

## 2. The livery row

Each special livery is a row in `customisation.custom_se_liveries.se_liveries`. `wep_override` names the archive the car loads: `_barcelona_26` on `toro_rosso_2026` loads `teams/toro_rosso/wep/toro_rosso_barcelona_26.erp`, a stock archive that holds no car.

```json [db/se_livery.json]
{
  "schema_version": 1,
  "ops": [
    {
      "op": "add-row",
      "table": "customisation.custom_se_liveries.se_liveries",
      "from": "customisation.custom_se_liveries.se_liveries.se_livery[_metadata_=toro_rosso_miami_2026]",
      "set": {
        "_metadata_": "toro_rosso_rm_alphatauri_2026",
        "id": "22",
        "wep_override": "_barcelona_26",
        "name_loc_tag": "{{picker_name}}",
        "desc_loc_tag": "{{picker_name}}",
        "ui_priority": "22"
      }
    },
    {
      "op": "add-row",
      "table": "unlockable_content.unlockable_items.unlockable_item_list_2.folder_23",
      "from": "unlockable_content.unlockable_items.unlockable_item_list_2.folder_23.unlockable_se_livery[_metadata_=toro_rosso_miami_26]",
      "set": {
        "_metadata_": "toro_rosso_rm_alphatauri_2026",
        "customisation_item": "toro_rosso_rm_alphatauri_2026",
        "statId": "5280"
      }
    }
  ]
}
```

The first op copies the stock Miami 2026 livery row and gives it a new label, id and archive. The second copies Miami's unlock row and points it at the new livery. `statId` must not be used by another unlock row.

## 3. The car

`copy-resource` copies each of the car's resources from RM Designs' archive into the empty `wep` archive.

```json [erp/livery.json]
{
  "schema_version": 1,
  "ops": [
    {
      "op": "warn",
      "message": "Scuderia AlphaTauri ORLEN by RM Designs: Racing Bulls 2026 special livery in toro_rosso_barcelona_26"
    },
    {
      "op": "copy-resource",
      "source": "erp/rm_object_opaque.nefx2.sm51.erp",
      "erp": "2025_asset_groups/f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso_barcelona_26.erp",
      "resource": "eaid://f1_2026_vehicle_package/teams/rbswapc/idf/rbswapc.idf?lod=all&ib=0"
    }
  ]
}
```

The full file has one `copy-resource` per resource: the model's index and vertex buffers, its materials and textures, the two `cf_pattern_6` textures, and RM Designs' world and layers. List an archive's resources with `tools/erp.py` ([Textures and models](./erp-assets#look-inside-an-archive)).

## 4. Ownership

The livery picker lists a special livery only when the player's inventory marks its unlock as owned. The key is the djb2 hash of `eSELivery` followed by the unlock row's `_metadata_` label: `0xf01d03c7` for `toro_rosso_rm_alphatauri_2026`.

When the launcher builds a profile, it compares the built database with the stock one. Every `unlockable_se_livery` row an addon adds to a folder under `unlockable_content.unlockable_items.unlockable_item_list_<n>` is written to `owned_items` in `swofty/settings.json`:

```json
"owned_items": [
  { "key": "0xf01d03c7", "addon": "rmdesigns.alphatauri-orlen-livery", "what": "se_livery 22" }
]
```

SwoftyMod reports these keys as owned in game. The build prints one line per owned item. A key that a stock row also gives is not reported and the build warns. A key repeated by two rows is reported once.

## 5. Build and check

```sh
swofty addons install rm-alphatauri-livery
swofty addons enable rmdesigns.alphatauri-orlen-livery
swofty build
```

The build prints the `warn` message and `items: rmdesigns.alphatauri-orlen-livery unlock row toro_rosso_rm_alphatauri_2026 (se_livery 22) is reported as owned, key 0xf01d03c7`. In game, the livery is a choice in the Racing Bulls car's livery picker.

If the livery is missing from the picker while the stock Miami and Suzuka 2026 liveries show, the rows loaded and the ownership check hid it: check `owned_items` in the session's `swofty/settings.json`.
