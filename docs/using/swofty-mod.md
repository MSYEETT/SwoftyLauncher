# In-game mod settings

SwoftyMod is the built-in mod. It adds core functionality for Swofty Mod loading and customisation, including **MOD SETTINGS** in the home menu, directly under **Career**. It is always enabled in modded profiles.

The home screen also has an **F9** keyboard prompt (**R3** on a controller) and a panel showing that you launched with F1 Mod Manager.

## The settings page

MOD SETTINGS has one section per installed mod, built for each session from the mods' [options](/modding/manifest#options) and the profile's values.

- **ENABLED** turns the mod on or off from the next launch. SwoftyMod shows **ALWAYS ON**.
- A toggle switches with **Select**, **Left** or **Right**.
- A choice moves to the next value with **Select** or **Right**, and back with **Left**.
- A slider steps with **Left** and **Right**. **Select** steps up and wraps to the lowest value.

Changes save to the profile, and the launcher shows them. Settings marked as applying live reach the mod at once. Database settings and **ENABLED** need a restart: **APPLY & RESTART** appears at the top of the page, and the game restarts with the profile rebuilt.

The same settings are in the launcher, in each mod's row on the **Mods** page.

## Current status

The settings page and the native bridge that saves in-game changes are tested offline. Whether the page's commands reach the bridge in the real game is still being tested.

If the game build is not recognised, the menu entry may be unavailable.
