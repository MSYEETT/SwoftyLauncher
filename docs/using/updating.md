---
title: Updating
description: Update Swofty Launcher for F1 25 from the update bar or the command line, and keep Simplified for Swofty, addons and mods up to date.
---

# Updating

The launcher offers each new release itself. Simplified for Swofty, the addon behind the **Mods** page, updates on its own. You update other addons and mods by installing the new version over the old one.

## Update the launcher

When you open the launcher, it checks for a new release, at most once every 6 hours. If there's one available, you'll see its version in a bar above **Play**, with **Update**, **Release notes** and **Skip this version**.

1. Close F1 25. The launcher won't update while the game runs.
2. Select **Update**. It downloads the archive, checks it against the release's `SHA256SUMS`, and replaces `bin/`, `loader/`, `addons/` and, on Linux, `share/`. A newer version of an [addon that ships with the launcher](/using/bundled-addons) replaces the installed one the next time the launcher starts.
3. Select **Restart**.

If a step fails, you'll see the error and the old files stay in place. An update interrupted partway is rolled back the next time you start the launcher.

Running the [installer](/guide/installation#install-with-the-installer) again on the same folder also installs the latest release.

If you built from source or installed through Nix, you'll see **Download** instead; it opens the release page.

### From a terminal

| Command | Does |
|---|---|
| `swofty update` | Install the latest release into this copy of the launcher |
| `swofty update --check` | Only report whether a new release is available |

Restart the launcher after `swofty update`. In a copy built from source or installed through Nix, `swofty update` prints the release page to download from.

## Simplified for Swofty

The launcher keeps [Simplified for Swofty](/using/addons#built-in-addons) up to date. At most once every 6 hours, when it starts and while it runs, it looks for a newer Team Simplified release, checks the download against the release's `SHA256SUMS` and installs it. Your mod library and each profile's mod order stay as they are.

## Addons and mods

| To update | Do |
|---|---|
| An addon | [Install](/using/addons#install-an-addon) the new version. It replaces the addon with the same id and keeps its switch and settings in every profile. |
| A mod or track | [Import](/using/mods#import-a-mod-or-track) the new version. It keeps its place and switch in every profile. |

## Next

- [Release notes](/releases/1.0.7): what changed in the latest release.
- [Restoring the game](/using/profiles-and-restoring): after a game update.
