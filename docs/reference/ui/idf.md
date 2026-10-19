---
title: UI scenes (.idf)
description: The scene layout behind every F1 25 menu screen — a !pkg holding an eScene graph — how a .ui screen attaches to it, where scenes live in the UI archives, and what a scene does and doesn't store.
---

# UI scenes (`.idf`)

A UI scene is the layout of a menu screen: the named rectangles a screen fills, with their positions, sizes and nesting. Each scene is an `.idf` resource holding a [`!pkg`](/reference/pkg) container, and the container holds an `eScene` graph of elements. A `.ui` screen attaches its widgets to the scene's elements by name.

[Swofty UI](./swofty-ui) opens, renders and edits scenes. [Editing menus with an addon](./editing-menus) takes an edited scene into the game.

Cars and tracks also name their model resources `.idf`. This page covers the menu scenes in `ui_package`.

## Where scenes live

Scenes ship in the UI archives: the `.erp` files in `2025_asset_groups/ui_package/` and its subfolders, such as `screens/`. The scene itself is the `UILayout` resource named exactly `<path>.idf`, with no `?context=`. It has one fragment, the `!pkg`.

The archive also holds the resources the scene's elements point at, including:

| Resource | Type | Contents |
|---|---|---|
| `eaid://ui_package/art/screens/career/rnd_overview.idf` | `UILayout` | The scene |
| `….idf/<element>?context=render&lod=0&mesh` | `GfxMeshRes` | An element's mesh: vertex and index buffer names and a material slot |
| `….idf/<element>?context=render&lod=0&vb=0`, `&ib=0` | `GfxBufferRes` | The mesh's vertex and index buffers |
| `….idf/<element>?context=animation` | `AnimClip` | An element's animation clip |
| `….idf?context=animstringtable&file=0` | `ACStringTable` | Names the clips use |
| `art/screens/career/rnd_overview.emb?context=default` | `GfxMatBindRes` | The scene's material bindings: mesh slot to `.material` |

Textures (`.tif`, `GfxSurfaceRes`) and materials (`.material`, `GfxMaterialRes`) are separate resources that the materials name.

The same scene is often in several archives: 2,161 of the 5,208 scene names appear in exactly two, and shared components such as tiles appear in dozens. List the archives that hold a scene with [Swofty UI](./swofty-ui), or with:

```sh
python3 tools/erp.py <archive> 'rnd_overview.idf' | grep UILayout
```

## The container

The `!pkg` root holds a `scenelist`. Every stock scene has exactly one entry:

```text
{ scenelist: [ { scene: <element>, name: <ui64>, nodeidlookup: [<ui32>, …] } ] }
```

| Field | Type | Value |
|---|---|---|
| `scene` | element | The root element, named `Scene Root`, of type `eScene` |
| `name` | `ui64` | The scene's `componentId` |
| `nodeidlookup` | `ui32[]` | The name index: the djb2 of every element's leaf name |

## Elements

Each element is an object with these keys:

| Key | Type | Value |
|---|---|---|
| `name` | `stri` | The leaf name a widget attaches to, such as `tile_4` |
| `elementId` | `ui32` | djb2 of the element's dotted path |
| `typeName` | `shnm` | The element class, such as `eNull` or `eXRef` |
| `position` | `vec3` | Local translation, relative to the parent |
| `scale` | `vec3` | Local scale |
| `rotation` | `quat` | Local rotation, `x y z w` |
| `params` | `[{ name, value }]` | Named parameters |
| `geoms` | `[{ resourceid, materialid, extentsmin, extentsmax, asset }]` | The element's art: a mesh resource, its material bindings and its bounds |
| `anims` | `[{ asset }]` | The element's animation clips |
| `children` | elements | Child elements |

An element's place on screen is its `position`, `rotation` and `scale` composed with those of every parent. Its size is the local box in its `minExtents` and `maxExtents` params, or the bounds of its geoms.

### Classes

| `typeName` | Element |
|---|---|
| `eScene` | The scene root |
| `eNull` | A group or transform, the most common class |
| `eXRef` | A reference to another scene: `fileRef` names the `.idf`, `XrefMaterialBinding` its `.emb` |
| `eText` | A text box: `style`, `boxWidth`, `halign`, `wrapping`, `textcase` |
| `eStacker` | A container that lays its children out in a line: `stackerType`, `gap`, `useStaticGap` |
| `eCentre` | A container that centres its children |
| `eScaler` | A scaling container: `scaleX`, `scaleY`, `scaleZ` |
| `eWindow` | A clipping rectangle |
| `ePoint`, `eAnchor` | Anchor points |
| `eMask`, `eMaskTexture` | Masks |
| `eScroller`, `eCanvas`, `eCamera`, `eBoundless` | Scroll regions, canvases, cameras and unclipped groups |

