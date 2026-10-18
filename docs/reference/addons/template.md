# The addon template

Copy [`templates/addon`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/addon) to start an addon. It uses the MIT licence.

## Get started

1. Copy the template to a new folder.
2. Edit `swofty.json`: set your addon's `id`, `name`, `author` and `version`.
3. Remove operation entries and files you do not need.
4. Install with `swofty addons install <dir>`.
5. Enable it with `swofty addons enable <id>`.

Use a lowercase ID such as `author.my-addon`. Do not use the reserved `swofty.` prefix.

## Included examples

| Path | Example |
|---|---|
| `swofty.json` | Toggle, choice and slider settings, one of them bound to the database |
| `db/example.json` | Change values, one of them from a setting |
| `ui/example.pkgpatch.json` | Change a menu label |
| `native/` | C++ plugin that reads its settings, and its build script |
| `README.md` | Build and test instructions |

See [Current status](/design/#current-status) for unfinished operation types.

## Share your addon

Share the folder or zip it as a `.swoftyaddon` file, with `swofty.json` at the zip's root. A `.swoftymod` file from an earlier version still installs.

Include your changes, not copies of the game's archives, database or executable.
