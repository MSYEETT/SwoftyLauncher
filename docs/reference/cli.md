# Command line

`swofty` does everything the launcher does, headless. On Linux it is also the Steam launch wrapper.

```
swofty run [-- <steam command>]
swofty build [--profile <name>]
swofty verify [--game <dir>]
swofty restore
swofty mods list
swofty mods install <path>
swofty mods enable <id>
swofty mods disable <id>
swofty maps list
swofty maps import <zip>
swofty maps remove <code>
swofty doctor
```

## `swofty run`

Starts a modded, offline session of the active profile. Any open journal is rolled back first.

- **From a terminal**, `swofty run` writes a launch request to `$DATA/state/launch.json` and asks Steam to start F1 25 (`steam -applaunch 3059520`). With `--direct`, or when the launch option is not installed, it starts the game through the Proton build Steam uses, under `steam-run` when that exists. It then follows the session and streams its logs live: `[loader]` and `[core]` lines from `swofty/loader.log`, `[wine]` errors and `[crash]` exceptions from the Proton log. Ctrl-C stops following; the game keeps running.
- **As the Steam launch option**, `swofty run -- %command%`, it is the wrapper: it creates `$DATA/sessions/<time>/` (with `latest.txt` pointing at it), records the Steam command and the environment, applies `launch.cpu_affinity` and `launch.env` from `config.json`, and runs `F1_25.exe` through Proton in place of the EA AntiCheat launcher, with `PROTON_LOG=1` and `WINEDEBUG` from the request (default `+timestamp,+pid,+tid,+seh,+loaddll,fixme-all`). See [Launch and injection](/reference/launch#linux-the-steam-wrapper).
- When the game exits, it prints a summary: exit code, run time, the last loader line, each distinct fatal-class exception with its count, and the count of Wine `err:` lines. Each faulting address is mapped to its module and, inside `F1_25.exe`, to a function: the bounds come from the exe's `.pdata` and the names from `re/db/functions.tsv`, which is built into the binary. An address with no `.pdata` bounds is reported as likely Denuvo `.data1`.

| Flag | Meaning |
|---|---|
| `--winedebug <channels>` | WINEDEBUG for the game process |
| `--restart` | Close a running F1 25 first |
| `--direct` | Start through Proton instead of asking Steam |
| `--game <dir>` | The game folder, when discovery does not find it |
| `--exe <file>` | A copy of `F1_25.exe` to read `.pdata` from (`SWOFTY_EXE_COPY`) |

## `swofty logs`

`swofty logs [<session dir>]` prints the same summary for a finished session, the latest one by default.

## `swofty build`

Resolves a profile (the active one by default), builds its files into `cache/overlay/<profile>/<buildhash>/`, and prints the conflict report. It does not start the game. A hard conflict fails the build.

## `swofty verify`

Reports every drift from the baseline, whether the launcher caused it or not: changed files, additions present (`version.dll`, `swofty/`), the prefix override present, open journals, and a build that differs from the baseline. `--game` names the game folder when discovery is not wanted. See [Baseline and journal](/reference/restore).

## `swofty restore`

Rolls back every open journal, removes every addition (`version.dll`, `swofty/mods`, the prefix override), restores every baselined file, and verifies that the folder is byte-identical to the baseline. It lists anything it could not restore and exits non-zero. See [Baseline and journal](/reference/restore).

## `swofty mods`

| Command | Effect |
|---|---|
| `mods list` | Installed mods, with versions and whether the active profile enables them |
| `mods install <path>` | Validates a mod directory or `.swoftymod` zip and copies it to `$DATA/mods/<id>/` |
| `mods enable <id>`, `mods disable <id>` | Changes the active profile. `swofty.core` cannot be disabled |

## `swofty maps`

| Command | Effect |
|---|---|
| `maps list` | Imported track packages, by track code |
| `maps import <zip>` | Validates a community track package and stores it unchanged in `$DATA/maps/<track_code>/`. See [Importing maps](/using/importing-maps) |
| `maps remove <code>` | Removes an imported package |

## `swofty doctor`

Prints what the launcher found and what is wrong: Steam roots and libraries, the F1 25 install and build, the exe hash, the Proton version and prefix, the launch option and the data directory, plus every drift from the baseline that `swofty verify` reports.
