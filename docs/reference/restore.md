# Baseline and journal

The baseline and the journal are how the launcher returns the game folder to Steam's copy. The user-facing rules are in [Profiles and restoring the game](/using/profiles-and-restoring).

## Paths

| Path | Contents |
|---|---|
| `$DATA/baseline/objects/<sha256>` | Original file contents, content-addressed |
| `$DATA/baseline/<build_id>/baseline.json` | The baseline record for one game build |
| `$DATA/state/journal/<id>.json` | One journal per apply |

## `baseline.json`

```json
{
  "schema_version": 1,
  "build_id": "25251167",
  "exe_sha256": "d873a1c8…",
  "files": {
    "game_package/gamedatabases/gamedatabases.2025.bin": {
      "size": 19341077,
      "sha256": "…",
      "source": "depot"
    }
  }
}
```

| Field | Meaning |
|---|---|
| `build_id` | The Steam `buildid` from `appmanifest_3059520.acf` |
| `exe_sha256` | The sha256 of `F1_25.exe` |
| `files` | Keyed by path relative to the game folder, with `/` separators |
| `size`, `sha256` | The original file |
| `source` | `depot` when the hash was verified against Steam's depot manifest, otherwise `first-sight` |

A file enters the baseline before the first change to it, and its original bytes go into `objects/` at the same time.

## Journal

```json
{
  "schema_version": 1,
  "id": "20261017T120000Z-grid24",
  "profile": "grid24",
  "build_id": "25251167",
  "entries": [
    { "seq": 0, "op": "replace-file", "path": "game_package/gamedatabases/gamedatabases.2025.bin",
      "before": "…", "after": "…", "state": "done" },
    { "seq": 1, "op": "create-dir", "path": "swofty/mods/core", "state": "done" },
    { "seq": 2, "op": "create-file", "path": "version.dll", "after": "…", "state": "pending" },
    { "seq": 3, "op": "registry-override", "path": "user.reg", "state": "pending" }
  ],
  "committed": false
}
```

| Field | Meaning |
|---|---|
| `seq` | Order of application |
| `op` | `replace-file`, `create-file`, `create-dir` or `registry-override` |
| `path` | Relative to the game folder; for `registry-override`, the prefix's `user.reg` |
| `before` | sha256 of the content before the step. Absent for created files and directories |
| `after` | sha256 of the content the step writes |
| `state` | `pending` when the entry is written, before the step runs; `done` after it |
| `committed` | True once every entry is done |

The entry is written and flushed before the mutation, and marked `done` after it, so the journal on disk always covers every change that might have happened.

## Rollback

A journal is open until it is rolled back. Every open journal is rolled back on launcher start, before every launch, and by `swofty restore`. Entries are undone in reverse `seq` order, `pending` ones included, because a pending step may have run before the process stopped.

| Op | Rollback |
|---|---|
| `replace-file` | Hash is `before`: nothing to undo. Hash is `after`, or the entry is `pending`, and the build is the journal's: restore the original from `objects/<before>` and check its hash. Anything else: leave the file and report it |
| `create-file` | File absent: nothing to undo. Hash is `after`, or the entry is `pending`: delete the file. Anything else: leave the file and report it |
| `create-dir` | Remove the directory if it is empty; otherwise leave it |
| `registry-override` | Remove the `[Software\\Wine\\AppDefaults\\F1_25.exe\\DllOverrides]` stanza the launcher appended. The launcher adds the override only when the prefix has no such stanza, so the whole stanza is its own |

A journal whose steps are all undone is deleted.

## Game updates

An update is detected by the appmanifest `buildid` and the exe sha256. A new build gets a new baseline, taken from the updated files.

Originals from an old build are never restored over a new build. During rollback, a `replace-file` entry from another build is left alone and reported, even when the file still holds what the launcher wrote; Steam's "Verify integrity of game files" repairs it. During restore, only the current build's baseline is used.

`swofty restore` also removes additions that no journal recorded: `version.dll` when the baseline does not list it, the whole `swofty/` folder (its `*.log` files move to `$DATA/sessions/orphaned-logs/`), and the override stanza in `user.reg`.

## Tests

The integration tests in `packages/core/tests/restore.rs` run against a synthetic game folder. They prove that each of these leaves the folder byte-identical to the baseline:

- switching profiles A, then B, then vanilla;
- a kill between any two journal steps, followed by recovery;
- a game update in between.

```sh
cargo test -p swofty-core --test restore
```

## Commands

| Command | Uses |
|---|---|
| [`swofty restore`](/reference/cli#swofty-restore) | Rolls back, removes additions, restores baselined files, verifies |
| [`swofty verify`](/reference/cli#swofty-verify) | Reports drift from the baseline |
| [`swofty doctor`](/reference/cli#swofty-doctor) | Reports drift, along with the rest of the install |
