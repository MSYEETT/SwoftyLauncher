# ERP archives

F1 25 stores most artwork and track assets in `.erp` files under `2025_asset_groups/`. Each archive contains named resources, split into one or more parts called fragments.

To change an asset in a mod, see [ERP operations](/modding/erp-ops).

## Tools

- `packages/formats/src/erp.rs`: Rust reader and writer.
- `tools/erp.py`: inspect and extract resources.
- `tools/erpw.py`: rebuild archives.

List resource names:

```sh
python3 tools/erp.py <archive> '*'
```

## Writing rules

Keep unchanged fragments as stored. For changed fragments, update their sizes, compression and resource hash. Rebuilding an unchanged archive should produce identical bytes.

UI resources inside ERPs use the [`!pkg` format](/reference/pkg).

## Test

```sh
F1_25_DIR="/path/to/F1 25" cargo test -p swofty-formats
```

This checks real game files when `F1_25_DIR` is set.
