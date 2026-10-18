---
title: Testing and publishing
description: Build a mod without touching the game, test it in game, read the logs, and publish a release.
---

# Testing and publishing

## Build

Start by building your profile so you can check its operations without touching the game folder:

```sh
swofty build
```

`swofty build` applies the profile's operations and writes the result to `$DATA/cache/overlay/<profile>/<buildhash>/`. You'll find `plan.json` there, listing the game files the build replaces and adds, along with their sources. The folder also holds the session's `swofty/settings.json`.

Read every line of output before you launch: it prints every `warn` message, each owned unlock row, skipped operations and each conflict. A conflict means a later addon replaced a target that an earlier addon wrote. Fix the warnings, then build again. See [`swofty build`](/reference/cli#swofty-build).

## Test in game

```sh
swofty run --apply
```

`swofty run --apply` installs your profile, starts the game and follows the logs. When the game exits, the game folder returns to its stock files. If the game crashes, use `swofty restore` to put them back.

| Log | Contents |
|---|---|
| `<game>/swofty/loader.log` | SwoftyMod, the runtime asset steps (`[assets]`), and the game's own warnings |
| `$DATA/sessions/<id>/` | One folder per session; `swofty logs` prints the latest summary |

First, test with only your mod enabled. Then test with the addons players combine it with. Try every value of each setting. Changing a `db` setting rebuilds the profile, so you'll need to build and relaunch after changing one.

## Publish

- For each release, bump `version` in `swofty.json`.
- Tell players which game build and season you tested on, and make sure `compat.game_builds` and `compat.seasons` match.
- If your mod includes other authors' files, credit every author and get permission to redistribute their files.
- To share your mod, package the folder as a `.swoftyaddon` — see [Packaging a mod](./packaging).
