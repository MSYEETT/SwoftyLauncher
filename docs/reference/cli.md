# Command line

From your terminal, `swofty` lets you manage addons, check files and read logs.

## `swofty run`

```sh
swofty run
```

Start Steam first, then run this to launch F1 25 and follow its logs. You can only play offline with mods.

When you leave the game, you'll get a summary with the exit code and any crash details. To stop following the logs while keeping the game running, press **Ctrl-C**.

| Flag | Purpose |
|---|---|
| `--profile <name>` | Choose a profile; otherwise, use the active profile |
| `--game <dir>` | Set the game folder if it wasn't found |
| `--restart` | Close a running F1 25 first |
| `--direct` | Launch directly instead of asking Steam |
| `--winedebug <channels>` | Choose Wine log channels |
| `--exe <file>` | Use a copy of `F1_25.exe` to identify crash locations |
| `--apply` | Install the profile (database, tracks, in-game loader) before the game starts and roll it back when it exits. The launcher always passes this flag |

On Linux, the [Steam launch option](/guide/linux) uses `swofty run -- %command%`.

For a direct launch, you'll find the wrapper's output in `$DATA/state/wrapper-run.log`. If the wrapper exits before the game starts or while it runs, `swofty run` prints the wrapper's last lines and its error, then exits with an error itself. It stops waiting after 10 minutes, or after 2 minutes when Steam launches the game.

## `swofty logs`

To read a finished session's summary, use `swofty logs [<session dir>]`. Leave out the folder to see the latest session.

## `swofty build`

Use `swofty build [--profile <name>]` when you want to prepare your profile's database and track files without starting the game. You'll find the output in `$DATA/cache/overlay/<profile>/<buildhash>/`: `swofty/settings.json` holds the session settings, and `plan.json` lists the game files to replace and add, along with their sources. Track archives, menu archives and car handling files are built in `$DATA/cache/maps`, `$DATA/cache/menus` and `$DATA/cache/ers`. Once the build succeeds, it removes older builds from those folders and your profile's overlay folder, keeping any that another profile's latest build uses.

The build also prepares addon ERP files and prints their warnings. Files with a `patch-pkg` edit other than `clone-entry` and `set-value` are skipped. Addon ERP operations work on the archives the track stages have already built; `add-mipmaps` files replace or add loose game files. If your build includes a `remove-resource`, it rewrites the archives even when `swofty_core` lists `erp` as `stable`. `file.put` operations and `operations.files` entries also replace or add loose game files. Other addon ERP and UI operations are currently skipped.

Some assets go into `swofty/assets.json` instead. This applies to each asset kind that `swofty_core` lists as `stable` in `swofty/addons/core/features.json`, or as `experimental` and named in `config.json` `"runtime_assets"`. These assets are asset groups, changed ERP resources, database records that differ from the game file, one language table per language and overlay files. For `group` and `lang` operations, this is the only delivery route: without the feature, they wait, and the build lists them. To keep `erp`, `db` or `file` kinds on the build route, list them in `config.json` `"build_assets"`.

You'll also see each conflict in the build output: a target that a later addon replaced after an earlier one wrote it.

When the game exits, `swofty run` merges any settings you changed in game into your profile. Choose **APPLY & RESTART** and it rebuilds the profile, then starts the game again.

## `swofty apply`

Close the game, then use `swofty apply [--profile <name>] [--game <dir>]` to build your profile and install it into the game folder. This includes the database, the archives addons patch, the enabled tracks, their menu entries, the cars' ERS settings, and the [in-game loader](/reference/addons/#in-game-loader) with the session's `swofty/settings.json`. Every change goes through the journal. To remove the installation, run `swofty restore` or `swofty --profile vanilla apply`.

## `swofty verify`

Use `swofty verify [--game <dir>]` to check whether your game files differ from the backed-up originals and whether they can be read and written correctly.

## `swofty restore`

Close the game, then run `swofty restore` to put back the backed-up files and remove launcher additions. If anything can't be restored, the command tells you.

See [Restoring the game](/using/profiles-and-restoring).

## `swofty addons`

| Command | Purpose |
|---|---|
| `swofty addons list` | List installed addons and whether they are enabled |
| `swofty addons install <path>` | Install an addon folder, a `.swoftyaddon` file or a zip that holds an addon |
| `swofty addons enable <id>` | Enable an addon |
| `swofty addons disable <id>` | Disable an addon; SwoftyMod cannot be disabled |

If you use `swofty mods install <addon folder>` or `swofty mods enable|disable <addon id>`, they still run the `swofty addons` command and print a note.

Built-in addons add their own top-level commands; run `swofty --help` to see them.

If you've used an earlier version, each command first moves its stored data: addons from `$DATA/mods/` to `$DATA/addons/`, and tracks from `$DATA/maps/` into the Team Simplified library. Those tracks stay turned on in the same profiles. `swofty --help`, `swofty --version` and mistyped commands leave the data folder alone.

## `swofty mods`

These commands come from the built-in Team Simplified addon. Use them to manage its mod library and your profile's load order.

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

## `swofty profile`

| Command | Purpose |
|---|---|
| `swofty profile list` | List the profiles and mark the active one |
| `swofty profile delete <name> [--yes]` | Delete a profile with its settings, build cache (`$DATA/cache/overlay/<name>/`), in-game changes and saves. It asks first; `--yes` skips the question |

`swofty profile delete` follows the same rules as the launcher's [Delete a profile](/using/profiles#delete-a-profile). It refuses `vanilla`, your only profile, any profile while F1 25 runs, and a profile with an open journal in the game folder; run `swofty restore` to close the journal. When you delete the active profile, `default` (or the first remaining profile) becomes active first.

## `swofty maps`

`swofty maps` is a hidden alias of `swofty mods`. Since tracks are mods, you can import a community track package with `swofty mods import <zip>`. Its mod id is the track code, or `<code>-track` if a mod in your library already has that id.

## `swofty update`

Close F1 25 before running `swofty update`; it refuses to update while the game runs. The command checks GitHub for the latest release and, if it's newer, installs it into the folder you're running the command from:

1. It downloads the archive for your platform and verifies it against the release's `SHA256SUMS`.
2. It extracts the archive to `.update/` in that folder.
3. It replaces the files in `bin/`, `loader/` and, on Linux, `share/`, and removes files from `loader/` that the new release no longer has.

If a step fails, it puts back the files replaced so far and removes `.update/`. You need to run the command from an extracted release it can write to; otherwise, it prints the release page instead.

To check for a newer release without installing it, use `swofty update --check`.

On Linux, each new file is renamed over the old one. On Windows, the old file moves into `.update/` first; the launcher removes `.update/` when you next start it. If a release has no `SHA256SUMS`, the command checks the download against the size and digest GitHub lists for the asset.

You'll find the state of the launcher's update check under `"updates"` in `$DATA/config.json`:

| Key | Purpose |
|---|---|
| `check` | `false` turns off the launcher's check; `swofty update` still works. The default is `true` |
| `skip` | The version **Skip this version** chose; the launcher offers later versions again |
| `checked_at` | Time of the last check, in Unix seconds; the launcher checks again 6 hours later |
| `latest` | Tag of the newest release the last check found |

To test an update, set `SWOFTY_UPDATE_PRETEND_VERSION=0.1.0`. The launcher and `swofty update` will treat the running version as `0.1.0`.

## `swofty doctor`

Use `swofty doctor` to check Steam, your game install, Proton, launch options and the data folder. You'll also see any changes from the backed-up originals.
