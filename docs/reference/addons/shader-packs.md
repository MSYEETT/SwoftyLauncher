---
title: Shader packs
description: Grade the race with your own colour look - a .cube LUT in an addon, with settings to pick the look and its strength.
---

# Shader packs

A shader pack is an addon that grades the game's picture with a colour look. The look is a `.cube` 3D LUT, the format DaVinci Resolve, Photoshop and most grading tools export. The in-game core applies it to the 3D picture after the game's tonemap. The HUD and menus draw on top of the graded picture and keep their own colours.

Start from [`templates/shader-pack`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/shader-pack): three looks and settings to pick one.

| | |
|---|---|
| Manifest | `shader`, `"category": "shaders"`, `"experimental_features": ["shaders"]` |
| Feature | `shaders` — experimental, on while an addon lists it in `experimental_features` |
| Launcher | 1.3.8 or later; an earlier launcher rejects the manifest |
| Picture | SDR. With HDR on, the game keeps its own colours |

## Make a look

1. Turn every shader pack off and take a screenshot of the game in SDR.
2. Grade the screenshot in DaVinci Resolve, Photoshop, Affinity Photo or any tool that exports a 3D LUT.
3. Export the grade as a `.cube` file. 17, 33 and 65 points per side all work.
4. Put the file in your addon, such as `looks/my-look.cube`.

The core applies the look to the same sRGB colours a screenshot holds, so the race looks the way your graded screenshot does.

## The manifest

```json [swofty.json]
{
  "schema_version": 1,
  "id": "me.my-looks",
  "name": "My Looks",
  "version": "1.0.0",
  "author": "Me",
  "category": "shaders",
  "experimental_features": ["shaders"],
  "compat": { "game": "f1_25" },
  "shader": { "lut": "looks/my-look.cube" }
}
```

| Field | Type | Value |
|---|---|---|
| `shader.lut` | string | The `.cube` file, a path inside the addon. <span v-pre>`{{id}}`</span> is replaced with a `db` option's value. |
| `shader.strength` | number or string? | How much of the look applies, from 0 to 100, or <span v-pre>`"{{id}}"`</span> naming a `db` slider that stays within 0 to 100. Defaults to 100. |

`shader` needs `"shaders"` in `experimental_features`. Installing checks the block: a `lut` that is not a `.cube` file inside the addon, an unknown option, an option that binds `native`, or a strength outside 0 to 100 is refused.

## Settings

To let players pick a look and its strength, declare `db` [options](./manifest#options) and name them in `shader`:

```json [swofty.json]
"options": [
  { "id": "look", "type": "choice", "label": "Look", "default": "monochrome",
    "values": [{ "value": "monochrome", "label": "Monochrome" }, { "value": "noir", "label": "Noir" }],
    "binds": "db" },
  { "id": "strength", "type": "slider", "label": "Strength", "min": 0, "max": 100, "step": 10,
    "default": 100, "binds": "db" }
],
"shader": { "lut": "looks/{{look}}.cube", "strength": "{{strength}}" }
```

The settings appear on the launcher's **Addons** page and in the game's **ADDON SETTINGS**. The build reads them, so a change applies from the next launch. Strength 0 shows the game's own colours, 100 shows the look in full, and values between mix the two.

## The .cube file

| Line | Value |
|---|---|
| `TITLE "…"` | Optional. |
| `LUT_3D_SIZE n` | Required. `n` is 2 to 129. |
| `DOMAIN_MIN r g b`, `DOMAIN_MAX r g b` | Optional. Default `0 0 0` and `1 1 1`. `LUT_3D_INPUT_RANGE min max` sets both. |
| `r g b` | `n`³ colour lines, red changing fastest, then green, then blue. |

`#` starts a comment, and other keyword lines are skipped. A 1D LUT (`LUT_1D_SIZE`) is refused. The build checks the file and stops at the first problem, with the line it is on:

```text
My Looks: looks/noir.cube: line 9: "0 0 x" is not 3 number(s)
```

## What players see

- The build prints the look: `shaders: My Looks grades the picture with looks/my-look.cube (33x33x33) at 100% strength`.
- The **Addons** page lists the pack under **Shaders**, tags it **Uses experimental features** and shows its `lut` under **Look** in its panel.
- One look applies at a time. With two shader packs on, the one later in the load order applies, and the build warns about the other: `My Looks: its look is not applied; one shader look applies at a time, and Other Looks loads after it`.
- With HDR on in the game's display settings, the picture keeps the game's own colours. The look applies again when HDR is off.
- `swofty/loader.log` in the game folder shows `[shaders] grading the picture with My Looks's look (33x33x33) at 100%`, then `[shaders] the picture now goes through My Looks's look` once the race renders.

## How it works

The game maps the colour of the 3D picture to display colour through a 64×64×64 LUT, `eaid://ego_package/aces_rrt_odt_lut.image`, and rewrites it every frame. The build checks your `.cube` file and installs it as `swofty/shaders/look.lut`, and `swofty/assets.json` names it under `shader`. In game, the core reads the game's LUT back, runs each entry through your look and copies the result over the game's LUT every frame. Every two seconds it reads the game's LUT again, and grades it again when the game changed it, as after a brightness change.
