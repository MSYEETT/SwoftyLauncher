---
title: Shader configs and effects
description: Write .swfx configs, bundle them in an addon, force one, and write your own effects with params, textures and passes.
---

# Shader configs and effects

Two things make the picture look different in 1.8:

- An **effect** is HLSL with a declared list of options. The launcher ships a [library of effects](./shader-effects), and an addon can add its own.
- A **config** is a `.swfx` text file that lists effect instances in order and sets their options. Players pick one on the [Shaders page](/using/shaders).

An addon bundles configs, effects or both.

## A config in full

```toml [realistic.swfx]
# Comments start with #.
name = "Realistic"                       # required
author = "Swofty"
description = "Flatter contrast, calmer reds and HDR-like highlights."
swfx = 1                                 # the format version; optional, only 1 exists

# Each [table] is an effect instance. They run in file order.
[auto_hdr]
shadow_tuning = 1.94                     # float: a number; an integer is fine
highlight_saturation = 1.5
auto_hdr_method = "highlights"           # enum: one of the effect's values
hdr_tonemap = true                       # bool

[ambient_light]
al_int = 0.96
al_lens = true
al_lens_int = 10

[color_mod]
chroma = 0.917
brightness = [-0.039, -0.033, -0.033]    # color3: r, g, b

# Switch an instance off without deleting it.
[perspective]
enabled = false
fov_angle = 120

# A second instance of the same effect takes a label.
[glare]
bloom = 35

[glare.wide]
bloom = 10
threshold = 60
```

Anything a table leaves out takes the effect's default. The [effect library](./shader-effects) lists every effect with its options and defaults.

## What the checker enforces

The launcher checks a config on every change in the editor, in `swofty shaders lint`, and when the profile builds. A config with an error is not applied; the last valid version stays live. Each message below is the checker's own wording.

Top-level keys:

```toml
name = "Mine"        # required: A config needs a `name = "..."` (quick fix: Add a name); an empty name is `name` must not be empty
author = "Me"        # a string
description = "..."  # a string
swfx = 1             # must be 1: `swfx` must be 1, the only config format version
colour = "x"         # warning: Unknown top-level key `colour`; it is ignored
```

Tables:

```toml
[grane]              # error: Unknown effect `grane`; if it comes from an addon, that addon is not installed
                     #        quick fix: Change to `grain`
[vignette]
amoutn = 0.4         # error: `vignette` has no param `amoutn`        (quick fix: Change to `amount`)
amount = "high"      # error: `amount` expects a number, not a string
radius = 9           # warning: `radius` is above its maximum of 1.5; it is clamped to 1.5
                     #          quick fix: Clamp to 1.5
enabled = 1          # error: `enabled` expects true or false, not a whole number

[vignette]           # error: `[vignette]` appears twice; give the second a label, like `[vignette.second]`
```

Value types follow the option's type:

```toml
al_int = 0.96                    # float: a number
al_adapt_base_black_lvl = 2      # int: a whole number
al_lens = true                   # bool
al_dirt_tex = "smudges"          # enum: a string; error: "smudge" is not a value of `al_dirt_tex`; use one of "smudges", "specks"
brightness = [0, 0, 0]           # float2, color3 and color4: an array of 2, 3 or 4 numbers
                                 # error: `brightness` expects 3 numbers, not 2
```

- A value outside the option's `min` and `max` is a warning, and the value is clamped.
- A label cannot repeat one of the effect's option names: `The label `bloom` is also a param of `glare``.
- A config with no enabled instance gets the info `This config changes nothing`.
- An effect from an addon that is not installed is the unknown-effect error. When two addons declare the same effect id, or an addon declares a library id, the first in load order wins and the other effect is ignored with a warning in the build output. Load order is the profile's addon order, then the remaining addons by id.

## Bundle a config in an addon

```json [swofty.json]
{
  "schema_version": 1,
  "id": "swofty.realistic-shader",
  "name": "Realistic Shader",
  "version": "1.0.0",
  "author": "Swofty",
  "category": "shaders",
  "compat": { "game": "f1_25", "launcher": ">=1.8.0" },
  "shaders": [
    { "config": "shaders/realistic.swfx" },
    { "config": "shaders/realistic-hdr.swfx", "force": false }
  ]
}
```

```text
swofty.realistic-shader/
  swofty.json
  shaders/realistic.swfx
  shaders/realistic-hdr.swfx
```

- `shaders` is a list of entries; `config` is a relative `.swfx` path inside the addon. Its file name without `.swfx` is the config's id: `addon:swofty.realistic-shader/realistic`. Two entries with the same file name are refused.
- `effects` is a list of relative `<id>.effect.json` paths inside the addon.
- The configs show on the Shaders page under **From addons** with the addon's name, read-only. A player can copy one to personal.
- The configs stay available while the addon is switched off. Deleting the addon removes them, and every profile that had one of them active goes back to no config.
- Set `compat.launcher` to `>=1.8.0`: an older launcher refuses the addon at install with `needs Swofty Launcher >=1.8.0`.
- An addon never turns its own config on. The player picks it.

