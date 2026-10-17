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
| `--profile <name>` | Choose a profile; the default is the active profile |
| `--game <dir>` | Set the game folder if it was not found |
| `--restart` | Close a running F1 25 first |
| `--direct` | Launch directly instead of asking Steam |
| `--winedebug <channels>` | Choose Wine log channels |
| `--exe <file>` | Use a copy of `F1_25.exe` to identify crash locations |
| `--apply` | Install the profile (database, tracks, in-game loader) before the game starts and roll it back when it exits. The launcher always passes it |

On Linux, the [Steam launch option](/guide/linux) uses `swofty run -- %command%`.

A direct launch writes the wrapper's output to `$DATA/state/wrapper-run.log`. If the wrapper exits before the game starts or while it runs, `swofty run` prints the wrapper's last lines and its error, then exits with an error. It stops waiting after 10 minutes, or after 2 minutes when Steam launches the game.

## `swofty logs`

`swofty logs [<session dir>]` shows a finished session's summary. With no folder given, it uses the latest session.

## `swofty build`

`swofty build [--profile <name>]` prepares the profile's database and track files without starting the game. Output goes to `$DATA/cache/overlay/<profile>/<buildhash>/`, with the session settings file in `swofty/settings.json` and `plan.json`, the list of game files to replace and add with their sources. Track archives, menu archives and car handling files are built in `$DATA/cache/maps`, `$DATA/cache/menus` and `$DATA/cache/ers`. After a successful build, older builds in those folders and in the profile's overlay folder are removed, except the ones the latest build of another profile uses.

It also builds addon ERP files and prints their warnings, except files with a `patch-pkg` edit other than `clone-entry` and `set-value`, which are skipped. Addon ERP operations apply on top of the archives the track stages built. `add-mipmaps` files replace or add loose game files. A build with a `remove-resource` rewrites the archives even when `swofty_core` lists `erp` as `stable`. `file.put` operations and `operations.files` entries replace or add loose game files. Other addon ERP and UI operations are currently skipped. Each asset kind that `swofty_core` lists as `stable` in `swofty/addons/core/features.json`, or as `experimental` and named in `config.json` `"runtime_assets"`, goes into `swofty/assets.json` instead: asset groups, changed ERP resources, database records that differ from the game file, one language table per language and overlay files. `group` and `lang` operations have no other delivery; without the feature they wait, and the build lists them. `config.json` `"build_assets"` keeps the `erp`, `db` or `file` kinds it lists on the build route. The build also lists each conflict: a target that a later addon replaced after an earlier one wrote it.

When the game exits, `swofty run` merges the settings changed in game into the profile. After **APPLY & RESTART** it rebuilds the profile and starts the game again.

## `swofty apply`

`swofty apply [--profile <name>] [--game <dir>]` builds the profile and installs it into the game folder: the database, the archives addons patch, the enabled tracks, their menu entries, the cars' ERS settings, and the [in-game loader](/reference/addons/#in-game-loader) with the session's `swofty/settings.json`. Every change goes through the journal. `swofty restore` or `swofty --profile vanilla apply` removes it. Close the game first.

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

`swofty mods install <addon folder>` and `swofty mods enable|disable <addon id>` still run the `swofty addons` command and print a note.

Built-in addons add their own top-level commands. `swofty --help` lists them.

Each command first moves what an earlier version stored in the data folder: addons in `$DATA/mods/` to `$DATA/addons/`, and tracks in `$DATA/maps/` into the Team Simplified library, turned on in the same profiles. `swofty --help`, `swofty --version` and mistyped commands leave the data folder alone.

## `swofty mods`

The built-in Team Simplified addon provides these commands. They work on its mod library and on the profile's load order.

| Command | Purpose |
|---|---|
| `swofty mods list` | List the library in load order, with each mod's id and whether it is on |
| `swofty mods import <path>…` | Import `.serps` packages, zips or folders that hold an `F1 25` folder; new mods are turned on |
| `swofty mods remove <id>` | Delete a mod from the library and from every profile |
| `swofty mods enable <id>` | Turn a mod on in the profile |
| `swofty mods disable <id>` | Turn a mod off in the profile |
| `swofty mods move <id> <position>` | Move a mod in the load order; position 1 loads first, and a later mod wins |

## `swofty settings`

| Command | Purpose |
|---|---|
| `swofty settings list [<id>]` | List addon settings with the profile's values and any pending rebuild |
| `swofty settings set <id> <option> <value>` | Set a value: `true`, `5`, `replica` |
| `swofty settings reset <id>` | Put an addon's settings back to their defaults |
| `swofty settings export [--out <file>]` | Write the session settings file that the game reads |
| `swofty settings merge` | Merge the changes made in game into the profile |

## `swofty maps`

`swofty maps` is a hidden alias of `swofty mods`. Tracks are mods: `swofty mods import <zip>` imports a community track package. Its mod id is the track code, or `<code>-track` when a mod in the library already has that id.

## `swofty update`

`swofty update` checks GitHub for the latest release. When it is newer, it installs it into the folder the command runs from: it downloads the archive for the platform, verifies it against the release's `SHA256SUMS`, extracts it to `.update/` in that folder and replaces the files in `bin/`, `loader/` and, on Linux, `share/`. Files in `loader/` that the new release does not have are removed. If a step fails, the files replaced so far are put back and `.update/` is removed. It refuses while F1 25 runs. When the command does not run from an extracted release that it can write to, it prints the release page instead.

`swofty update --check` only reports whether a newer release is available.

On Linux each new file is renamed over the old one. On Windows the old file is moved into `.update/` first, and the launcher removes `.update/` at its next start. For a release without `SHA256SUMS`, the download is checked against the size and digest that GitHub lists for the asset.

The launcher's check keeps its state under `"updates"` in `$DATA/config.json`:

| Key | Purpose |
|---|---|
| `check` | `false` turns off the launcher's check; `swofty update` still works. The default is `true` |
| `skip` | The version **Skip this version** chose; the launcher offers later versions again |
| `checked_at` | Time of the last check, in Unix seconds; the launcher checks again 6 hours later |
| `latest` | Tag of the newest release the last check found |

`SWOFTY_UPDATE_PRETEND_VERSION=0.1.0` makes the launcher and `swofty update` treat the running version as `0.1.0`, for testing an update.

## `swofty doctor`

`swofty doctor` checks Steam, the game install, Proton, launch options and the data folder. It also reports changes from the backed-up originals.
