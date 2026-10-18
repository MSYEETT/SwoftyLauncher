# ERP archives

You'll find most of F1 25's artwork and track assets in `.erp` files under `2025_asset_groups/`. Inside each archive are named resources. Each resource has one or more parts, called fragments.

To change one of those assets in your addon, follow [ERP operations](/reference/ops/erp).

## Tools

- `packages/formats/src/erp.rs`: read and write archives in Rust.
- `tools/erp.py`: inspect and extract resources.
- `tools/erpw.py`: rebuild archives.

To see which resources an archive contains, list their names:

```sh
python3 tools/erp.py <archive> '*'
```

## Writing rules

When you rebuild an archive, keep unchanged fragments as stored. If you change a fragment, update its size, compression and resource hash. With no changes, you should get identical bytes back.

UI resources inside ERPs use the [`!pkg` format](/reference/pkg).

## Test

```sh
F1_25_DIR="/path/to/F1 25" cargo test -p swofty-formats
```

Set `F1_25_DIR` to have this check real game files.
