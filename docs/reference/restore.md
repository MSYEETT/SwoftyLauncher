---
title: Backups and recovery
description: For contributors — how the baseline, the journal and session locks let the launcher undo every change to the game folder.
---

# Backups and recovery

This page is for contributors. Players should use [Restoring the game](/using/profiles-and-restoring).

The **baseline** records original game files. The **journal** records changes so they can be undone after a crash.

## Stored files

| Path | Contents |
|---|---|
| `$DATA/baseline/objects/<sha256>` | Original file contents |
| `$DATA/baseline/<build_id>/baseline.json` | File sizes and hashes for a game build |
| `$DATA/state/journal/<id>.json` | Changes to undo; `"session": true` for a `swofty run` session |
| `$DATA/state/placed.json` | Every file the launcher wrote into the game folder, with its SHA-256 |
| `$DATA/state/session.lock` | Held by a running session until its game files are rolled back |
| `$DATA/state/blocked.json` | Files of unknown origin that stopped the last launch |
| `$DATA/state/recovered.json` | The last recovery, for the launcher to announce |

## Recovery rules

Record each change before applying it. Undo changes in reverse order, including steps that may have been interrupted.

Restore replaced files from backups. Remove files and empty folders created by the launcher. Remove the Wine override added for the session.

If a file has unexpected contents, leave it alone and report it. Never restore files from an old game build over a newer one.

## Sessions

The `swofty run` wrapper holds `session.lock` from before it installs the profile until the offline play staging and the profile are rolled back. It rolls back when the game exits for any reason, on an error, and on a stop signal. A direct `swofty run` waits for the wrapper to exit and rolls back anything the wrapper left.

A session journal still open while the lock is free belongs to a session that ended without rolling back. `swofty run`, `swofty apply` and the wrapper roll back every open journal before they start. The launcher rolls back session journals when it starts, and keeps a `swofty apply` install.

## Leftovers

A file in the way of an install counts as the launcher's own when `placed.json` records those exact bytes at that path, or when it is byte-identical to the file about to be placed and not a backed-up original. The install clears those files, logs the recovery and the `swofty doctor` report, and tries once more. Any other file stops the install untouched and is recorded in `blocked.json`; `swofty restore` moves it to `$DATA/set-aside/<time>/`.

At start, files that `placed.json` records and no open journal covers are cleared the same way.

## Damaged state files

State files are written to a temporary file, flushed to disk and renamed over the old one. Backups in `baseline/objects` and restored originals are flushed before they take their name.

A state file that is not JSON at all (empty, zero-filled or truncated) is moved to `<name>.corrupt-<unix time>` and never deleted. Valid JSON in a shape the launcher cannot read is left in place and reported as an error.

| File | When damaged |
|---|---|
| `baseline.json` | Started over in rebuild mode (`"rebuild"` in the file) |
| A journal | Skipped; `placed.json` and the baseline clear its leftovers |
| `placed.json` | Started over empty; leftovers are reported as files of unknown origin |
| A profile or `config.json` | Started over as if missing |

In rebuild mode a game file is recorded as an original only when its bytes are already a backed-up original in `baseline/objects`. Any other file stops the install untouched with "the launcher's record of your original game files is damaged; verify the game files in Steam, then press Play again". The next install records such files at first sight. A file that `placed.json` records as written by the launcher is never recorded as an original, and `swofty verify` reports it until the game files are verified.

`swofty doctor` lists the set-aside copies.

## Commands

- `swofty restore`: restore originals and remove launcher additions.
- `swofty verify`: report remaining changes.
- `swofty doctor`: report changes and check the rest of the setup.

## Tests

`packages/core/tests/restore.rs` covers profile changes, interrupted changes and game updates using a test folder. `packages/core/tests/session_recovery.rs` covers crashed and killed sessions, leftovers of lost journals and files copied in by hand. `packages/core/tests/damaged_state.rs` covers damaged state files.

```sh
cargo test -p swofty-core --test restore --test session_recovery --test damaged_state
```
