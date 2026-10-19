# Stock R&D card art: style notes

Measured from the stock 2026 (`rnd_om_xm`) arrays in a verified-stock F1 25 install. `src/extract_stock.py` decodes the frames the preview compares against into `stock/` (not committed).

## Where the art lives

| list (`overrides.uidb`) | slice | format | slices | archives |
|---|---|---|---|---|
| `overrides.career.rnd_om_xm.rnd_part_image` | 240×240 | BC7 (70), 1 mip | 337 | `common_flow_misc.erp`, `flow_team_hq.erp` |
| `overrides.career.rnd_om_xm.rnd_part_mask` | 240×240 | BC7, 1 mip | 110 | `flow_r_and_d.erp`, `flow_team_hq.erp` |
| `overrides.career.rnd_part_image` (pre-2026 lists) | 240×240 | BC7, 1 mip | 362 | `common_flow_misc.erp`, `flow_team_hq.erp` |
| `overrides.career.rnd_part_mask` | 240×240 | BC7, 1 mip | 120 | `flow_r_and_d.erp`, `flow_team_hq.erp` |
| `overrides.career.rnd_om_xm.rnd_category_icons` | 32×32 | BC7, 1 mip | 20 | `flow_career_hub.erp`, `flow_team_hq.erp` |
| `overrides.career.rnd_om_xm.rnd_card_bg` | 260×348 | BC7, 1 mip | 6 | `flow_r_and_d.erp` |

A frame is the slice at its entry's index (`HOOKS.md`, v0.8.0).

## Four frames per part

| frame | state | content |
|---|---|---|
| `om_xm_<part>` | available / locked | hidden-line wireframe of the part's mesh. RGB is a flat 128 grey; alpha carries the line (1 px, anti-aliased, silhouettes up to 2 px at alpha 255) |
| `om_xm_<part>_progress` | in progress | the same wireframe with RGB 255. Alpha is identical to the available frame |
| `om_xm_<part>_complete` | fitted | full-colour render of the same model from the same camera, transparent background, no ground shadow |
| `om_xm_<part>_m` | mask | white; alpha is the silhouette of the complete render grown 1–3 px and feathered (5–7 px at most) |

The pre-2026 lists use `<part>`, `<part>_in_progress`, `<part>_complete` and an unsuffixed mask `<part>`, with the same look.

## Look of the renders

- 3D renders of game-quality car-part meshes, not illustrations. The wireframes are the same meshes' edges: mostly quads, revolved parts at 48–72 segments, dense areas (drilled edges, fins) read as solid hatching.
- Framing: the part is centred and fills 65–95 % of the 240 px square (bounding boxes 155–228 px), with 5–40 px of empty margin. Groups of parts (a set of rims, two calipers) are composed into one cluster.
- Camera: three-quarter view from above and to the front-left, about 30–40° elevation, mild perspective.
- Light: dark studio. A large soft key from the upper left, a hard rim from behind on the right, little fill; shadows fall to near black. Metal shows bright edge highlights against a dark body.
- Palette: near-black carbon and rubber (median luminance 30–45), machined aluminium and titanium mid greys, small saturated accents (red anodising, Pirelli compound stripes, gold foil, copper windings). No coloured backgrounds, no tint.
- Texture: carbon weave, cast and machined metal, rubber grain, visible at 240 px only as fine noise.

## Card layout (`art/components/team_hq/research/part_card.idf`)

The card is 2.6×3.48 units (260×348 px textures). The part image (`2x6_rnd_content_image`) is drawn 2.2×2.2 units, centred 0.1 above the card centre. Title (`26_bold_drp_shw`, upper case, 2.3 wide) and the category row (0.24 icon + `14_regular` label) sit bottom left; the state icon stack sits top right. `rnd_card_bg` holds greyscale backgrounds per state (`part_available`, `part_in_progress`, `part_complete`, …).

## Category icons

32×32 white glyphs, about 1 px padding, two-tone: main shape alpha 255, secondary shape alpha 128. Frames are named `<department>_<type>` (`chassis_brakes`, `chassis_tyre_wear`).

## How ours are made

- `src/blender/` models every part procedurally in Blender 5.2 (bmesh lathes, lofts and boxes, so the wireframes are clean quads) and renders two passes per part at 480×480: a Cycles beauty pass (AgX, 128 samples, denoised) and a Freestyle hidden-line pass of every mesh edge over holdout materials.
- `src/post.py` downsamples both passes to 240×240 and writes the four frames: wire coverage as alpha with RGB 128 and RGB 255, the beauty pass as the complete frame, and the mask from the beauty silhouette.
- `src/icons/*.svg` are the category icons, rasterised by `src/icons.py` with `resvg` and forced to white.
- `src/build.sh [part …]` rebuilds everything (`out/`, `manifest.json`, `preview.html`).
