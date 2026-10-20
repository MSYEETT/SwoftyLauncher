---
title: Updating
description: Update Swofty Launcher for F1 25 from the update bar or the command line, and keep Simplified for Swofty, addons and mods up to date.
---

# Updating

The launcher keeps itself and Simplified for Swofty, the addon behind the **Mods** page, up to date. You update other addons and mods by installing the new version over the old one.

## Update the launcher

The launcher checks for a new launcher release and a new Simplified for Swofty release when it opens, then every minute while it's open. When either is available, **Play** becomes **Update**. The button names what will update, and the line next to it shows the versions and a link to the release notes.

1. Close F1 25. While the game runs, you'll keep the **Stop** button; **Update** appears once the game exits.
2. Select **Update**. It installs Simplified for Swofty first, then the launcher, and shows the progress on the button. Each download is checked against its release's `SHA256SUMS`. The launcher update replaces `bin/`, `loader/`, `addons/` and, on Linux, `share/`. A newer version of an [addon that ships with the launcher](/using/bundled-addons) replaces the installed one the next time the launcher starts. The same goes for the Career Mode Editor and Driver Manager when you have an older version installed; the launcher doesn't install either one for you.
3. The launcher restarts on its own when the update finishes, including when only Simplified for Swofty updated.

If a check fails because you're offline, GitHub is down or GitHub's rate limit is reached, you can still play. The launcher checks again a minute later, or when GitHub's rate limit resets.

If an update fails, you'll see the error next to **Retry update** and the old files stay in place. Select **Retry update**, or **Play without updating** to play the version you have. An update interrupted partway is rolled back the next time you start the launcher.

Running the [installer](/guide/installation#install-with-the-installer) again on the same folder also installs the latest release.

If you built from source or installed through Nix, the launcher can't replace its own files. A bar above **Play** names the new version and links to its release page, and **Play** stays available.

### From a terminal

| Command | Does |
|---|---|
| `swofty update` | Install the latest release into this copy of the launcher |
| `swofty update --check` | Only report whether a new release is available |

Restart the launcher after `swofty update`. In a copy built from source or installed through Nix, `swofty update` prints the release page to download from. `swofty update` doesn't update Simplified for Swofty; the launcher does.

## Simplified for Swofty

The launcher keeps [Simplified for Swofty](/using/addons#built-in-addons) up to date through the same **Update** button. If the addon isn't installed, the launcher installs it in the background. Your mod library and each profile's mod order stay as they are.

## Addons and mods

| To update | Do |
|---|---|
| An addon | [Install](/using/addons#install-an-addon) the new version. It replaces the addon with the same id and keeps its switch and settings in every profile. The Career Mode Editor and Driver Manager also update with the launcher. |
| A mod or track | [Import](/using/mods#import-a-mod-or-track) the new version. It keeps its place and switch in every profile. |

## Next

- [Release notes](/releases/1.0.7): what changed in the latest release.
- [Restoring the game](/using/profiles-and-restoring): after a game update.
