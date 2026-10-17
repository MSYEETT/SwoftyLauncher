# Profiles and restoring the game

Every change the launcher makes to the game folder is recorded before it happens and can be undone. Switching profiles, going back to vanilla, a crash and a game update all end with the folder byte-identical to Steam's copy.

## What a profile owns

A [profile](/using/profiles) owns:

- its mods, maps, load order and per-mod options;
- its own build cache, `$DATA/cache/overlay/<profile>/<buildhash>/`;
- its own save namespace. Saves are redirected to `$DATA/saves/<profile>/` and never touch vanilla saves or Steam Cloud.

## The vanilla profile

`vanilla` is a built-in profile that cannot be edited. Launching it places no `version.dll`, no overlay files and no prefix override. It is identical to Steam's own Play.

## The baseline

Before the first change to any game file, the launcher stores the original in a content-addressed backup store, `$DATA/baseline/objects/<sha256>`.

The per-build record is `$DATA/baseline/<build_id>/baseline.json`. It lists each file's path, size and sha256, and its source: `depot` when the hash was verified against Steam's depot manifest, otherwise `first-sight`.

## The journal

Every mutation is written to `$DATA/state/journal/<id>.json` before it happens and marked done after it. The journal covers:

- replaced game files;
- created files: `version.dll` and `swofty/mods/...`;
- created directories;
- the Wine prefix override in `user.reg`:

  ```
  [Software\\Wine\\AppDefaults\\F1_25.exe\\DllOverrides]
  "version"="native,builtin"
  ```

On every launcher start and before every launch, any open journal is rolled back first, in reverse order. A crash or a kill in the middle of applying leaves an open journal, and the next start rolls it back.

The file formats and the rollback rules are in [Baseline and journal](/reference/restore).

## Restoring

`swofty restore`:

1. rolls back every open journal;
2. removes every addition: `version.dll`, `swofty/mods` and the prefix override;
3. restores every baselined file;
4. verifies that the folder is byte-identical to the baseline.

It lists anything it could not restore and exits non-zero.

## Checking

`swofty verify` and `swofty doctor` report every drift from the baseline, whether the launcher caused it or not: changed files, additions present, the override present, open journals, and a build that differs from the baseline.

## Game updates

The launcher detects an update by the appmanifest `buildid` and the exe sha256. A new build gets a new baseline, taken from the updated files.

Originals from an old build are never restored over a new build. A file whose hash is neither the baseline's nor what the launcher wrote is left alone and reported.

## Steam's verify

Steam's "Verify integrity of game files" is a valid last resort. It restores the data files, but it does not remove `version.dll` or `swofty/`. `swofty restore` does.

## Delivery modes

Today, data files are delivered in journalled in-place mode: the profile's built files are copied into the game folder for the session, under the journal, and restored afterwards. The VFS mode, later, redirects the game's file opens to the overlay and writes no data files to the game folder at all.
