# Shader pack

A starting point for a shader pack for Swofty Launcher for F1 25: three colour looks for the race, picked with a setting. Copy this directory, rename it, and change `id` in `swofty.json`.

## Contents

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: the `shaders` category and experimental feature, a `look` choice, a `strength` slider and the `shader` block that names the look's `.cube` file |
| `looks/monochrome.cube` | Near-grey with a little contrast, 10% darker |
| `looks/overcast.cube` | Half the colour, lifted blacks and a cool cast |
| `looks/noir.cube` | Black and white with strong contrast |
| `looks/make_looks.py` | The script that wrote the three `.cube` files |

## Your own look

1. Take a screenshot of the game with the pack off.
2. Grade the screenshot in DaVinci Resolve, Photoshop or any tool that exports a 3D LUT.
3. Export the grade as a `.cube` file, 17 to 65 points per side, and put it in `looks/`.
4. Add its name to the `look` choice.

The look applies to the game's picture after its tonemap and before the HUD, in the same colour as the screenshot you graded.

## Install

```sh
swofty addons install .
swofty addons enable example.monochrome-looks
```

The build prints `shaders: Monochrome Looks grades the picture with looks/monochrome.cube (17x17x17) at 100% strength`. Changing the look or the strength rebuilds the profile at the next launch. One look applies at a time: with two shader packs on, the one later in the load order wins.

Looks are SDR. With HDR on in the game's display settings, the picture keeps the game's own colours.

Shader packs are documented under Modding, Shader packs, in the Swofty Launcher for F1 25 documentation.
