# SwoftyMod core

The in-game half of Swofty Launcher for F1 25. `native/build.sh` writes it to `native/dist/` in the game folder's layout. The launcher places these files in the game folder for a modded session and removes them when the game exits.

| File | Source | Role |
|---|---|---|
| `version.dll` | `src/loader/version.c` | Proxy for the system `version.dll`. It forwards every export, does nothing in any process other than `F1_25.exe`, and loads every DLL in `swofty/addons/core/`, then `swofty/addons/*.dll`, just before the game creates its window. It also loads `swofty/mods/core/` and `swofty/mods/*.dll` with them, in name order, until the next minor release. |
| `swofty/addons/core/swofty_core.dll` | `src/core/` | SwoftyMod. Adds ADDON SETTINGS directly under CAREER on the home screen, runs the mixin API (version 3) for plugins, bridges ADDON SETTINGS commands to `swofty/settings.json` and the requests file, registers addon screens and drives their view models, reports the special liveries that addons add as owned (`owned_items` in `swofty/settings.json`), and writes crash reports. |
| `swofty/addons/core/swofty_gamelog.dll` | `src/gamelog/` | Copies the engine's own log lines into `loader.log`. |
| `swofty/addons/core/bindings.json` | `re/tools/bindgen.py` | Game functions, data and call sites that plugins can bind to, with their signatures. |
| `swofty/addons/core/symbols.tsv` | `re/tools/bindgen.py` | Function names by address, for crash reports. |
| `swofty/addons/core/log_channels.tsv` | `src/gamelog/log_channels.tsv` | The engine's log channels, for the game log filter. |
| `swofty/addons/core/features.json` | `FEATURES` in `src/core/stage.cpp` | The runtime asset features of this core, each `stable` or `experimental`. |

Everything logs to `<game>/swofty/loader.log`. Plugins get the C ABI in `packages/sdk/include/swofty_api.h` and the C++ helpers in `packages/sdk/include/swofty/mixin.hpp`. The generated game headers are in `include/swofty/game/`.

## Core

