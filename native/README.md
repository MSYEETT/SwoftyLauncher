# SwoftyMod core

The in-game half of F1 Mod Manager. `native/build.sh` writes it to `native/dist/` in the game folder's layout. The launcher places these files in the game folder for a modded session and removes them when the game exits.

| File | Source | Role |
|---|---|---|
| `version.dll` | `src/loader/version.c` | Proxy for the system `version.dll`. It forwards every export, does nothing in any process other than `F1_25.exe`, and loads every DLL in `swofty/mods/core/`, then `swofty/mods/*.dll`, just before the game creates its window. |
| `swofty/mods/core/swofty_core.dll` | `src/core/` | SwoftyMod. Adds MOD SETTINGS directly under CAREER on the home screen, runs the mixin API (version 3) for plugins, bridges MOD SETTINGS commands to `swofty/settings.json` and the requests file, and writes crash reports. |
| `swofty/mods/core/swofty_gamelog.dll` | `src/gamelog/` | Copies the engine's own log lines into `loader.log`. |
| `swofty/mods/core/bindings.json` | `re/tools/bindgen.py` | Game functions, data and call sites that plugins can bind to, with their signatures. |
| `swofty/mods/core/symbols.tsv` | `re/tools/bindgen.py` | Function names by address, for crash reports. |
| `swofty/mods/core/log_channels.tsv` | `src/gamelog/log_channels.tsv` | The engine's log channels, for the game log filter. |

Everything logs to `<game>/swofty/loader.log`. Plugins get the C ABI in `packages/sdk/include/swofty_api.h` and the C++ helpers in `packages/sdk/include/swofty/mixin.hpp`. The generated game headers are in `include/swofty/game/`.

## Core

- **Settings bridge.** Core wraps `FeHome_Handle_Command` and handles the commands that `swofty/settings.json` assigns to each option, each mod's ENABLED switch and APPLY & RESTART. It updates plugin settings live and writes each change to the requests file that the launcher merges into the profile. APPLY & RESTART closes the game window so the launcher can rebuild and relaunch.
- **Crash log.** A vectored exception handler writes the exception, the registers and a stack named from `symbols.tsv` to `loader.log`. Calls to `ExitProcess` and `TerminateProcess` are logged with the caller's stack.
- **Mixins.** Plugins wrap functions, call sites and virtual table slots by binding name. Core orders each chain by priority and load order. With a `swofty/session.json`, registrations not declared there are refused.

## Game log

`swofty_gamelog.dll` logs warnings and errors from every channel, and info from the UI and flow channels, by default. Configure it with `"game_log": { "level", "channels", "rate", "repeat", "max_lines" }` in `swofty/session.json`, or with the `SWOFTY_GAME_LOG`, `SWOFTY_GAME_LOG_CHANNELS`, `SWOFTY_GAME_LOG_RATE`, `SWOFTY_GAME_LOG_REPEAT` and `SWOFTY_GAME_LOG_MAX` environment variables, which take precedence.

## Build

```sh
native/build.sh
```

Runs `re/tools/bindgen.py`, then cross-compiles the three DLLs with zig for `x86_64-windows-gnu` into `native/build/` and copies them to `native/dist/`. safetyhook and Zydis are vendored in `third_party/` with their licences. The build is deterministic.

The launcher takes the files from `loader/` beside its `bin/` folder in a release, from `native/dist/` in a checkout, or from the folder named by `SWOFTY_LOADER_DIR`.

## Loading under Proton

Wine uses its builtin `version.dll` unless the prefix overrides it for the game. The launcher writes `[Software\Wine\AppDefaults\F1_25.exe\DllOverrides] "version"="native,builtin"` into the prefix's `user.reg` for the session and journals it. Windows needs no override.

## Wine check

```sh
native/test/run_wine_test.sh
```

Builds a small stand-in `F1_25.exe` that carries the signatures and the real home-menu instructions, nine test plugins and test bindings, then runs it under Wine with `native/dist/` in nine modes. It checks the proxy, the load trigger, the MOD SETTINGS tile, mixin chains, refused and conflicting registrations, the settings bridge with and without a requests path, `session.json`, the game log, the crash report and the `ExitProcess` log. CI runs it on every push.
