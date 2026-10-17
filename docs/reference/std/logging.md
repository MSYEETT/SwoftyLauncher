---
title: Logging
description: Write to swofty/loader.log from a plugin, and configure the copy of the game's own log.
---

# Logging

Everything the loader, SwoftyMod and plugins log goes to `swofty/loader.log` in the game folder. The launcher shows it live while the game runs, and `swofty logs` summarises a previous session's logs and crashes.

## From a plugin

```cpp
template <class... A> void swofty::log(const char *fmt, A... a);
```

`printf`-style. It does nothing before `SwoftyAddonInit` stores the API. Prefix lines with the addon's name:

```cpp
swofty::log("[sample] %.*s from the sample mod", int(text.size()), text.data());
```

From C, call `api->log(fmt, ...)`.

## What gets logged

- Each plugin as it loads, and the API version it is given.
- Each signature scan and its hit count.
- Each mixin chain as it installs, conflicts and dropped addons.
- Each setting declared in code that `swofty.json` does not declare.
- Each node of an addon screen that the engine rejects, with its reason.
- Crashes: the exception, the registers and a stack named from `symbols.tsv`. Calls to `ExitProcess` and `TerminateProcess`, with the caller's stack.

## The game's own log

`swofty_gamelog.dll` copies the engine's log lines into `loader.log`: warnings and errors from every channel, and info from the UI and flow channels. Configure it in `swofty/session.json`:

```json
"game_log": { "level": "info", "channels": "default", "rate": 100, "repeat": 100, "max_lines": 20000 }
```

| Field | Default | Meaning |
|---|---|---|
| `enabled` | `true` | `false` turns the copy off |
| `level` | `warn` | `off`, `error`, `warn` or `info` |
| `channels` | The default rules | Comma-separated channel patterns, each with an optional `:level`. `*` matches any text, `default` adds the default rules and `all` matches every channel. A list of strings also works |
| `rate` | 100 | Lines per second. Further lines are dropped and counted |
| `repeat` | 100 | Times one format string is logged before its repeats are suppressed |
| `max_lines` | 20000 | Lines per session |

These environment variables take precedence:

| Variable | Field |
|---|---|
| `SWOFTY_GAME_LOG` | `level` |
| `SWOFTY_GAME_LOG_CHANNELS` | `channels` |
| `SWOFTY_GAME_LOG_RATE` | `rate` |
| `SWOFTY_GAME_LOG_REPEAT` | `repeat` |
| `SWOFTY_GAME_LOG_MAX` | `max_lines` |

`swofty/addons/core/log_channels.tsv` lists the engine's channels.
