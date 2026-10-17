# Hooking game functions

Native plugins are Windows x86-64 DLLs that run inside F1 25. They are for mod authors who need code changes beyond data edits.

## SDK

Use [`packages/sdk/include/swofty_api.h`](https://github.com/Swofty-Developments/F1ModManager/blob/main/packages/sdk/include/swofty_api.h). The SDK uses the MIT licence.

Export this function:

```c
__declspec(dllexport) int SwoftyModInit(const SwoftyApi *api);
```

Return non-zero on success or 0 on failure. The DLL stays loaded either way, so leave no hooks installed after a failure.

The current API provides:

| Member | Purpose |
|---|---|
| `version` | Check the API version |
| `log` | Write to `swofty/loader.log` |
| `module_base` | Get the base address of `F1_25.exe` |
| `scan` | Find a code pattern; use the result only when exactly one match is found |

The [Game API](/reference/game/) lists the game functions and structs that plugins can use.

## Manifest entry

```json
"operations": {
  "native": [
    { "dll": "native/my_plugin.dll", "capabilities": ["log", "scan", "hook"] }
  ]
}
```

The manifest also accepts `read-db`, `options` and `on-frame`. These are planned capabilities, not members of the current API.

## Hook rules

- Find addresses by signature, not fixed addresses. Stop if a signature has zero or multiple matches.
- Hook only `.xtls` code. Do not patch `.data1` bodies.
- Keep hook setup out of `DllMain`.
- Keep modded play offline only.

Plugin approval and file-hash checks are planned. The current loader loads DLLs from `swofty/mods/core/`, then `swofty/mods/`.

## Build

Use [the mod template](/modding/template). It builds with `zig cc -target x86_64-windows-gnu`. Other toolchains that produce Windows x86-64 DLLs, including MSVC, also work.
