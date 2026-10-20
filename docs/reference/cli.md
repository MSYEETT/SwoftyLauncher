# Command line

From your terminal, `swofty` lets you manage addons and mods, build and run profiles, check the game files and read logs.

## `swofty run`

```sh
swofty run
```

Start Steam first, then run this to launch F1 25 and follow its logs. Modded play is [offline](/reference/safety) only.

When you leave the game, you'll get a summary with the exit code and any crash details. `swofty run` exits once the session's game files are rolled back.

To stop following the logs while keeping the game running, press **Ctrl-C**. When Steam launched the game, `swofty run` exits right away. For a direct launch, it keeps waiting to roll the game files back when the game exits; press **Ctrl-C** again to leave now, and the next start rolls them back.

| Flag | Purpose |
|---|---|
| `--profile <name>` | Choose a profile; otherwise, use the active profile |
| `--game <dir>` | Set the game folder if it wasn't found |
| `--restart` | Close a running F1 25 first |
| `--direct` | Launch directly instead of asking Steam |
| `--vr` | Start the game in VR, like Steam's **Launch in SteamVR mode**: passes `-vr` to the game and, on Linux, starts SteamVR first. See [Playing in VR](/using/vr) |
| `--winedebug <channels>` | Choose Wine log channels |
| `--exe <file>` | Use a copy of `F1_25.exe` to identify crash locations |
| `--apply` | Install the profile (database, tracks, in-game loader) before the game starts and roll it back when it exits. The launcher always passes this flag |

On Linux, the [Steam launch option](/guide/linux) uses `swofty run -- %command%`.

For a direct launch, you'll find the wrapper's output in `$DATA/state/wrapper-run.log`, and `swofty run` prints what the wrapper reports after the game exits. If the wrapper exits before the game starts or while it runs, `swofty run` prints the wrapper's last lines and its error, rolls back anything the wrapper left, then exits with an error itself. It stops waiting after 10 minutes, or after 2 minutes when Steam launches the game.

Before anything else, `swofty run` and `swofty apply` roll back what an unfinished session left in the game folder and print `Recovered the game folder from an unfinished session` with what they put back and the `swofty doctor` report. If a session is still running or rolling back, they wait up to 2 minutes for it.