### Forcing a config

```json [swofty.json]
"shaders": [
  { "config": "shaders/broadcast.swfx", "force": true }
]
```

While the addon is switched on, `force: true` makes that config the active one, locks the Shaders page and shows the banner "You have an addon which forces this shader configuration", with a **View addon** button. Use it for an addon whose other content only looks right with that config.

An addon may force one config; the manifest is refused with `only one config may be forced` otherwise. When several enabled addons force a config, the last one in load order wins, as later addons override earlier ones everywhere else in the launcher. A shader-only addon has no switch and counts as on whenever it is installed.

### Shader-only addons

An addon whose content is only `shaders` and `effects`, plus metadata, an icon and a category, is a shader-only addon. Any `options`, `operations`, `launcher`, `hud`, `hub_tab`, `career_setup`, `multiplayer`, `shader` or `postfx` entry makes it a mixed addon. It has no on/off switch on the **Addons** page, and shows a **Shader config** chip with **Open in Shaders**. `swofty addons list` shows it as `shaders`. An addon that has other content keeps its switch, and says under it "Its shader config stays in Shaders while it's off".

## An effect in full

An effect is a JSON file named `<id>.effect.json` beside its HLSL. This one tints the picture and adds an optional darkening towards a corner.

```json [effects/warm_tint.effect.json]
{
  "id": "warm_tint",
  "name": "Warm Tint",
  "description": "Pulls the picture towards a colour, optionally darker towards the bottom.",
  "params": [
    { "id": "amount", "type": "float", "default": 0.3, "min": 0, "max": 1, "step": 0.01,
      "label": "Amount", "doc": "How far the picture moves towards the tint colour." },
    { "id": "tint", "type": "color3", "default": [1.0, 0.85, 0.7], "min": 0, "max": 2,
      "label": "Tint", "doc": "The colour in linear light." },
    { "id": "mode", "type": "enum", "values": ["multiply", "add"], "default": "multiply",
      "label": "Mode", "doc": "Multiply darkens, add lifts." },
    { "id": "fade_bottom", "type": "bool", "default": false,
      "label": "Fade bottom", "doc": "Darkens the lower third." }
  ],
  "textures": [],
  "passes": [
    { "name": "apply", "shader": "warm_tint.hlsl", "entry": "apply" }
  ]
}
```

```hlsl [effects/warm_tint.hlsl]
#include "swofty/params.hlsli"

float4 apply(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 rgb = to_linear(c.rgb);
    float3 tinted = param_mode() == MODE_ADD ? rgb + param_tint() * 0.1 : rgb * param_tint();
    rgb = lerp(rgb, tinted, param_amount());
    if (param_fade_bottom())
        rgb *= 1 - 0.4 * smoothstep(0.66, 1.0, uv.y);
    return float4(from_linear(rgb), c.a);
}
```

Register it in the manifest. A config in the same addon, or any other addon, can use it:

```json [swofty.json]
"effects": ["effects/warm_tint.effect.json"],
"shaders": [{ "config": "shaders/warm.swfx" }]
```

```toml [shaders/warm.swfx]
name = "Warm"

[warm_tint]
amount = 0.4
tint = [1.0, 0.8, 0.6]
mode = "add"
```

An effect id is `[a-z][a-z0-9_]*` and unique. The library owns the unprefixed names, so name yours with something of your own.

## Params

```json
{ "id": "amount",  "type": "float",  "default": 0.3, "min": 0, "max": 1, "step": 0.01, "label": "Amount", "doc": "..." }
{ "id": "levels",  "type": "int",    "default": 4, "min": 1, "max": 8, "label": "Levels", "doc": "..." }
{ "id": "smooth",  "type": "bool",   "default": true, "label": "Smooth", "doc": "..." }
{ "id": "mode",    "type": "enum",   "values": ["off", "low", "high"], "default": "low", "label": "Mode", "doc": "..." }
{ "id": "offset",  "type": "float2", "default": [0, 0], "min": -1, "max": 1, "label": "Offset", "doc": "..." }
{ "id": "tint",    "type": "color3", "default": [1, 1, 1], "min": 0, "max": 2, "label": "Tint", "doc": "..." }
{ "id": "border",  "type": "color4", "default": [0, 0, 0, 1], "min": 0, "max": 1, "label": "Border", "doc": "..." }
```

- `id` is `[a-z][a-z0-9_]*` and unique in the effect. `label` and `doc` are what the editor shows on hover.
- `min` and `max` apply to every number of a vector. `step` is the editor's nudge size. A `default` outside `min` and `max` is refused.
- An int's `default`, `min` and `max` are whole numbers.
- An enum needs at least one value, and its default is one of them.

### Slots

An effect's params pack into 32 float slots, in declaration order. `float`, `int`, `bool` and `enum` take one slot, `float2` two, `color3` three and `color4` four. An effect that needs more than 32 is refused: ``effect `x` needs 33 param slots; the limit is 32``.

