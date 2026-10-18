---
title: Logging
description: Write to swofty/loader.log from a plugin with swofty::log, and configure the copy of the game's own log.
---

# Logging

You'll find everything the loader, swofty_core and plugins write in `swofty/loader.log` in the game folder. Watch it live in the launcher while the game runs, or use `swofty logs` for a summary of a past session's logs and crashes.

| | |
|---|---|
| Header | `swofty/mixin.hpp` |
| Member | `SwoftyApi::log` |
| Feature | `log` |
| API version | 1 |
| Thread | Any |

## From a plugin

Call `swofty::log` with a `printf` format. You're sharing the log with the loader and other plugins, so prefix each line with your addon's name:

```cpp
#include <swofty/mixin.hpp>

SWOFTY_ON_LOAD
{
    swofty::log("[my-addon] loaded, game base 0x%llx", (unsigned long long)swofty::api->module_base());
}

SWOFTY_ADDON()
```

Wait until `SwoftyAddonInit` stores the API before logging; `swofty::log` writes nothing before then. If you're writing C, call `api->log(fmt, ...)`. You'll see the line in `swofty/loader.log`:

```text
[my-addon] loaded, game base 0x7ff6a4120000
```

## What the log holds

To trace what happened to your addon, look for:

- Plugins as they load, with the API version each receives: `addon my_plugin.dll: loaded at 0x..., calling SwoftyAddonInit (API v5)`.
- Signature scans and the hit count for each.
- Mixin chains as they install, including every conflict and dropped addon.
- Settings declared in code but missing from `swofty.json`.
- Addon screen nodes the engine rejects, with a reason for each.
- Crashes, with the exception, registers and a stack named from `symbols.tsv`; calls to `ExitProcess` and `TerminateProcess` include the caller's stack.

## The game's own log

You'll also see the engine's own messages in `loader.log`. `swofty_gamelog.dll` copies warnings and errors from every channel, and info from the UI and flow channels. To configure that copy, edit `swofty/session.json`:

```json [swofty/session.json]
"game_log": { "level": "info", "channels": "default", "rate": 100, "repeat": 100, "max_lines": 20000 }
```

| Field | Default | Meaning |
|---|---|---|
| `enabled` | `true` | Set to `false` to turn the copy off |
| `level` | `warn` | `off`, `error`, `warn` or `info` |
| `channels` | The default rules | Supply comma-separated channel patterns or a list of strings, each with an optional `:level`. `*` matches any text, `default` adds the default rules, `all` matches every channel |
| `rate` | 100 | Lines per second; lines beyond this limit are dropped and counted |
| `repeat` | 100 | How many times one format string is logged before further repeats are suppressed |
| `max_lines` | 20000 | Lines per session |

You can override those fields with environment variables. If you set both, the environment variable takes precedence:

| Variable | Field |
|---|---|
| `SWOFTY_GAME_LOG` | `level` |
| `SWOFTY_GAME_LOG_CHANNELS` | `channels` |
| `SWOFTY_GAME_LOG_RATE` | `rate` |
| `SWOFTY_GAME_LOG_REPEAT` | `repeat` |
| `SWOFTY_GAME_LOG_MAX` | `max_lines` |

To find the engine's channel names, look in `swofty/addons/core/log_channels.tsv`.
