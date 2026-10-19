# Profiles

Use a profile to keep a whole setup together: your addons and their load order, your mods and tracks and their order, and every addon setting. When you want to swap setups, switch profiles. Each one keeps its own saves.

To play F1 25 exactly as shipped, choose `vanilla`, the built-in profile with nothing installed. You can't edit it. Every modded profile includes SwoftyMod.

## Create a profile

1. Open the **Profiles** page.
2. Select the **New profile** button in the header or the dashed **New profile** card.
3. Type a name using letters, digits, `-` and `_`.
4. Select **Create**.

Your new profile starts with SwoftyMod and nothing else. To fill out your setup, add content on the [Addons](/using/addons) and [Mods](/using/mods) pages.

## Switch profile

You can switch in either place:

- On the **Profiles** page, select **Switch to this** on the profile's card. You'll recognise the active card by **Active** and **Used when you press Play**.
- At the bottom of the sidebar, under **ACTIVE PROFILE**, select the profile picker and choose a profile.

When you press **Play**, the launcher uses your active profile.

## Rename or duplicate

To change a profile's name or make a copy of its setup, open the **Profiles** page and expand its card footer:

- The pencil icon renames it.
- The copy icon duplicates it into a new profile with the same content.

You won't see either icon on `vanilla`, because that profile cannot be changed.

## Delete a profile

1. Open the **Profiles** page.
2. In the profile's card footer, select the trash icon.
3. Read the confirmation, then select **Delete profile**.

Deleting a profile removes its choice of addons and mods, their settings, its build cache and its saves. Installed addons and mods stay in the library, and other profiles don't change. You can't undo it.

- **The active profile:** the launcher switches to another profile first and names it in the confirmation. It picks `default` if that profile exists, otherwise the first remaining profile in alphabetical order.
- **Your only profile:** you can't delete it, so the trash icon is unavailable. Create another profile first.
- **`vanilla`:** it's built in, so it has no trash icon.
- **While F1 25 runs:** close the game first. A profile that's still installed in the game folder after a crash or `swofty apply` can't be deleted until you [restore the game](/using/profiles-and-restoring).

From the command line, use `swofty profile delete <name>`. It asks before deleting; add `--yes` to skip the question. `swofty profile list` shows every profile and marks the active one.

## Restoring the game

Switch to `vanilla` when you want to play without any content. The game folder still keeps its backups until you restore it; to return every game file to stock, see [Restoring the game](/using/profiles-and-restoring).
