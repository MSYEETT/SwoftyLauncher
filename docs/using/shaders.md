---
title: Shaders
description: Pick a shader config, edit it with live validation, preview it on a short F1 clip, and keep your own configs next to the ones addons bring.
---

# Shaders

The **Shaders** page, under **Addon Browser** in the sidebar, chooses the look of the game's picture. A look is a **config**: a short text file that lists effects, such as bloom, colour grading or sharpening, and sets their options.

```toml
name = "My look"

[look]
brightness = 125
saturation = 110

[glare]
bloom = 35

[vignette]
amount = 0.3
```

Each `[table]` is an effect from the launcher's [effect library](/reference/addons/shader-effects), and the keys under it are that effect's options. Effects run top to bottom. One config is active per [profile](./profiles), or none.

## The page

```text
Shaders        Realistic · from Realistic Shader        In-game key [B]   Browse shaders
+------------------------------------------+-----------------------------+
| [ Editor | Preview ]                      |  Off                        |
|                                           |  Your configs               |
|  name = "Realistic"                       |    My look                  |
|  [auto_hdr]                               |    Night race               |
|  ...                                      |  From addons                |
|                                           |    Realistic   Realistic Shader |
|  Valid · live in game                     |    PolyphoniX Native  PolyphoniX |
+------------------------------------------+-----------------------------+
```

- The header names the active config and where it comes from, or **Off**.
- The list is on the right: **Off**, then **Your configs**, then **From addons**.
- The left side is the **Editor** or the **Preview**, switched at the top.
- **Browse shaders** opens the [Addon Browser](./addon-browser) on the Shaders category.

## Use a config

Select a row to open it on the left. Select **Use** on the row to make it the active config; select **Off** to run no config. The choice belongs to the current profile and applies the next time the game starts, and straight away while the game runs.

The **In-game key** turns every shader pass on and off while you play, with a toast. The default is `B`. Pressing it does not change the profile.

## The Editor

The Editor shows the config as text, with line numbers and colours. It checks the text as you type.

```toml
name = "Test"

[grane]            # error: Unknown effect `grane`; if it comes from an addon, that addon is not installed
amount = 0.4       #        quick fix: Change to `grain`

[vignette]
amoutn = 0.4       # error: `vignette` has no param `amoutn`   (quick fix: Change to `amount`)
radius = 3         # warning: `radius` is above its maximum of 1.5; it is clamped to 1.5
```

- **Problems** are listed in a strip under the editor. Select one to jump to it. Squiggles mark them in the text.
- **Quick fixes** apply from the problem. They fix a misspelt effect or option, add a missing `name`, and clamp a value.
- **Completions** open after `[` with effect ids, inside a table with that effect's options, and after `=` with an option's values.
- **Hover** over an effect for its description, and over an option for its label, what it does, its range and its default.
- The status pill reads **Valid · live in game**, **Valid**, or **2 problems · not applied**. A config with an error is not applied; the last valid version stays live.

A config of yours saves by itself whenever it is valid. When it is the active config and the game is running, every valid change reaches the game within a moment, so you can tune a value and watch the picture.

Any effect can be switched off without deleting it:

```toml
[vignette]
enabled = false
amount = 0.4
```

The [authoring reference](/reference/addons/shaders) lists every rule the checker enforces.

## Preview

The Preview plays a short clip from the game with the config applied.

- Play and pause, and a scrub bar with a frame counter.
- The clip picker switches between clips. The launcher downloads a clip the first time you pick it.
- **Compare** puts a vertical line over the picture. Drag it: the left side is the original, the right side has your config.

The Preview updates as you type. It runs the same effects as the game, so the picture is close to what you see in game, though not pixel for pixel.

## Your configs

Your configs are files in the launcher's data folder, under `shaders/`.

```text
New config      starts from a template with the effects commented out
Import          a .swfx file, or pasted text
Rename          changes the name and the file name
Duplicate       "My look (copy)"
Export          saves a .swfx file to share
Delete          removes the file; if it was active, shaders go Off
```

## Configs from addons

An addon can bring configs. They appear under **From addons** with the addon's name, and they are read-only.

- **Copy to personal** makes an editable copy under **Your configs**.
- An addon's config is never turned on by the addon. You choose it with **Use**.
- It stays in the list while the addon is switched off. Deleting the addon removes it, and if it was active, shaders go Off.
- An addon can also bring effects. They are available to your own configs while the addon is installed; a config that uses one shows an error naming it once the addon is gone.

The **Addons** page shows an addon that holds only shader configs with a **Shader config** chip and **Open in Shaders**, and no on/off switch. An addon that has other content too keeps its switch and says under it "Its shader config stays in Shaders while it's off".

### A forced config

An addon can force its config. While that addon is switched on, the page shows

```text
You have an addon which forces this shader configuration
```

with the addon's name. The list and **Use** are locked, and the forced config opens in the Editor read-only. Switch the addon off to choose a config again.

## Built-in configs

- **Realistic** from the Realistic Shader addon: flatter contrast, calmer reds and HDR-like highlights.
- **PolyphoniX Native** from the PolyphoniX Native addon: the Assetto Corsa look by Mark Wazowski.

## From the command line

```sh
swofty shaders list
swofty shaders use "addon:swofty.realistic-shader/realistic"
swofty shaders lint my-look.swfx
```

[`swofty shaders`](/reference/addons/shaders#the-swofty-shaders-command) has the full list.
