# Uninstalling

Steam's **Verify integrity of game files** repairs game files, but leaves the launcher's additions in place. You'll need `swofty restore` to remove those:

1. Close F1 25.
2. Run `swofty restore`. If it reports files it couldn't restore, resolve those before you continue. See [Restoring the game](/using/profiles-and-restoring).
3. On Linux, open F1 25 → **Properties** → **General** in Steam and clear F1 25's **Launch options**.
4. Copy out anything you want to keep, then delete the launcher folder and the [data folder](/guide/installation#where-your-files-are-stored). Deleting the data folder removes your installed addons, mods, profiles, backups and any saves stored there.
