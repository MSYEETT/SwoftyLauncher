# `!pkg` resources

F1 25 uses `!pkg` files for menu screens, UI databases, routes and other resources. They are usually stored inside [ERP archives](/reference/erp).

To change one from an addon, use the [`patch-pkg` ERP operation](/reference/ops/erp#editing-pkg-resources). [UI patches](/reference/ops/ui-patches) describe the planned per-node format, which the build skips for now.

## Common types

| File | Purpose |
|---|---|
| `.ui` | Screens, labels, buttons and their events |
| `.uidb` | Values used by the menus, including input prompts |
| `.uir` | Routes that open and close screens |
| `.layer` | Objects placed in a track world, such as grid slots, garages and pit boxes |
| `.idf` | [UI scenes](/reference/ui/idf): the layout of a menu screen |

Screens contain named nodes. References connect nodes to events, actions and layout entries.

## Tools

- `tools/pkg.py`: read and write resources.
- `tools/uinode.py`: work with screen nodes and their IDs.
- `packages/formats/src/pkg.rs`: Rust format support.

Writers must keep key order, value alignment and node references valid. Unchanged resources should write back with identical bytes.

Value tables are aligned by type: 16 bytes for `vec3`, `vec4`, `quat`, `woid`, `shnm` and `mat4`, 32 for `bbox`, 8 for 64-bit integers and 4 for the rest.

## Layout

The game's files hold the chunks breadth-first from the root, with no bytes between them, then the value tables, padded to 4 bytes at the end. Some mod tools edit a file in place: they leave a replaced chunk where it was, unreferenced, and append the new one after the last chunk, and they may leave the end unpadded. `pkg.rs` records that layout when it reads such a file and writes it back byte for byte while every chunk keeps its size. An edit that resizes a chunk writes the game's layout.
