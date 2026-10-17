# `!pkg` resources

F1 25 uses `!pkg` files for menu screens, UI databases, routes and other resources. They are usually stored inside [ERP archives](/reference/erp).

For addon changes, see [UI patches](/reference/ops/ui-patches).

## Common types

| File | Purpose |
|---|---|
| `.ui` | Screens, labels, buttons and their events |
| `.uidb` | Values used by the menus, including input prompts |
| `.uir` | Routes that open and close screens |
| `.layer` | Objects placed in a track world, such as grid slots, garages and pit boxes |

Screens contain named nodes. References connect nodes to events, actions and layout entries.

## Tools

- `tools/pkg.py`: read and write resources.
- `tools/uinode.py`: work with screen nodes and their IDs.
- `packages/formats/src/pkg.rs`: Rust format support.

Writers must keep key order, value alignment and node references valid. Unchanged resources should write back with identical bytes.

Value tables are aligned by type: 16 bytes for `vec3`, `vec4`, `quat`, `woid`, `shnm` and `mat4`, 32 for `bbox`, 8 for 64-bit integers and 4 for the rest.
