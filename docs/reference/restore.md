# Backups and recovery

This page is for contributors. Players should use [Restoring the game](/using/profiles-and-restoring).

The **baseline** records original game files. The **journal** records changes so they can be undone after a crash.

## Stored files

| Path | Contents |
|---|---|
| `$DATA/baseline/objects/<sha256>` | Original file contents |
| `$DATA/baseline/<build_id>/baseline.json` | File sizes and hashes for a game build |
| `$DATA/state/journal/<id>.json` | Changes to undo |

## Recovery rules

Record each change before applying it. Undo changes in reverse order, including steps that may have been interrupted.

Restore replaced files from backups. Remove files and empty folders created by the launcher. Remove the Wine override added for the session.

If a file has unexpected contents, leave it alone and report it. Never restore files from an old game build over a newer one.

## Commands

- `swofty restore`: restore originals and remove launcher additions.
- `swofty verify`: report remaining changes.
- `swofty doctor`: report changes and check the rest of the setup.

## Tests

`packages/core/tests/restore.rs` covers profile changes, interrupted changes and game updates using a test folder.

```sh
cargo test -p swofty-core --test restore
```
