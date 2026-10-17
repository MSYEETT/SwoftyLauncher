# SwoftyMod core

The in-game half of F1 Mod Manager. The launcher places these two DLLs in the game folder for a modded session and removes them when the game exits.

| File | Source | Role |
|---|---|---|
| `version.dll` | `src/loader/version.c` | Proxy for the system `version.dll`. It forwards every export, does nothing in any process other than `F1_25.exe`, and loads `swofty/mods/core/*.dll`, then `swofty/mods/*.dll`, just before the game creates its window. |
| `swofty/mods/core/swofty_core.dll` | `src/core/swofty_core.cpp` | SwoftyMod. Adds MOD SETTINGS directly under CAREER on the home screen with one safetyhook `MidHook` found by signature. It installs nothing if any signature fails to match exactly once. |

Both log to `<game>/swofty/loader.log`. Plugins get the C ABI in `packages/sdk/include/swofty_api.h`.

## Build

```sh
native/build.sh
```

Cross-compiles both DLLs with zig for `x86_64-windows-gnu` into `native/build/` and copies them to `native/dist/` in the layout above. safetyhook and Zydis are vendored in `third_party/` with their licences.

## Loading under Proton

Wine uses its builtin `version.dll` unless the prefix overrides it for the game. The launcher writes `[Software\Wine\AppDefaults\F1_25.exe\DllOverrides] "version"="native,builtin"` into the prefix's `user.reg` for the session and journals it. Windows needs no override.

## Wine check

```sh
native/test/run_wine_test.sh
```

Builds a small stand-in `F1_25.exe` that carries the four signatures and the real home-menu instructions, then runs it under Wine with both DLLs. It checks that the proxy forwards, that the hook installs before the window, that MOD SETTINGS lands directly under CAREER, and that a broken signature or a different host process leaves the game untouched. CI runs it on every push.
