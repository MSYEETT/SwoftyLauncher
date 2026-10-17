# Restoring the game

Close F1 25, then run:

```sh
swofty restore
```

This restores backed-up game files and removes files added by the launcher. It reports anything it could not restore.

## After a crash

Run `swofty restore` before returning to vanilla play. Use `swofty verify` to check for remaining changes, or `swofty doctor` to check your setup too.

## After a game update

The launcher does not restore files from an old game build over a newer one. If files still need repair, use Steam's **Verify integrity of game files**.

Steam repairs game files but does not remove `version.dll` or `swofty/`. Use `swofty restore` to remove those additions.

## Backups

Keep the [data folder](/guide/installation#where-your-files-are-stored) until restoration is complete. It contains the original files needed to undo changes.

Contributors can read [Backups and recovery](/reference/restore) for more detail.
