# Native plugins

A native plugin is a Windows x86-64 DLL that the in-game loader loads into `F1_25.exe` during a modded session. Plugins can scan the game's code, hook it and read mod options.

## The SDK

The plugin ABI is C, in [`packages/sdk/include/swofty_api.h`](https://github.com/Swofty-Developments/F1ModManager/blob/main/packages/sdk/include/swofty_api.h). The SDK is MIT, so plugins can use any licence.

```c
#define SWOFTY_API_VERSION 1

typedef struct SwoftyApi {
    uint32_t version;
    void (*log)(const char *fmt, ...);
    uintptr_t (*module_base)(void);
    int (*scan)(const char *section, const char *pattern, uintptr_t *out);
} SwoftyApi;

typedef int (*SwoftyModInitFn)(const SwoftyApi *api);
```

A plugin exports one function:

```c
__declspec(dllexport) int SwoftyModInit(const SwoftyApi *api);
```

It returns non-zero on success and 0 on failure. The loader logs the result either way and does not unload the DLL, so a plugin that fails must leave nothing installed.

| Member | Meaning |
|---|---|
| `version` | The API version the loader provides. Check it before using newer members |
| `log` | `printf`-style logging into `swofty/loader.log` |
| `module_base` | The base address of `F1_25.exe` |
| `scan` | Searches a PE section (by name, or every executable section when `NULL`) for a hex pattern with `??` wildcards, such as `"45 8D 67 01 ?? 8B"`. Returns the number of hits. Writes the address to `out` only when there is exactly one hit, and 0 otherwise |

The `swofty-sdk` Rust crate mirrors these types for plugins written in Rust.

## Declaring a plugin

List the DLL in the manifest with the capabilities it uses:

```json
"operations": {
  "native": [
    { "dll": "native/my_plugin.dll", "capabilities": ["log", "scan", "hook"] }
  ]
}
```

| Capability | Meaning |
|---|---|
| `log` | Writes to the loader log |
| `scan` | Searches the game's code |
| `hook` | Installs code hooks |
| `read-db` | Reads the game database |
| `options` | Reads the mod's options |
| `on-frame` | Runs a callback every frame |

## Consent and pinning

- The first time a profile enables a mod with a native plugin, the launcher shows the DLL, its sha256 and its declared capabilities, and asks for consent. Consent is per DLL hash, so an updated DLL asks again.
- The session file pins each consented DLL's sha256. The loader refuses any DLL that is not pinned or whose hash differs.
- Plugins load after the game's first window appears, core plugins first, then the rest in sorted order.

Consent and hash pinning belong to the loader's safety gate, which is not written yet. The current loader loads every DLL in `swofty/mods/core/` and then `swofty/mods/`.

## Rules for hooks

- Resolve every address by signature scan, never by a hard-coded address. A signature must match exactly once; zero or several hits means the plugin installs nothing and logs why.
- Hook only code in `.xtls`, the game's real code section. Functions relocated into `.data1` are reached through 5-byte jump stubs; call them, but never hook or patch them.
- Install hooks after the first window, not in `DllMain`.
- A plugin must never touch the anticheat, online services or the session's offline state.

The loader bundles [safetyhook](https://github.com/cursey/safetyhook) for its own hooks. A plugin may link its own copy.

## Building

The loader and the template's plugin are cross-built from Linux with `zig cc -target x86_64-windows-gnu`. Any toolchain that produces an x86-64 Windows DLL works, including MSVC. See [the mod template](/modding/template).
