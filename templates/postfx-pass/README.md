# Post-process pass

A starting point for a post-process pass for Swofty Launcher for F1 25: a vignette with saturation, set with two sliders. Copy this directory, rename it, and change `id` in `swofty.json`.

## Contents

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: the `postfx` experimental feature, `vignette` and `saturation` sliders, and the `postfx` block that passes them to the shader as `option(0)` and `option(1)` |
| `fx/vignette.hlsl` | The pass: it darkens the corners and mixes the picture towards grey or stronger colour |

## Your own pass

1. Edit `effect` in `fx/vignette.hlsl`, or add another `.hlsl` file and a second entry under `postfx`.
2. Read the picture with `scene(uv)` or `scene_at(pixel)`, and work in linear light with `to_linear` and `from_linear`.
3. Pass settings through `options` and read them with `option(i)`.

The header `swofty/postfx.hlsli` lists every input. `swofty postfx compile .` compiles the passes the way the build does and prints dxc's errors.

## Install

The launcher compiles the pass when it builds the profile, with the dxc in its `dxc/` folder.

```sh
swofty addons install .
swofty addons enable example.vignette-pass
```
