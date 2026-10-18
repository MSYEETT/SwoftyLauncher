# Restoring the game

To get every game file back to stock, use restore. It puts back the backed-up files and removes what the launcher added.

## Restore

1. Close F1 25.
2. Run:

   ```sh
   swofty restore
   ```

If anything couldn't be restored, you'll see it in the report. Resolve those issues before you play vanilla.

## After a crash

Before you return to vanilla play after a crash, run `swofty restore`. Then use `swofty verify` to check for remaining changes, or `swofty doctor` if you want to check your whole setup.

## After a game update

The launcher won't put files from an old game build back over a newer one. If your game files still need repair:

1. In Steam, open F1 25 → **Properties** → **Installed Files** → **Verify integrity of game files**.
2. Run `swofty restore` again to remove the launcher's additions (`version.dll` and `swofty/`). Steam repairs game files but leaves those additions in place.

## Backups

Your [data folder](/guide/installation#where-your-files-are-stored) holds the original files needed to undo changes, so keep it until restoration is complete.

If you're contributing, see [Backups and recovery](/reference/restore) for how restoration works internally.