```text
amount    float   slot 0
levels    int     slot 1
mode      enum    slot 2
offset    float2  slots 3-4
tint      color3  slots 5-7
border    color4  slots 8-11        12 of 32 used
```

A pass reads slot `n` with `option(n)`. `option(0)` to `option(15)` come from the original options, and `option(16)` to `option(31)` from the four extra `float4`s that follow them in the constant buffer.

### The generated accessors

Before it compiles an effect, the build writes `swofty/params.hlsli` on the include path. For the params above it holds

```hlsl
#pragma once
#include "swofty/postfx.hlsli"

float param_amount() { return option(0); }
int param_levels() { return int(round(option(1))); }
uint param_mode() { return uint(round(option(2))); }
#define MODE_OFF 0
#define MODE_LOW 1
#define MODE_HIGH 2
float2 param_offset() { return float2(option(3), option(4)); }
float3 param_tint() { return float3(option(5), option(6), option(7)); }
float4 param_border() { return float4(option(8), option(9), option(10), option(11)); }
```

- Effect HLSL does `#include "swofty/params.hlsli"`. It pulls in `swofty/postfx.hlsli`.
- A bool accessor is `bool param_<id>() { return option(n) > 0.5; }`.
- An enum constant is `<ID>_<VALUE>` upper-cased with every other character as `_`: the value `"two-way"` of `mode` is `MODE_TWO_WAY`. Two constants with the same name are refused.

## Textures and passes

An effect's `textures` and `passes` are the [post-process passes](./postfx) contract with names local to the effect. Two instances of the same effect, such as `[glare]` and `[glare.wide]`, each get their own textures.

```json
"textures": [
  { "name": "half", "scale": 0.5, "format": "rgba16f" },
  { "name": "adapt", "width": 1, "height": 1, "format": "rgba16f", "persistent": true },
  { "name": "dirt", "file": "ambient_light_dirt.png" }
],
"passes": [
  { "name": "bright", "shader": "ambient_light.hlsl", "entry": "bright", "output": "half" },
  { "name": "down",   "shader": "ambient_light.hlsl", "entry": "down", "inputs": ["half"], "output": "half2" },
  { "name": "apply",  "shader": "ambient_light.hlsl", "entry": "apply", "inputs": ["half", "adapt", "dirt"] }
]
```

- A pass has `name`, `shader`, `entry`, `raw`, `threads`, `inputs`, `output`, `when` and `budget_ms`, as in [post-process passes](./postfx#passes).
- `inputs` names the effect's textures, and also `original` (the picture before any pass this frame) and `previous` (the last frame's picture after every pass). Read them with `input(i, uv)` by their place in the list.
- No `output` writes the picture. A pass that writes a texture is sized by it.
- A `persistent` texture keeps its contents from frame to frame. The ambient light effect stores its eye adaptation in one.
- A `file` texture is an image from the effect's folder, read-only.
- The pass order is the order in the file. Passes of the next instance start where this one ends.
- A pass that reads `previous` sees the picture after all the instances, so it lags one frame behind.

## The `swofty shaders` command

```sh
swofty shaders list                          # every config: ref, name, source; marks the active (or forced) one
swofty shaders effects                       # every effect available
swofty shaders effects ambient_light         # one effect with its params, slots and defaults
swofty shaders show personal:night-race      # a config's text; a config's name works in place of a ref
swofty shaders lint my-look.swfx             # check a file or a ref; exits non-zero on an error
swofty shaders use addon:swofty.realistic-shader/realistic
swofty shaders use off
swofty shaders new "Night race"              # from the starter template
swofty shaders copy addon:swofty.realistic-shader/realistic
swofty shaders rename personal:night-race "Night race 2"
swofty shaders delete personal:night-race
swofty shaders import my-look.swfx
swofty shaders export personal:night-race night-race.swfx
swofty shaders key F9                        # the in-game on/off key: A-Z, 0-9, F1-F24 or "None"
swofty shaders key                           # print the key
```

A ref is `personal:<slug>` for a file in the launcher's `shaders/` folder, or `addon:<addon id>/<file name without .swfx>`.

`swofty shaders lint` prints each diagnostic as `file:line:column: severity: message`, followed by a `fix:` line when there is a quick fix, and exits 1 when the config has an error. `swofty shaders use` also says when an addon forces a config over the one you chose.

## What the game logs

The core logs each time the active config changes. `serial` counts the changes:

```text
[shaders] config "Realistic" (serial 42): 5 effect(s), 11 pass(es)
[shaders] config "Realistic" (serial 43): 4 effect(s), 9 pass(es)
```

A parameter-only change updates the values and logs the new serial. The per-pass GPU time lines name the effect and its instance, like `ambient_light` or `glare.wide`. A pass whose average GPU time stays over its `budget_ms` is turned off, as in [post-process passes](./postfx#in-game).

A plain `postfx` pass in an addon's manifest still runs, after the config's effects.