If the wrapper receives a stop signal (Steam's **Stop**, `kill`, a closed terminal), it closes the game it started and rolls the session back before exiting.

## `swofty logs`

To read a finished session's summary, use `swofty logs [<session dir>]`. Leave out the folder to see the latest session.

### `swofty logs send`

```sh
swofty logs send --name <username> [--message <text>] [--dry-run [<zip>]]
```

Sends your logs to the developers on Discord, like **Send Logs to Developers** in the launcher. [Send your logs to the developers](/using/send-logs) lists what the zip holds.

| Flag | Purpose |
|---|---|
| `--name <username>` | Your Discord username, 2 to 32 characters. Without it, the command uses the last username sent, from `config.json` `"discord_username"` |
| `--message <text>` | What went wrong, up to 1000 characters |
| `--dry-run [<zip>]` | Write the zip to `<zip>` (a file or a folder; the current folder when left out), list its files, and send nothing |

The command prints its progress and exits with an error when Discord can't be reached or refuses the upload, naming the HTTP status. A zip over 9 MB goes in several messages, labelled `part 1/3` and so on. Builds from source can't send; there, only `--dry-run` works.

## `swofty build`

Use `swofty build [--profile <name>]` when you want to prepare your profile's database and track files without starting the game. You'll find the output in `$DATA/cache/overlay/<profile>/<buildhash>/`: `swofty/settings.json` holds the session settings, and `plan.json` lists the game files to replace and add, along with their sources. Track archives, menu archives and car handling files are built in `$DATA/cache/maps`, `$DATA/cache/menus` and `$DATA/cache/ers`. Once the build succeeds, it removes older builds from those folders and your profile's overlay folder, keeping any that another profile's latest build uses.

The build also prepares addon ERP files and prints their warnings. Files with a `patch-pkg` edit other than `clone-entry` and `set-value` are skipped. Addon ERP operations work on the archives the track stages have already built; `add-mipmaps` files replace or add loose game files. If your build includes a `remove-resource`, it rewrites the archives even when `swofty_core` lists `erp` as `stable`. `file.put` operations and `operations.files` entries also replace or add loose game files. Other addon ERP and UI operations are currently skipped.

Some assets go into `swofty/assets.json` instead. This applies to each asset kind that `swofty_core` lists as `stable` in `swofty/addons/core/features.json`, or as `experimental` and named in `config.json` `"runtime_assets"`. These assets are asset groups, changed ERP resources, database records that differ from the game file, one language table per language and overlay files. For `group` and `lang` operations, this is the only delivery route: without the feature, they wait, and the build lists them. To keep `erp`, `db` or `file` kinds on the build route, list them in `config.json` `"build_assets"`.

You'll also see each conflict in the build output: a target that a later addon replaced after an earlier one wrote it.

When the game exits, `swofty run` merges any settings you changed in game into your profile. Choose **APPLY & RESTART** and it rebuilds the profile, then starts the game again.

## `swofty apply`

Close the game, then use `swofty apply [--profile <name>] [--game <dir>]` to build your profile and install it into the game folder. This includes the database, the archives addons patch, the enabled tracks, their menu entries, the cars' ERS settings, and the [in-game loader](/reference/addons/#what-the-launcher-does-with-an-addon) with the session's `swofty/settings.json`. Every change goes through the journal. To remove the installation, run `swofty restore` or `swofty --profile vanilla apply`.

## `swofty verify`

Use `swofty verify [--game <dir>]` to check whether your game files differ from the backed-up originals and whether they can be read and written correctly.

## `swofty restore`

Close the game, then run `swofty restore` to put back the backed-up files and remove launcher additions. Files of unknown origin that stopped the last launch are moved to `$DATA/set-aside/<time>/` rather than deleted. If anything can't be restored, the command tells you.

See [Restoring the game](/using/profiles-and-restoring).

## `swofty addons`

| Command | Purpose |
|---|---|
| `swofty addons list` | List installed addons and whether they are enabled |
| `swofty addons install <path>` | Install an addon folder, a `.swoftyaddon` file or a zip that holds an addon |
| `swofty addons enable <id>` | Enable an addon |
| `swofty addons disable <id> [--yes]` | Disable an addon; SwoftyMod cannot be disabled |
| `swofty addons remove <id> [--yes]` | Delete an installed addon and take it out of every profile; built-in addons cannot be removed |

For an addon whose manifest sets [`saves.affects`](/reference/addons/manifest#saves), `disable` (when the addon is on) and `remove` print `Warning: <name> (<id>) has affected your saves. Careers you played with it may not load without it.`, then the addon's message, and ask `[y/N]`. `--yes` (`-y`) skips the question. When stdin is not a terminal, the command needs `--yes`; without it, nothing changes and the command exits with status 1.

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

## `swofty offline`

| Command | Purpose |
|---|---|
| `swofty offline status` | Show whether you've accepted the offline play terms, and the launch extension's digests and where it came from |
| `swofty offline accept` | Read the offline play terms and accept them |
| `swofty offline withdraw` | Withdraw consent; modded launches are refused |
| `swofty offline fetch` | Download the pinned release of the launch extension and verify its digests |
| `swofty offline install <dir>` | Verify and install the launch extension from a downloaded release folder |

See [Offline play](/reference/safety).

## Tracks

Tracks are mods: import a community track package with `swofty mods import <zip>`. Its mod id is the track code, or `<code>-track` if a mod in your library already has that id.

## `swofty update`

Close F1 25 before running `swofty update`; it refuses to update while the game runs. The command checks GitHub for the latest release and, if it's newer, installs it into the folder you're running the command from:

1. It downloads the archive for your platform and verifies it against the release's `SHA256SUMS`.
2. It extracts the archive to `.update/` in that folder.
3. It replaces the files in `bin/`, `loader/`, `addons/` and, on Linux, `share/`, and removes files from `loader/` and `addons/` that the new release no longer has.

If a step fails, it puts back the files replaced so far and removes `.update/`. You need to run the command from an extracted release it can write to; otherwise, it prints the release page instead.

To check for a newer release without installing it, use `swofty update --check`.

On Linux, each new file is renamed over the old one. On Windows, the old file moves into `.update/` first; the launcher removes `.update/` when you next start it. If a release has no `SHA256SUMS`, the command checks the download against the size and digest GitHub lists for the asset.

The launcher's update check reads `"updates"` in `$DATA/config.json`:

| Key | Purpose |
|---|---|
| `check` | `false` turns off the launcher's check for new launcher releases; `swofty update` still works and Simplified for Swofty is still kept up to date. The default is `true` |

Older configs may still hold `skip`, `checked_at` and `latest` under `"updates"`, and `checked_at` under `"managed"`. The launcher ignores them and drops them the next time it saves the config.

The launcher reads the latest release tag from the `releases/latest` redirect on github.com, which doesn't count against GitHub's API rate limit. It calls the GitHub API once for each new release, to get its downloads, and waits for `Retry-After` or the rate limit reset when GitHub refuses a request.

To test an update, set `SWOFTY_UPDATE_PRETEND_VERSION=0.1.0`. The launcher and `swofty update` will treat the running version as `0.1.0`.

## `swofty doctor`

Use `swofty doctor` to check Steam, your game install, Proton, launch options and the data folder. You'll also see any changes from the backed-up originals. On Windows, the `run as admin` row shows whether `F1_25.exe` is set to run as administrator, which stops modded sessions from starting (see [Launching the game](/reference/launch#windows)).
