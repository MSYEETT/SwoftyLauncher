---
title: Testing and publishing
description: Build a mod, test it in game, read the logs, and publish it.
---

# Testing and publishing

## Build

```sh
swofty build
```

`swofty build` applies the profile's operations without touching the game folder. It prints each `warn` message, every owned unlock row, the operations it skips and every conflict: a target that a later addon replaced after an earlier one wrote it. The output is in `$DATA/cache/overlay/<profile>/<buildhash>/`, with `plan.json` listing the game files the build replaces and adds. See [`swofty build`](/reference/cli#swofty-build).

## Test in game

```sh
swofty run --apply
```

`swofty run --apply` installs the profile, starts the game and follows the logs. When the game exits, the game folder returns to its original files. After a crash, `swofty restore` puts the original files back.

| Log | Contents |
|---|---|
| `<game>/swofty/loader.log` | SwoftyMod, plugins and the game's own warnings and errors |
| `$DATA/sessions/<id>/` | One folder per session. `swofty logs` prints the latest summary |

Test with only your mod enabled, then with the addons players are likely to combine it with. Test every value of each setting.

## Publish

- Bump `version` in `swofty.json` for each release.
- State the game build and season it was tested on, and match `compat.game_builds`.
- Credit every author whose files the mod carries, and redistribute their files only with permission.
- Ship the folder as described in [Packaging a mod](./packaging).