Common params are `className` (the class again), `guid` (a unique id per element), `minExtents` and `maxExtents`, `pivotRot`, `pivotTrans` and `scaleRot`, and `eLayer.layer` with `eLayer.layerIsOffset` for draw order.

## Identity

Three hashes tie a scene to the screens that use it. A tool that edits a scene keeps all three consistent.

**componentId.** The scene's `name` is the first 8 bytes, big-endian, of the MD5 of its `eaid://` path without the `.idf` suffix:

```text
md5("eaid://ui_package/art/screens/career/rnd_overview") = be419b0b8324e33c…
componentId                                              = 0xbe419b0b8324e33c
```

**elementId.** Each element's `elementId` is the djb2 of its dotted path. The path starts at the top-level child's name; `Scene Root` is not part of it. `Scene Root`'s own id is `djb2("Scene Root")`.

```text
screen_null.tut_scaler.transition_null_4.tile_4  ->  elementId 2861292184
```

**nodeidlookup.** The scene's name index is the sorted, de-duplicated list of `djb2(leaf name)` for every element, `Scene Root` included. It holds leaf-name hashes, not elementIds. A new element adds the hash of its name in sorted position.

djb2 runs over the name's UTF-8 bytes: start at `5381`, and for each byte set `h = (h * 33 + byte) mod 2^32`.

## How a screen attaches

A `.ui` screen (UIDF) places its nodes with layout attaches:

- The screen's root attaches to a whole scene with `{ componentId, isComponentId: true, debugName: "<path>.idf" }`. The game finds the scene whose `name` is that `componentId`.
- A widget attaches to an element with `{ nodeId: djb2(debugName), debugName }`. `debugName` is the element's leaf name, never a dotted path, and resolves through `nodeidlookup`. A list of several entries walks through referenced scenes, such as `[screen_container, screen_cc]`: the `screen_container` eXRef, then `screen_cc` inside the scene it references.

A widget therefore binds to an element by name. Two elements with the same leaf name are ambiguous to an attach, so a new element needs a name no other element in the scene uses, such as `tile_5`.

## What a scene stores

A scene stores layout: the element tree, names and ids, transforms, boxes, params such as text style and stacker gaps, and references to art, animation clips and other scenes.

It does not store:

- **Pixels.** Textures are separate resources. A geom names a mesh and a material binding, and the material names its textures.
- **Meshes.** A geom points at `GfxMeshRes` and `GfxBufferRes` resources by name. A wedge's arc, for example, is baked into its mesh.
- **Text.** An `eText` element is a box with a style. The string comes from the `.ui` node attached to it, at runtime.
- **Live data.** Values, lists, colours and visibility come from the screen's `.ui` bindings and the game's view models.

What the game shows can also differ from the stored layout:

- **Animations.** An element with `anims` plays clips whose channels are element names and whose keys are absolute local transforms. When a clip plays, it sets the element's position, rotation and scale, which replaces a stored edit to that element. Move an unanimated child instead.
- **Stackers.** An `eStacker` lays out its children at runtime by `gap`, so a child's stored position is replaced. A stacker places any child added to it.
- **Collapsed elements.** Some elements, such as bars, pie wedges and highlights, are stored at a scale near 0, and an animation or live data grows them.

## Tools

- [Swofty UI](./swofty-ui) browses, renders and edits scenes, and checks every export.
- `tools/idf.py` prints and edits scenes from the command line:

```sh
python3 tools/idf.py tree rnd_overview.idf          # tree with positions, sizes and ids
python3 tools/idf.py params rnd_overview.idf        # the same, with every param
python3 tools/idf.py find rnd_overview.idf tile_4   # one element in detail
python3 tools/idf.py scene-id rnd_overview.idf eaid://ui_package/art/screens/career/rnd_overview.idf
python3 tools/idf.py roundtrip *.idf                # parse and rewrite, compare bytes
python3 tools/idf.py clone-slot rnd_overview.idf tile_4 tile_5 0 -2.52 0 out.idf
```

`tools/pkg.py` and `packages/formats/src/pkg.rs` read and write the container. All 5,208 scenes in the UI archives round-trip byte for byte through `tools/idf.py`, with every `elementId`, `nodeidlookup` and `componentId` matching the rules above.
