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

A session puts the game folder back when the game exits, whether it quits, crashes or is stopped. If the launcher itself was stopped before it finished, the next launch, `swofty run`, `swofty apply` or launcher start puts the folder back first and says **Recovered the game folder from an unfinished session**. Use `swofty verify` to check for remaining changes, or `swofty doctor` if you want to check your whole setup.

## Files in the way

When a file the profile needs to place is already in the game folder and the launcher can't prove it put it there (for example, a mod you copied in by hand), it leaves the file alone and doesn't start the game. In the launcher, select **Restore game folder**. From the command line, run `swofty restore`. Either one rolls back the launcher's changes and moves those files to `$DATA/set-aside/<time>/`, where you can get them back.

## After a game update

The launcher won't put files from an old game build back over a newer one. If your game files still need repair:

1. In Steam, open F1 25 → **Properties** → **Installed Files** → **Verify integrity of game files**.
2. Run `swofty restore` again to remove the launcher's additions (`version.dll` and `swofty/`). Steam repairs game files but leaves those additions in place.

## Backups

Your [data folder](/guide/installation#where-your-files-are-stored) holds the original files needed to undo changes, so keep it until restoration is complete.

If you're contributing, see [Backups and recovery](/reference/restore) for how restoration works internally.