- **Settings bridge.** Core wraps `FeHome_Handle_Command` and handles the commands that `swofty/settings.json` assigns to each option, each addon's ENABLED switch and APPLY & RESTART. It updates plugin settings live and writes each change to the requests file that the launcher merges into the profile. APPLY & RESTART closes the game window so the launcher can rebuild and relaunch.
- **Crash log.** A vectored exception handler writes the exception, the registers and a stack named from `symbols.tsv` to `loader.log`. Calls to `ExitProcess` and `TerminateProcess` are logged with the caller's stack.
- **Crash reports.** On a fatal-class exception that reaches the top-level filter, the game's own fatal paths (a fatal `Ne_ReportMemoryError` heap error, hooked by its prologue, and `Ne_FatalAbort`), and an `ExitProcess`/`TerminateProcess`/`NtTerminateProcess` with a non-zero code or one that follows a fatal exception, core writes `swofty/crash-<time>.txt` and a minidump beside it, once per process. A dedicated thread writes them while the faulting thread waits, so a stack overflow or a corrupt stack still gets a report. The report holds the stack walk and a return-address scan, all registers, the bytes around `rip` and around every register that points into readable memory with its region, the faulting address's Ne heap block with its neighbours and guard state, the loaded addons and modules, a lock-free ring of recent flow, session and screen events and game-log warnings, the live profiler counters, the thread list and the last 200 `loader.log` lines. `MiniDumpWriteDump` writes the dump with the report as its comment stream, capped by a per-stack budget; `SWOFTY_CRASH_DUMP=full` dumps all memory and `off` skips it. Core keeps the last 8 reports and 3 dumps.
- **Addon screens.** Core reads the `"ui"` block of `swofty/settings.json` (or `swofty/ui.json`) and registers each listed screen file from `swofty/ui/` through the engine's own screen loader when the screen's flow loads. A route to an addon screen that is not registered is cancelled and the last screen reopens, instead of rebooting the front end. Every node the engine rejects is logged with its reason.
- **View models.** For each screen with `"view_model": true`, core registers a view model under `view_model_data.<screen>`. `swofty_addon_<N>` pages map to `addons[N]` of `swofty/settings.json`: they publish `title` and `o<k>.label`, `o<k>.text`, `o<k>.value`, `o<k>.min`, `o<k>.max` per option, and handle `swofty_next:<addon>:<option>`, `swofty_prev:<addon>:<option>` and `swofty_set:<addon>:<option>:<index>` through the settings bridge.
- **Plugins.** A plugin exports `SwoftyAddonInit` (`SWOFTY_ADDON()` in `mixin.hpp`). The loader also calls `SwoftyModInit` when that is the only export, and `SWOFTY_MOD()` is an alias of `SWOFTY_ADDON()`. Core reads `addons` and `owned_items[].addon` from `swofty/settings.json`, or `mods` and `owned_items[].mod` when the new fields are absent.
- **Mixins.** Plugins wrap functions, call sites and virtual table slots by binding name. Core orders each chain by priority and load order. With a `swofty/session.json`, registrations not declared there are refused.
- **Binding resolution.** Core resolves each binding from `bindings.json` by its byte pattern on the analysed build. A binding may also carry a `string_anchor` (an engine string, the referencing function's RVA and the offset of the signature match inside it). On a build whose PE timestamp matches the analysed one, core resolves by pattern and falls back to the anchor; on any other build it resolves by anchor first: it locates the string in the read-only data sections, finds the RIP-relative instruction in the code section that references it (the first `EXECUTE|READ` section, not a name), maps that site to the function base through the exception directory, then applies the binding's offset and resolve. Anchors marked `durable` resolve to the function base itself and hold across repacks; the rest reproduce the pattern result where the function's internal layout is unchanged. `re/tools/anchorcheck.py` proves every anchor resolves to its signature target on the Steam image.
- **KB addresses and the scan cache.** Each binding in `bindings.json` carries `match`, the RVA where its pattern matches on the analysed build. On that build core compares the pattern with the bytes at that address, or with the bytes under a hook core installed there, and scans the section only when they differ. Every other scan goes through core, which keeps the address of each pattern that a scan found exactly once in `swofty/cache/scan-cache.tsv`, keyed by the exe's PE headers, and checks the pattern at that address before it uses it. The loader scans with SSE2 on the two rarest bytes of the pattern. `SWOFTY_RESOLVE=scan` turns the KB addresses and the cache off.
- **Runtime assets.** Core reads `swofty/assets.json` and changes game content in memory, with nothing written into the game folder. Without the file it does nothing.

| Feature | Key in `swofty/assets.json` | Status | Core part |
|---|---|---|---|
| `group` | `asset_groups`, `asset_groups_auto` | stable | `assets.cpp`: entries added to the asset manifest after it is parsed |
| `erp` | `resource_erps`, `resources`, `resources_trace` | stable | `resources.cpp`: overlay archives the launcher built (`resource_erps`) or the core builds in `swofty/cache/` (`resources`), read first by every group that lists a target archive |
| `db` | `db_edits`, `early_stage` | stable | `gamedb.cpp`: edits applied when the game database loads |
| `lang` | `lang.<code>.strings`, `lang.<code>.table`, `lang.<code>.priority` | stable | `overlay.cpp`: string tables added after the stock table |
| `file` | `overlay`, `lang.<code>.replace` | experimental | `overlay.cpp`: files under `swofty/overlay/` served in place of game paths |

An experimental feature applies only when `"features"` in `swofty/assets.json` lists it. `"early_stage": true` makes the loader load addons inside the game's `CreateMutexW("Codemasters F1 25")` call, before the game database loads, and core runs its window-stage handlers at the window trigger. `SWOFTY_EARLY_STAGE=1` or `0` overrides it. File paths in `swofty/assets.json` are relative to `swofty/`.

## Game log

`swofty_gamelog.dll` logs warnings and errors from every channel, and info from the UI and flow channels, by default. Configure it with `"game_log": { "level", "channels", "rate", "repeat", "max_lines" }` in `swofty/session.json`, or with the `SWOFTY_GAME_LOG`, `SWOFTY_GAME_LOG_CHANNELS`, `SWOFTY_GAME_LOG_RATE`, `SWOFTY_GAME_LOG_REPEAT` and `SWOFTY_GAME_LOG_MAX` environment variables, which take precedence.

## Build

```sh
native/build.sh
```

Runs `re/tools/bindgen.py`, then cross-compiles the three DLLs with zig for `x86_64-windows-gnu` into `native/build/` and copies them to `native/dist/` with `features.json`. safetyhook, Zydis and zstd 1.5.7 are vendored in `third_party/` with their licences. safetyhook carries one local change, marked `Swofty:` in `trap_threads`: it keeps an ntdll page executable while it patches it. The build is deterministic.

The launcher takes the files from `loader/` beside its `bin/` folder in a release, from `native/dist/` in a checkout, or from the folder named by `SWOFTY_LOADER_DIR`.

`native/build.sh` also builds the addons a release bundles (`addons/re-optimization`) and packages each as `native/bundled/<id>.swoftyaddon` with `tools/pack_addon.py`. A release ships them in `addons/` beside `bin/`. At start, the launcher installs each package into `$DATA/addons/` switched off (`packages/core/src/bundled.rs`); it reads them from `addons/` beside `bin/`, from `native/bundled/` in a checkout, or from the folder named by `SWOFTY_BUNDLED_DIR`. `native/bundled/` stays outside `native/dist/`, which is copied into the game folder.

## Loading under Proton

Wine uses its builtin `version.dll` unless the prefix overrides it for the game. The launcher writes `[Software\Wine\AppDefaults\F1_25.exe\DllOverrides] "version"="native,builtin"` into the prefix's `user.reg` for the session and journals it. Windows needs no override.

## Wine check

```sh
native/test/run_wine_test.sh
```

Builds a small stand-in `F1_25.exe` that carries the signatures and the real home-menu instructions, the test plugins and test bindings, then runs it under Wine with `native/dist/` in each mode. It checks the proxy, the load trigger, loading from `swofty/addons/` and the legacy `swofty/mods/`, both init exports, the ADDON SETTINGS tile, mixin chains, refused and conflicting registrations, the settings bridge with and without a requests path and with the old settings fields, the ownership check for `owned_items`, `session.json`, the game log, and the crash log and reports: an access violation, a non-zero `ExitProcess`, a fatal Ne heap error (a block overrun into its neighbour, freed) and `Ne_FatalAbort` each write a `crash-<time>.txt` that parses into its sections, with the faulting heap block and its corrupt neighbour, and a real minidump beside it. A staged run loads one plugin at the early stage and two at the window stage, and checks that a window-stage binding still resolves on a function the early plugin already hooked, and that wrong or ambiguous patterns are still refused, that bindings resolve at their KB address without a scan, and that one whose KB address holds other bytes is scanned. Across the modes it checks that the scan cache is written on the first run, used on the next, rewritten when an address is stale or the exe differs, and off with `SWOFTY_RESOLVE=scan`. CI runs it on every push.

```sh
native/test/run_scan_test.sh
```

Checks the loader's scan against a byte-by-byte scan and the scan cache format on the host. With `work/exe/F1_25.exe` and a built `native/dist/`, it also checks that every binding matches at its KB address and has exactly one hit there, and prints the scan timings.

```sh
native/test/run_runtime_assets_test.sh
```

Builds a stand-in `F1_25.exe` that carries the runtime asset signatures and transcribes the game's asset manifest, archive loader, game database, file system mounts and language tables, then runs it under Wine with `native/dist/` in six modes. It checks the early stage and its wait for a load on another thread, resource overlays and their checks, game database edits with and without the early stage, string tables, the file overlay, asset groups, the experimental gate and that nothing changes without `swofty/assets.json`. CI runs it on every push.
