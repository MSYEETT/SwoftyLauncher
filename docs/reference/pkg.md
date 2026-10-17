# `!pkg` resources

F1 25 uses `!pkg` files for menu screens, UI databases, routes and other resources. They are usually stored inside [ERP archives](/reference/erp).

For mod changes, see [UI patches](/modding/ui-patches).

## Common types

| File | Purpose |
|---|---|
| `.ui` | Screens, labels, buttons and their events |
| `.uidb` | Values used by the menus, including input prompts |
| `.uir` | Routes that open and close screens |

Screens contain named nodes. References connect nodes to events, actions and layout entries.

## Tools

- `tools/pkg.py`: read and write resources.
- `tools/uinode.py`: work with screen nodes and their IDs.
- `packages/formats/src/pkg.rs`: Rust format support.

Writers must keep key order, value alignment and node references valid. Unchanged resources should write back with identical bytes.
