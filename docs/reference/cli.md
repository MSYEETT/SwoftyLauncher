# Command line

Use `swofty` from a terminal to manage addons, check files and read logs.

## `swofty run`

```sh
swofty run
```

Launches F1 25 and follows the logs. Steam must be running. Modded play is offline only.

When the game exits, it prints a summary with the exit code and any crash details. **Ctrl-C** stops following the logs; the game keeps running.

| Flag | Purpose |
|---|---|
| `--profile <name>` | Choose a profile; the default is `default` |
| `--game <dir>` | Set the game folder if it was not found |
| `--restart` | Close a running F1 25 first |
| `--direct` | Launch directly instead of asking Steam |
| `--winedebug <channels>` | Choose Wine log channels |
| `--exe <file>` | Use a copy of `F1_25.exe` to identify crash locations |
| `--apply` | Install the profile (database, maps, in-game loader) before the game starts and roll it back when it exits. The launcher always passes it |

On Linux, the [Steam launch option](/guide/linux) uses `swofty run -- %command%`.

A direct launch writes the wrapper's output to `$DATA/state/wrapper-run.log`. If the wrapper exits before the game starts or while it runs, `swofty run` prints the wrapper's last lines and its error, then exits with an error. It stops waiting after 10 minutes, or after 2 minutes when Steam launches the game.

## `swofty logs`

`swofty logs [<session dir>]` shows a finished session's summary. With no folder given, it uses the latest session.

## `swofty build`

`swofty build [--profile <name>]` prepares the profile's database and map files without starting the game. Output goes to `$DATA/cache/overlay/<profile>/<buildhash>/`, with the session settings file in `swofty/settings.json` and `plan.json`, the list of game files to replace and add with their sources. Map archives, menu archives and car handling files are built in `$DATA/cache/maps`, `$DATA/cache/menus` and `$DATA/cache/ers`.

It also builds addon ERP files that use only `patch-pkg`, `copy-resource`, `put-resource` and `warn`, and prints their warnings. Addon ERP operations apply on top of the archives the map stages built. `file.put` operations and `operations.files` entries replace or add loose game files. Other addon ERP and UI operations are currently skipped. `group` and `lang` operations wait for runtime support in `swofty_core`, and the build lists them. The build also lists each conflict: a target that a later addon replaced after an earlier one wrote it.

When the game exits, `swofty run` merges the settings changed in game into the profile. After **APPLY & RESTART** it rebuilds the profile and starts the game again.

## `swofty apply`

`swofty apply [--profile <name>] [--game <dir>]` builds the profile and installs it into the game folder: the database, the archives addons patch, the enabled maps, their menu entries, the cars' ERS settings, and the [in-game loader](/reference/addons/#in-game-loader) with the session's `swofty/settings.json`. Every change goes through the journal. `swofty restore` or `swofty --profile vanilla apply` removes it. Close the game first.

## `swofty verify`

`swofty verify [--game <dir>]` checks for changes from the backed-up originals and checks that game files can be read and written correctly.

## `swofty restore`

`swofty restore` restores backed-up files and removes launcher additions. It reports anything it cannot restore. Close the game first.

See [Restoring the game](/using/profiles-and-restoring).

## `swofty addons`

| Command | Purpose |
|---|---|
| `swofty addons list` | List installed addons and whether they are enabled |
| `swofty addons install <path>` | Install an addon folder, a `.swoftymod` file or a zip that holds an addon |
| `swofty addons enable <id>` | Enable an addon |
| `swofty addons disable <id>` | Disable an addon; SwoftyMod cannot be disabled |

`swofty mods` is another name for `swofty addons` while no addon provides a `mods` command. Once one does, `swofty mods install <addon folder>` and `swofty mods enable|disable <addon id>` still run the `swofty addons` command and print a note.

Built-in addons add their own top-level commands. `swofty --help` lists them.

Each command first moves addons that an earlier version stored in `$DATA/mods/` to `$DATA/addons/`.

## `swofty settings`

| Command | Purpose |
|---|---|
| `swofty settings list [<id>]` | List addon settings with the profile's values and any pending rebuild |
| `swofty settings set <id> <option> <value>` | Set a value: `true`, `5`, `replica` |
| `swofty settings reset <id>` | Put an addon's settings back to their defaults |
| `swofty settings export [--out <file>]` | Write the session settings file that the game reads |
| `swofty settings merge` | Merge the changes made in game into the profile |

## `swofty maps`

| Command | Purpose |
|---|---|
| `swofty maps list` | List imported tracks |
| `swofty maps import <zip>` | Import a community track package |
| `swofty maps remove <code>` | Remove an imported track |
| `swofty maps enable <code>` | Enable an imported track in the profile |
| `swofty maps disable <code>` | Disable a track in the profile |

## `swofty doctor`

`swofty doctor` checks Steam, the game install, Proton, launch options and the data folder. It also reports changes from the backed-up originals.
