# Swofty Mod Launcher: Design v0.3

> Paths under `work/` in this document refer to the research workspace, which is not part of this repository.

Date: 2026-10-17. Scope: F1 25 (Steam app 3059520, build 25251167, `F1_25.exe` sha256 `d873a1c8…`), including the F1 2026 season content. Platforms: Windows, and Linux through Steam + Proton (including Steam Deck). Facts come from the notes in `research/`; runtime results come from the lead's tests with the user and are marked as such.

---

## 0. Decisions at a glance

1. **Everything shipped host-side is Rust** (user decision), in one Cargo workspace.
   - GUI: GPUI (Zed's UI framework, crate `gpui`, Apache-2.0), plus `gpui-component` widgets where useful, styled to look like Zed. GPUI is pre-1.0; risks in §16.
   - Core library and CLI: Rust. The formats (GVDB, ERP, `!pkg`, BXML, texarray) are ported from the Python tools, which stay as the byte-identity test oracle and do not ship.
   - Ptasev's C# EgoEngineLibrary is a format reference only. No code is reused.
2. **The in-game loader is C/C++ with safetyhook for now**, in `native/`: a C `version.dll` proxy and a C++23 `swofty_core.dll` (safetyhook `MidHook` with full register context, bundled Zydis). A later move to Rust is possible (§12).
3. **Injection is a `version.dll` proxy** next to `F1_25.exe` (`exe.md` §2.2). It is a static import, so the OS loader maps it and runs its `DllMain` before the exe's entry point and Denuvo's stub. It works on Windows with no configuration and under Proton with a prefix-scoped DLL override. A create-suspended injector (`SwoftyHost.exe`) is the fallback.
4. **The game-folder footprint is session-scoped.** A modded session adds `version.dll` and the `swofty/` folder (core DLL, consented plugins, log) for its duration only. The launcher places them through the restore journal and removes them on exit, because EA AntiCheat flags unsigned modules and they must never be present for a vanilla or online launch. Mod data (ERPs, GVDB, loose files) is delivered by the loader's VFS with the game's data files untouched; journaled apply/restore is the fallback.
5. **Launch integration.**
   - Linux: a Steam launch-option wrapper, `swofty run -- %command%`. It keeps Steam's Proton, Steam Linux Runtime, prefix and ownership, and runs `F1_25.exe` directly in place of the EA AntiCheat launcher. The anticheat does not run under Proton.
   - Windows: the launcher's "Play modded (offline)" starts `F1_25.exe` itself. Steam's Play button stays vanilla, online and anticheat-on.
6. **One mod format** (`swofty.json`) with declarative operations: database ops (named paths, row selectors, dataset scoping), ERP ops (add, replace, copy, `!pkg` patch), loose files, and native plugins. EGO track zips are accepted unmodified through an adapter.
7. **Safety.** Mods load only in a session the launcher prepared. The loader fails closed unless a fresh ticket is present, no anticheat module is loaded and the host is `F1_25.exe`. The session is held offline in-process, and modded saves are kept away from Steam Cloud and vanilla saves. Nothing is designed to evade detection online.
8. **In-game Swofty Mod.** The F9 "MOD SETTINGS" prompt, the bottom-left panel and the settings screen are data, generated per session. The tile directly under Career is native: one safetyhook `MidHook` at `HOME_AFTER_CAREER` (0x141742fd6).
9. **24 cars is most likely DB-only; 26 needs more.** The 22-car limit was data: every GVDB table stores its row count in an `_instancecount_` key, and rows added past the count are never read (§2.2). A DB fix that also bumps the counts (variant B24) is built and awaits one in-game confirmation. The 26-car (13-team) showcase needs track-side additions and the 11-entry logo arrays, and probably exe changes wherever per-car pools turn out to be fixed-size.
10. **A reverse-engineering program runs alongside the milestones** under `re/PROTOCOL.md`: an `re/` knowledge base with evidence levels L0–L3, a trace plugin for runtime confirmation, and independently verified car-count audits per subsystem (§14.1).

---

## 1. Goals and non-goals

**Goals**
- G1. One-click modded offline play on Windows and on Linux via Steam + Proton (including Steam Deck), from the Swofty launcher or, on Linux, from Steam's Play button.
- G2. A modded session can never be online, and nothing modded is ever present in a session that runs with the anticheat.
- G3. Non-destructive by default. The game's data files stay byte-identical to Steam's copy, "verify files" passes, and uninstalling is trivial. The only additions (`version.dll` and `swofty/`) exist only during a modded session.
- G4. One mod format covering assets, the database, UI, track packages and native plugins, with deterministic builds, conflict reporting and profiles.
- G5. The in-game Swofty Mod: a MOD SETTINGS tile under Career, the F9 prompt, a permanent "Launched with Swofty Mod Launcher" panel, and a settings screen.
- G6. Survives game updates. Data mods resolve by name and label, code hooks by signature, and mods re-apply automatically.
- G7. Showcase mods: the home-menu work, a 24-car F1 2026 grid (12 teams), then a 26-car grid (13 teams).
- G8. One Rust toolchain for everything shipped host-side, with a Zed-like GUI.

**Non-goals**
- Online or multiplayer modding, and anything that hides mods from the anticheat or online services.
- macOS (CrossOver, Whisky, GPTK), by user decision.
- Editing localisation (`game.dat`/`.lng`) in v1. Literal-text tricks are used instead.
- A hosted mod repository or marketplace in v1.
- Asset authoring (3D, AC-to-Ego conversion). ModKit only packages and diffs.
- Reverse engineering PavelGames' editor. We interoperate with its public package format only.
- Other EGO titles in v1. Game-specific facts live in a game profile so F1 24/26 can be added later.

---

## 2. Ground truth this design relies on

Proven by the research notes: GVDB read/write with row additions and a byte-identical round trip (`gamedb.md`); byte-identical ERP rebuilds and `!pkg` read/write with a breadth-first layout and hash-sorted keys (`ui.md` §1); a data-only F9 prompt, `mod_menu` route and static tile that work in game (`ui.md` §4b); the 24-slot `vehicles_current_myteam.layer` in the `_f1_26_myteam` world of all 28 tracks (`grid-assets.md` §2); the EGO track-package layout (`track-packages.md`); and the native code layer (`exe.md`).

### 2.1 Exe and injection

- **The code is statically readable.** About 88% of functions are plain compiler output in `.xtls` (the real `.text`) with full `.pdata` bounds. The rest are relocated into `.data1` and reached through 5-byte `jmp rel32` stubs. Rule: hook only `.xtls`; call `.data1` bodies but never hook them.
- **No ASLR.** The PE header of `work/exe/F1_25.exe` has Characteristics 0x23 (relocations stripped) and DllCharacteristics 0x8120 (no DYNAMIC_BASE). The image always loads at 0x140000000, so a verified per-build address cache is valid between runs.
- **`version.dll` is a static import** (IAT in `.xtext`). The game uses 6 of its 17 exports. It is not on the input path, unlike `dinput8`, which matters because the user drives a G920 with force feedback. It is not a KnownDLL. The fallback name is `dinput8.dll`; `winmm` would need about 180 forwards.
- **Proton and Windows mechanics.** Wine needs `WINEDLLOVERRIDES="version=n,b"` or, more robustly, the prefix-scoped key `[Software\Wine\AppDefaults\F1_25.exe\DllOverrides] "version"="native,builtin"` (app id looked up from the user's `compatdata`). Windows needs no configuration, but the game must start without EA AntiCheat, which blocks or flags unsigned modules. That means offline only, which matches this design.
- **The loader self-gates to `F1_25.exe`.** A `version` override can also load the proxy into other processes in the same prefix, so the proxy checks `GetModuleFileNameW(NULL)` and only forwards elsewhere.
- **No signed-load blocker.** `WinVerifyTrust` has three callers and none references `version`, `dinput8` or `winmm`; the certificate checks target vendor DLLs (Logitech, likely Streamline). There is no evidence of code-section self-checksumming in the `.xtls` paths we hook. Code hooks are installed late (after the first window), not in `DllMain`.
- **The home tile under Career is one `MidHook`** at `HOME_AFTER_CAREER` = 0x141742fd6, right after the Career adder call (0x141742fd1) in the shared tile-list builder 0x14173df80 (`exe.md` §3). It needs the v2 data files (the `mod_menu` route in `persistent.erp`, the tooltip entry in `databases.erp`), and the v3 static tile must be dropped or the tile appears twice.
- **Other anchors:** `FEHomeViewModel::onCommandRaised` (0x141bb3c20) for custom commands; the gamedb load (0x1445a0350, `work/exe/decomp/`); the eaid resolver (0x143b130a0, 0x143b13710; `exe.md` §5), not yet analysed.

### 2.2 Grid size: what is data and what is not

| Finding | Source |
|---|---|
| The world is chosen by DB data. World-variant table `{446668bc25ba571c}`: row 8 `f1_26` → `_f1_26.wep` (22 grid slots, 11 garages); row 9 `myteam_26` → `_f1_26_myteam.wep` (24, 12), reached only through `track_layers_override=myteam_26` in the custom-team configs. | `exe.md` §4 |
| The entrant setup-mode table `entrants.entrant_setup_modes` (`{9cf63e2e9fcfd89e}`) has `max_entrants=22` in rows 8 (`f1_26`) and 9 (`f1_generic_2026`), and `max_teams=-1` (no limit). | `exe.md` §4, `grid24-cap.md` §2 |
| **Every GVDB table stores its row count** in a key `<container>._<row>_instancecount_`, and the game reads only rows `0..count-1`. Pristine counts: `entrant_list_34` 22, `entrants.teams` 358, `entrants.vehicles` 413. | `grid24-cap.md` §1 |
| The F1 2026 grid is `entrants.entrant_lists.entrant_list_34`: 22 entrants, 11 teams × 2. | `grid24-experiment.md` |
| The participant path (candidate pool, selector, `Weekend::start`, garage assignment) uses heap vectors. There is no literal 22 and no 22-sized array on it; the selector's limit is `min(max_entrants, #candidates)`. | `grid24-cap.md` §2–3 |
| The only code-side limits found are dev-only: the `Max ai participants` debug slider (0x140bc6f70), and the benchmark config's `num_cars`/`grid_pos` clamp to [1, 26] (0x1407a2daa), which suggests the engine anticipates up to 26 cars. | `grid24-cap.md` §3 |
| No track layer has more than 24 grid slots, 12 garages or 12 pit boxes. | `grid-assets.md` §3 |

**Runtime results.** The primary variants in `work/experiments/grid24/` each replace only `gamedatabases.2025.bin`. All of them point row 8's world at `_f1_26_myteam` and raise `max_entrants` to 24. The world switch works: garage 12's door is open in game.

| Variant | Extra rows | Result |
|---|---|---|
| A | `entrant_22/23` as Cadillac clones (Jack Tremblay #66, Rashid Nair #65) | 22 drivers on the select screen, 22 cars in the session |
| B | 12th team `team12_26` (T12), car `team12_2026` (clone of `myteam_honda_2026`), two entrants | T12 is selectable on the select screen. Player on McLaren: 22 cars, T12 absent. Player on T12: works. |

**Explanation (`grid24-cap.md`).** A and B added rows without bumping the table counts, so the entrant-list parser never read `entrant_22/23`, and `team_358`/`vehicle_413` sat outside their tables. The player's T12 car worked because the selector inserts the player's selection itself and resolves its team and car by label (its AI teammate was probably still missing). How the select screen still listed T12 is not traced (probably a separate reader). **Variant B24** is B plus the three count bumps (24, 359, 414); it is built and verified (`build/verify_b24.py`, 0 failures) and awaits the in-game test. So 24 cars is most likely DB-only, pending that one runtime confirmation. If a code fallback is ever needed, the natural `MidHook` site is the selector's limit load at 0x1424a30a8.

**Beyond 24.** The selection and session layers are vector-based, so the limits on a 26-car grid sit elsewhere: the world files (24 grid slots, 12 garages, 12 pit boxes; grid slots 25–26, garage 13 and pit box 13 must be added per track, `research/grid26-tracks.md`), the 11-entry logo arrays (`ui.md` §6), and probably per-car pools that are not yet audited (physics and AI, replay and results, the HUD timing tower).

### 2.3 Platform and launch facts

- **Steam's launch command** for app 3059520 targets `EAAntiCheat.GameServiceLauncher.exe`. The modded path runs `F1_25.exe` directly under the same Proton, prefix and ownership instead.
- **`preloader_l.dll`** (EA Javelin) contains "Wine, Proton, and Steam Deck are not supported by this application!", so the anticheat does not run under Proton.
- **Saves** go through ISteamRemoteStorage and sync via Steam Cloud (`userdata/<steamid>/3059520/remote/savegame@profile#…`), so modded career progress must be isolated (§9.2).
- **The "Retry connection" element is data:** `fe_home.contextual_panels.offline_contextual_panel`, shown by `contextual_decision [MVCD]` on `offline`; its `extra_2_listener` raises the native `panel_pressed` reconnect (`ui.md` §4b).

---

## 3. Architecture

```
 HOST OS (Windows, or Linux incl. SteamOS)          GAME PROCESS (native Windows, or inside the Proton prefix)
 ─────────────────────────────────────────          ──────────────────────────────────────────────────────────
  swofty-launcher (GPUI)        swofty-cli (swofty)
   library, profiles,            run (Steam wrapper), build,
   conflicts, setup, Play        verify, restore, mods, doctor
             └──────────────┬─────────────┘          placed for the session only (journaled):
                            v                          <game>/version.dll
                swofty-core (Rust crate)               <game>/swofty/mods/core/swofty_core.dll
  +------------------------------------------+         <game>/swofty/mods/*.dll  (consented plugins)
  | Store     mods/ archives/ profiles/      |       Linux: AppDefaults\F1_25.exe\DllOverrides
  | Resolver  deps, load order, compat       |              "version"="native,builtin"
  | Patcher   DB merge, ERP + !pkg ops,      |
  |           loose files, EGO adapter       |
  | Overlay   cache/overlay/<buildhash>/     |  session.json  +--- F1_25.exe ---------------------------------+
  | Baseline  pristine manifests + journal   | -------------> | version.dll (C), mapped before entry point    |
  | Steam     vdf, libraries, launch option, |                |  forward 17 exports; host != F1_25 -> inert   |
  |           prefix registry                |                |  gate: ticket+expiry, exe hash, no anticheat  |
  | Session   ticket, overlay map, options   |                |  first window -> load swofty/mods/core, mods  |
  +------------------------------------------+                |  SwoftyApi: log, module_base, scan            |
        ^        |                                            |  +-- swofty_core.dll (C++, safetyhook) ----+  |
        |        v                                            |  | VFS: NtCreateFile/NtOpenFile -> overlay |  |
        |  $DATA/cache/overlay/<hash>/  <-- reads ------------|  | OfflineGuard, reconnect removed         |  |
        |  $DATA/state/requests/*.json  <-- writes -----------|  | address cache (targets.json)            |  |
        |  $DATA/sessions/<id>/status   <-- heartbeat --------|  | home MidHook, cmd bridge, title suffix  |  |
        +---- watches / polls --------------------------------|  +-----------------------------------------+  |
                                                              +-----------------------------------------------+
  Fallback only: SwoftyHost.exe (create suspended + inject).
  Game data files are never written in VFS mode; version.dll and swofty/ are removed on exit.
```

`$DATA` is `$XDG_DATA_HOME/swofty` (`~/.local/share/swofty`) on Linux, `%LOCALAPPDATA%\Swofty` on Windows, and `~/.var/app/com.valvesoftware.Steam/data/swofty` under Flatpak Steam, so the files stay visible inside the sandbox.

**Components**

| Component | Form | Responsibility |
|---|---|---|
| `swofty-formats` | Rust lib | GVDB, ERP, `!pkg`, BXML and texarray read/write, held byte-identical to the Python oracle where one exists. |
| `swofty-core` | Rust lib | Store, resolver, patch engine, overlay builder, baseline and journal, Steam/Proton discovery (`libraryfolders.vdf`, `appmanifest_3059520.acf`, `localconfig.vdf`, compat-tool mapping), launch-option and prefix-registry wiring, session placement and `session.json`. No UI dependencies. |
| `swofty-cli` | Rust bin `swofty` | `run` (the Steam wrapper), `build`, `verify`, `restore`, `mods …`, `profile …`, `conflicts`, `doctor`. No GPUI dependency, so it runs headless (Steam wrapper, Steam Deck Game Mode). |
| `swofty-launcher` | Rust bin, GPUI + `gpui-component` | A thin shell over core with a Zed-like look: library, profiles, conflicts, setup wizard, Play, session banner and history. |
| `version.dll` | C, `native/src/loader/` | Stage-0 proxy: forwarding, host check, gate, wait for the first window, then load `swofty/mods/core/*.dll` and `swofty/mods/*.dll`, calling `SwoftyModInit(const SwoftyApi*)`. Logs to `swofty/loader.log`. |
| `swofty_core.dll` | C++23 + safetyhook, `native/src/core/` | Native side of the built-in `swofty.core` mod: VFS and save redirect, OfflineGuard, per-build address cache, home-tile `MidHook`, command/notify bridge, window-title suffix. |
| `swofty.core` data | generated by `swofty-core` | F9 prompt, bottom-left panel, settings screen, `mod_menu` route and tooltip entries. Built in; cannot be disabled. |
| `SwoftyHost.exe` | C++ | Fallback create-suspended + inject, only if the proxy can't be used. |

**Launcher ↔ loader channel.** File-based, because files are the only channel that crosses the Wine boundary and works while the launcher isn't running:

| File | Writer → reader | Purpose |
|---|---|---|
| `sessions/<id>/session.json` | core → loader | Read-only for the loader. |
| `state/requests/<profile>.json` | loader → core | In-game changes; atomic write-and-rename. |
| `sessions/<id>/status.json` | loader → launcher | 2-second heartbeat: phase, hook table, errors. |
| `sessions/<id>/loader.log`, `crash/` | loader, copied by post-run | Logs and minidumps. |

An optional 127.0.0.1 JSON-lines socket (random port, token from the session) can carry live status later; nothing depends on it.

---

## 4. Launch flows

### 4.1 Common: building the session (core, on the host OS)

1. Take a single-instance lock. Refuse if `F1_25.exe` or any `EAAntiCheat.*` process is running (the `modswap.py` pgrep guard, generalised).
2. Identify the build: appmanifest `buildid` plus exe sha256, cached by size and mtime. An unknown build triggers a re-baseline (§10).
3. Resolve the profile: enabled mods, load order, option values. Confirm the user has consented to each native-plugin DLL hash.
4. Compute `buildhash` from the base build, the ordered `(mod id, version, content hash)` list and the options. On a cache hit, reuse `cache/overlay/<buildhash>`; on a miss, run the patcher (§7, §8).
5. Write `sessions/<id>/session.json`:

```json
{
  "schema": 1,
  "ticket": "b64-random-32-bytes",
  "expires": "2026-10-17T12:34:56Z",
  "mode": "offline-modded",
  "game": { "app_id": 3059520, "build": "25251167", "exe_sha256": "d873a1c8…" },
  "profile": "grid24",
  "overlay": { "root": "Z:\\home\\swofty\\.local\\share\\swofty\\cache\\overlay\\9f3c…",
               "files": { "game_package/gamedatabases/gamedatabases.2025.bin": "gdb.bin",
                          "2025_asset_groups/ui_package/flow_frontend.erp": "ui/flow_frontend.erp" } },
  "plugins": [ { "id": "swofty.core", "dll": "swofty/mods/core/swofty_core.dll", "sha256": "…" } ],
  "options": { "swofty.f1-2026-grid24": {} },
  "ui_bridge": { "notify_map": { "3404951093": ["swofty.core", "open_mod_settings", true] } },
  "policy": { "offline": "strict", "save_namespace": "swofty-grid24" }
}
```

Paths are given in the process's view; under Proton `/home/...` becomes `Z:\home\...`.

### 4.2 Linux (Steam + Proton, Steam Deck, NixOS, Flatpak Steam)

**One-time setup.** The launcher discovers the Steam roots (native, `~/.steam/steam`, Flatpak), the libraries, the install, the compatdata and the configured Proton. It shows the launch option to paste:

```
~/.local/share/swofty/bin/swofty run -- %command%
```

It detects the option in `localconfig.vdf` and can write it itself while Steam is closed. The user's affinity and environment move into `config.json`:

```json
"launch": { "cpu_affinity": "0-11,14-31", "env": { "VKD3D_SHADER_CACHE_PATH": "0" } }
```

The wrapper applies them with `sched_setaffinity` before spawning, and children inherit them. NixOS works because Steam runs in an FHS environment and the wrapper is a normal glibc binary. The Linux binaries are built against an older glibc baseline so they also run on SteamOS.

**Play** (Steam's button, or the launcher runs `steam -applaunch 3059520` after writing a short-lived `launch_request.json`):

1. Steam runs `swofty run -- <SLR/reaper entry> … proton waitforexitandrun … <anticheat-launcher target>`. The exact shape is confirmed in M0.
2. The wrapper parses argv: the Proton verb, the SLR entry, the environment and the prefix. It drops the anticheat-launcher target and substitutes `F1_25.exe`. **If the argv shape isn't recognised, it fails closed:** it shows an error through `notify-send`/zenity when present, logs, and exits without launching.
3. It runs §4.1, then places `version.dll` and `swofty/` in the game folder (journaled) and writes the prefix-scoped override into `compatdata/<appid>/pfx/user.reg`. The registry key survives the wrapper dropping environment variables; it also sets `WINEDLLOVERRIDES="version=n,b"` as a second route.
4. It launches the Proton chain on `F1_25.exe` directly, through the SLR, with the game folder as cwd. Proton, the prefix, the Steam overlay, ownership and `SteamAppId`/`SteamGameId` are unchanged.
5. In the process, `version.dll` loads first, forwards the real exports, checks the host, runs the gate, waits for the first top-level window, then loads `swofty/mods/` (VFS timing: §12).
6. The wrapper waits and forwards signals. It does not `exec`, so it can run the post-run steps (§4.4).

**Defaults on Linux.** Steam's Play means offline-modded with the default profile. Online is impossible under Proton regardless.

**Steam Deck.** Same flow. The GUI runs in desktop mode; in Game Mode the wrapper builds headlessly and MOD SETTINGS is reachable with R3.

**Other Wine setups** (Steam for Windows inside Lutris, Heroic or Bottles) are best effort: `swofty run --wine <bin> --prefix <dir> --game <path>`. The Windows Steam client must be running in that prefix.

### 4.3 Windows

**Primary.** The launcher's "Play modded (offline)" button runs §4.1, places `version.dll` and `swofty/` (journaled), sets `SteamAppId`/`SteamGameId` to 3059520 (Steam must be running), and starts `F1_25.exe` directly. It does not use `steam -applaunch`, because Steam's launch command runs the EA AntiCheat launcher, which flags unsigned modules (`exe.md` §2.2). Steam's launch options are left alone, so Steam's Play button stays vanilla, online and anticheat-on, and because the proxy is removed on exit, that path never sees it.

**Contingency, only if M0 test T2 shows a Steam self-relaunch.** Use `swofty.exe run -- %command%` with vanilla passthrough as the default. With no fresh ticket, the wrapper checks that the game folder is clean (no proxy) and runs Steam's original command unchanged. With a ticket, it runs the modded flow. Windows Steam's support for `%command%` wrappers is confirmed in M0 (T3).

### 4.4 Shared post-run and the relaunch loop

When `F1_25.exe` exits, the wrapper or launcher:
- records the outcome (ok, crashed or killed) in the session status;
- copies `swofty/loader.log` and any minidumps into `sessions/<id>/`;
- runs the journal restore: removes `version.dll` and `swofty/`, clears the prefix override, and in in-place mode restores the swapped data files (§9.1), leaving the game folder byte-identical to Steam's copy;
- merges `state/requests/<profile>.json` into the profile.

If the loader set `relaunch: true` (in-game "Apply & restart"), it rebuilds, re-places the loader and relaunches without returning to Steam, which keeps showing the game as running.

### 4.5 Visible state

- **Launcher:** a fixed banner while a session is live: "MODDED SESSION · OFFLINE · anticheat not loaded · profile X". Session history shows crashes and logs.
- **Game window:** the title gets the suffix " [Swofty Mod · Offline]", via a native hook on `CreateWindowExW`/`SetWindowTextW`.
- **In game (§11):** the permanent bottom-left panel, the F9 prompt, the MOD SETTINGS tile under Career, and a settings-screen header ("Profile X · N mods · Offline").

### 4.6 Running without the anticheat

The requirement is a safety constraint, not a capability: **mods are active only in a session that is already running without the anticheat, and the launcher must never produce a modded online session.** The design meets it by relying on conditions that already hold, not by defeating the anticheat service.

- **Linux / Proton (the user's platform).** There is no anticheat path: the EA Javelin preloader refuses to run under Wine, Proton and Steam Deck (§2.3), and the game reaches its own offline front end. The launcher does not interact with the anticheat. The loader only confirms, inside the process, that no `EAAntiCheat.*` module or service connection is present before doing anything.
- **Windows.** The modded entry point and Steam's vanilla Play button are physically separate. The vanilla button keeps the anticheat and online exactly as shipped, and the launcher never touches it. The modded path is taken only when the user chooses "Play modded (offline)", and the loader files are removed afterwards.

**Out of scope, not designed:** any technique whose purpose is to make the anticheat report a false state, to run mods while the anticheat or online services are active, or otherwise to evade detection online. Determining the exe's behaviour when no anticheat service is present belongs to the exe and native-loader work (M0 test T1). Whatever it finds, the loader's policy module only (a) verifies that the session is already anticheat-free and offline, and (b) refuses to load mods otherwise.

**The loader's offline guard.** Independently of how the session started, the loader enforces offline inside the process:
- The "Retry connection" reconnect action is neutralised in the front-end data (§11).
- Outbound connections from the game process to EA and online endpoints are blocked at the WinHTTP/WS2_32 layer, while Steam's local ownership pipe is left intact. This restricts the session to local play; it is not an evasion.
- If the loader ever detects that the session is not offline, or that an anticheat module is present, it unloads all mods, stops the VFS, writes an error to the session status, and lets the game continue vanilla (or exits) rather than continuing modded.

---

## 5. Mod format and manifest

A mod is a directory or a `.swoftymod` zip:

```
<mod-id>/
  swofty.json          manifest
  db/*.json            database operation sets
  erp/...              payload files referenced by ERP ops
  ui/*.pkgpatch.json   !pkg node-graph patches
  files/...            loose files
  native/*.dll         native plugins (optional)
  assets/, icon.png, README.md
```

### 5.1 Manifest schema (JSON example)

```json
{
  "schema_version": 1,
  "id": "swofty.f1-2026-grid24",
  "name": "F1 2026 — 24-Car Grid",
  "version": "1.0.0",
  "author": "swofty",
  "kind": ["database"],

  "compat": { "game": "f1_25", "game_builds": [">=25251167"], "launcher": ">=0.3.0",
              "seasons": ["f1_2026"] },

  "requires": [{ "id": "swofty.core", "version": ">=0.3.0" }],
  "conflicts": [{ "id": "*", "touches": "db:entrants.entrant_lists.entrant_list_34.*" }],
  "load_after": ["swofty.track.*"],

  "options": [
    { "id": "team_livery", "type": "enum", "default": "titlesponsor",
      "values": ["titlesponsor", "tier01", "replica"], "binds": "db" }
  ],

  "operations": {
    "database": ["db/grid24.json"],
    "erp": [],
    "files": [],
    "ui": [],
    "native": []
  }
}
```

Notes:
- `kind` is a filter; `operations` is authoritative.
- `foreach` with a game-profile enumerator (`@game.tracks.f1_2026`) expands one ERP op across all 28 tracks. The per-track world/layer route (copy `vehicles_current_myteam.layer` into each `_f1_26.erp` and list it in the World) is the fallback if the DB world switch ever misbehaves; it is built for Bahrain as `A-erp`/`B-erp` in `work/experiments/grid24/`.
- `binds: "db"` options re-run the DB merge and need an apply-and-restart; `binds: "native"` options are read live by the plugin.
- `compat.game_builds` ranges over the Steam `buildid`.
- Native capabilities are declared and shown in the consent dialog. grid24 has no native ops if the B24 test passes (§2.2).

### 5.2 Database operations (named paths, selectors, dataset scoping)

The grid24 DB ops, using variant B24's rows (`work/experiments/grid24/build/build.py`, `build_b24.py`):

```json
{
  "schema_version": 1,
  "ops": [
    { "op": "set", "select": "entrants.entrant_setup_modes.entrant_setup_mode[_metadata_=f1_26]",
      "field": "max_entrants", "value": "24" },
    { "op": "set", "select": "entrants.entrant_setup_modes.entrant_setup_mode[_metadata_=f1_generic_2026]",
      "field": "max_entrants", "value": "24" },

    { "op": "set", "select": "{446668bc25ba571c}[_metadata_=f1_26]", "field": "worldEri",
      "value_from": "{446668bc25ba571c}[_metadata_=myteam_26].worldEri" },
    { "op": "set", "select": "{446668bc25ba571c}[_metadata_=f1_26]", "field": "worldAssetGroup",
      "value_from": "{446668bc25ba571c}[_metadata_=myteam_26].worldAssetGroup" },

    { "op": "add-row", "table": "entrants.teams",
      "from": "entrants.teams.team[_metadata_=gm_cadillac_26]",
      "set": { "_metadata_": "team12_26", "display_name": "lng_custom_team_26",
               "display_name_code": "T12", "stat_id": "500", "primary_colour": "-16735845" } },
    { "op": "add-row", "table": "entrants.vehicles",
      "from": "entrants.vehicles.vehicle[_metadata_=myteam_honda_2026]",
      "set": { "_metadata_": "team12_2026", "stat_id": "509" } },

    { "op": "add-row", "table": "entrants.entrant_lists.entrant_list_34",
      "from": "entrants.entrant_lists.entrant_list_34.entrant_20",
      "set": { "_metadata_": "team12_driver_01", "team": "team12_26", "vehicle": "team12_2026",
               "entrant_reference": "jack_tremblay", "car_number": "66",
               "vehicle_customisation": "jack_tremblay_2026" } },
    { "op": "add-row", "table": "entrants.entrant_lists.entrant_list_34",
      "from": "entrants.entrant_lists.entrant_list_34.entrant_21",
      "set": { "_metadata_": "team12_driver_02", "team": "team12_26", "vehicle": "team12_2026",
               "entrant_reference": "rashid_nair", "car_number": "65",
               "vehicle_customisation": "jack_tremblay_2026" } }
  ]
}
```

**Row-count rule.** Every table keeps its row count in `<container>._<row>_instancecount_` (for example `entrants.teams._team_instancecount_`), and the game reads only rows `0..count-1`; a row past the count is silently ignored (`grid24-cap.md` §1). Therefore:
- `add-row` appends. The new row takes index = the table's current count, in load order, and the count is incremented in the same op. On the pristine DB the ops above produce `team_358`, `vehicle_413` and `entrant_22/23`, and set the counts to 359, 414 and 24.
- `remove-row` keeps rows contiguous: later rows move down one index and the count is decremented.
- `add-row` and `remove-row` need the table's real path to compute the count key. A table known only by its `{hash}` placeholder accepts `set`, not row ops.

Other rules:
- Ops: `set`, `add-row`, `remove-row`, `list-append`, `list-remove` (GVDB list values, type 13), and `track-package` (embeds an EGO manifest, §6).
- `select` and `table` use `[field=value]` on `_metadata_` or any field. Selectors resolve to concrete 64-bit keys when the op is applied, so a game update that renumbers rows doesn't break the mod. A table whose path string is still unknown is named by its `{hash}` placeholder, as in `gamedb.json`.
- `from` clones an existing row. Type tags are copied field by field, never guessed (`gamedb.md`).
- `value_from` copies a value from another resolved path.
- `datasets` scopes a value the way EGO track overrides do; absent means all datasets.
- Two mods adding rows with the same `_metadata_` label to one table is a conflict.
- The team logo reuses Cadillac's key (`gm_cadillac_26`). Logo arrays are UIDB data in `overrides.uidb` with baked texture arrays (`ui.md` §6), not GVDB rows, so a unique logo needs a `pkg-patch` plus a texarray slice (Q10).

### 5.3 ERP and `!pkg` patch representation

ERP ops: `replace-resource`, `copy-resource` (a resource and its fragments between ERPs; the per-track world/layer move), `add-resource`, `remove-resource`, `pkg-patch`, plus loose files and `add-mipmaps`.

A `!pkg` patch is a declarative transform over the node graph (the `pkg.py`/`uinode.py` model), not a byte diff, so it survives the resource moving inside its ERP. The F9 prompt and panel edits from `build_home_tile_mod.py` v4, expressed this way:

```json
{
  "schema_version": 1,
  "root": "Screens[0]:fe_home",
  "edits": [
    { "op": "clone-node", "src": "settings", "as": "mod_menu_listener",
      "set": { "action_name": "Menu Right Stick Click" },
      "replace_event": { "event": "pressed",
        "signals": [ { "to": "fe_home.Action", "pin": "route", "attrs": { "key": "mod_menu" } },
                     { "to": "fe_home.audio_theme_trigger", "pin": "confirm" } ] } },
    { "op": "clone-node", "src": "tooltips.settings", "as": "tooltips.mod_menu",
      "set": { "tooltipsName": "mod_menu" },
      "set_text": { "node": "text", "value": "MOD SETTINGS", "localise": false } },
    { "op": "set-text", "node": "contextual_panels.offline_contextual_panel.subtitle",
      "value": "Launched with Swofty Mod Launcher, press", "localise": false },
    { "op": "set-text", "node": "contextual_panels.offline_contextual_panel.tooltip_label",
      "value": "to open settings", "localise": false },
    { "op": "set-props", "node": "contextual_panels.offline_contextual_panel.tooltip_extra_2",
      "set": { "tooltipsName": "mod_menu_offline" } }
  ]
}
```

The same patch also carries v4's two signal edits in `contextual_decision` (§11). The companion edits are a `mod_menu` route in `routes_frontend_general.uir` (`persistent.erp`) and the `mod_menu` and `mod_menu_offline` entries in `tool_tips.uidb` (`databases.erp`).

The engine resolves node paths, recomputes djb2 ids, keeps `!idi` keys hash-sorted, writes breadth-first and recomputes ERP fragment MD5s, as the Python tools do. **The byte-identity oracle is a unit test:** an empty edit list must reproduce the original ERP byte for byte.

**Routing ops to files.** The patcher groups ops by target file, applies them in load order over the baseline copy, writes the result to `cache/overlay/<buildhash>/`, and records it in the overlay map. The GVDB and the shared ERPs (`flow_frontend.erp`, `persistent.erp`, `databases.erp`, each track's `_f1_26.erp`) are the merge hot spots (§7).

---

## 6. Track-package interoperability (EGO format)

- **Import.** The launcher reads an EGO track zip (`manifest.json` schema_version 1, `<code>.erp`, optional `mipmaps/`, `ui/{images,minimap}.json`) and presents it as a Swofty mod without repacking. The adapter maps:
  - `activation.copy_slot` / `copy_from_track` → `add-row`/clone ops on the track tables and per-dataset track rows, keyed by `_metadata_`/`track`;
  - `activation.overrides` and `track_overrides` → `set` ops (these are GVDB dotted paths with `path`/`value`/`datasets`);
  - `ers` → `set` ops;
  - `copy_directories` → loose-file `add` ops;
  - `ui/*.json` → recorded for the UI/texture stage.

  The `<code>.erp` is mounted into the overlay as-is.
- **Export.** ModKit can emit a Swofty track mod as an EGO-compatible zip, within the public format only.

This satisfies "installed as-is": the ERP is never rebuilt, and only the public JSON is interpreted.

---

## 7. Database merge algorithm

The GVDB is one flat FNV-1a-64-keyed record list with a continuous add-mask (`gamedb.md`). The merge runs over records, never raw bytes, then re-emits with the proven writer (re-sort by `key_hi`, rewrite `count`, re-mask the whole stream).

```
INPUT:  baseline records B (keyed by 64-bit FNV path hash)
        ordered mods M1..Mn, each an op list

1. index = { key -> record } from B                         // pristine
2. layers = []                                              // provenance per key
3. for mod in M1..Mn (load order):
     for op in mod.ops:
       resolve selectors against index (as merged so far)
       materialise to concrete (key, type, value) writes:
         set         -> write existing key (error unless op.create)
         add-row     -> n = index[<container>._<row>_instancecount_]
                        for each field: key=fnv(<container>.<row>_<n>.<field>); type from the `from` sibling
                        index[<container>._<row>_instancecount_] = n + 1
         list-append -> read current list, append token if absent, rewrite
         remove-row  -> delete the row's keys, move later rows down one, count = count - 1
       record (mod, op, prev.value, new.value) in layers[key]
4. conflict pass:
     key with >1 distinct final value across mods -> CONFLICT(key, contributors, before/after)
     same value from two mods                     -> harmless duplicate
     two mods add rows with one _metadata_ label  -> CONFLICT
     list ops on the same list                    -> MERGE (ordered union of appends)
5. policy: hard conflict -> fail build + report; soft (list) -> auto-merge; explicit profile `wins` -> honour
6. size guard: estimate new file size; warn as it approaches the GVDB load-buffer cap (Q6)
7. build: index -> records; write() with sort+remask
          assert untouched keys == baseline
          assert every touched table: _instancecount_ == row count, rows 0..count-1 all present
          checksum into overlay
```

Properties:
- **Deterministic.** The same baseline and the same ordered ops give identical bytes.
- **Last writer wins per key, with provenance,** so the conflict report names both mods and shows both values. List fields merge as an ordered union.
- **Row counts are an invariant.** Every table's `_instancecount_` equals its contiguous row count; the build fails otherwise. Variants A and B broke this, and the game ignored their new rows (§2.2).
- **Row-add safety.** New keys keep `key_hi` sorted with stable ties, matching the game's binary-search-then-check-lo lookup. Type tags are copied from siblings.
- **Selectors resolve against the merged index in load order,** so index renumbering across updates, and rows appended by earlier mods, don't break ops.
- **No raw-byte merge, ever.**

**grid24 in merge terms:** two `max_entrants` sets, two world-row sets via `value_from`, and `add-row`s for the team, the car and two entrants, each bumping its table's count (§5.2). That is variant B24, built and verified (§2.2); whether it gives 24 cars in game is the M6a test.

---

## 8. Overlay build and the baseline

- **Baseline** is Steam's pristine copy. Core keeps a per-build `baseline.json` of (path, size, sha256) for every file any mod could touch, plus a content-addressed store of the original bytes of those files only, fetched lazily the first time a mod names them. This generalises `modswap.py`'s pristine-verify pattern.
- **Overlay build** writes only modified files into `cache/overlay/<buildhash>/`, keyed so identical profiles reuse the cache, with per-file provenance and checksums.
- **Verification** re-reads each overlay file through its format parser and checks invariants (GVDB untouched keys round-trip and every table's `_instancecount_` matches its rows, ERP hashes recompute, `!pkg` re-serialises) before launch. A failure fails the build.

---

## 9. Non-destructive operation, backup and restore

### 9.1 Two data-delivery modes, chosen by capability

1. **VFS override (preferred for mod data).** `swofty_core.dll` redirects the game's file opens (`NtCreateFile`/`NtOpenFile`) to the overlay for any path in `session.json.overlay.files`. The game's data files are never written, "verify files" is satisfied, enable and disable are instant, and crash recovery is trivial. Under Proton the hook runs inside the Windows process, before Proton translates to host syscalls, so it never touches host files. *Open:* path canonicalisation under Proton, and installing the hooks before the game's first data-file open (§12). Validated in M1.
2. **Journaled apply/restore (fallback and degraded no-VFS mode).** Copy overlay files over the originals before launch and restore them after exit, as easyerpmod and `modswap.py` do, hardened with a transactional journal. Before touching a file, record `(path, original sha256, backup location)` in `state/journal/<id>.json` and stash the original in the content store. Any open journal is rolled back on the next launch or by `swofty restore`. Steam's "verify files" also restores, and because the baseline hashes are known, core detects that and re-applies.

Until M1 proves the VFS, the journal is the safe default; it already works via `modswap.py`.

**The loader files (`version.dll`, `swofty/`) are always delivered by the journal, never by the VFS,** because the OS loader must find `version.dll` on disk before any of our code runs. The journal guarantees they are removed even after a crash.

### 9.2 Save-game isolation

Saves sync through Steam Cloud, so the loader namespaces them per profile by redirecting the save directory (VFS, or a `UserDataFolder` redirect) to `$DATA/saves/<profile>/`, which Steam Cloud does not sync. Vanilla saves stay pristine and syncable; modded saves stay local and labelled. This is a safety feature, not just hygiene.

### 9.3 Clean restore and uninstall

`swofty restore` rolls back any open journal (including a proxy left behind by a crash) and verifies every baseline file. Uninstalling the launcher: run `swofty restore`, then delete `$DATA`, the Steam launch option and the prefix override.

### 9.4 Profiles, baseline and journal guarantees

- **Vanilla is a built-in profile.** `vanilla` cannot be edited. Launching it places no `version.dll`, no overlay files and no prefix override; it is identical to Steam's own Play.
- **Per-profile state.** A profile owns its mods, maps, load order and per-mod options, its own build cache `$DATA/cache/overlay/<profile>/<buildhash>/`, and its own save namespace: saves are redirected to `$DATA/saves/<profile>/` and never touch vanilla saves or Steam Cloud.
- **Baseline store.** Before the first change to any game file, the original is stored in `$DATA/baseline/objects/<sha256>`. The per-build record `$DATA/baseline/<build_id>/baseline.json` lists each file's path, size, sha256 and source: `depot` when verified against Steam's depot manifest hashes, otherwise `first-sight`.
- **Journal.** Every mutation is written to `$DATA/state/journal/<id>.json` before it happens and marked done after: replaced game files, created files (`version.dll`, `swofty/mods/...`), created directories, and the Wine prefix override in `user.reg`. On every launcher start and before every launch, any open journal is rolled back first, in reverse order, so a crash or kill mid-apply is undone by the next start.
- **Restore, verify, doctor.** `swofty restore` rolls back every open journal, removes every addition, restores every baselined file and verifies the folder is byte-identical to the baseline; it lists anything it could not restore and exits non-zero. `swofty verify` and `swofty doctor` report every drift from the baseline, ours or not: changed files, additions, the override, open journals, and a build that differs from the baseline.
- **Updates.** A new `buildid` or exe sha256 gets a new baseline taken from the updated files. Originals from an old build are never restored over a new build; a file whose hash is neither the baseline's nor what the launcher wrote is left alone and reported.
- **Steam verify** stays a valid last resort: it restores the data files but does not remove `version.dll` or `swofty/`; `swofty restore` does.
- **Delivery today and later.** Data files are delivered in journalled in-place mode today. The VFS mode, later, writes no data files to the game folder at all.

---

## 10. Update resilience

- **Detection.** Core watches the `appmanifest_3059520.acf` `buildid` and the exe hash. A change invalidates the overlay cache and the per-build address cache, and flags mods whose `compat.game_builds` excludes the new build.
- **Data mods** are build-agnostic: paths and selectors resolve against the new baseline. After an update, core re-baselines lazily, re-resolves selectors, rebuilds, and reports any selector that no longer resolves.
- **Code mods** never hard-code addresses. Every hook is resolved by **signature scan** against the loaded image, with single-match enforcement, and the result is cached per build in `targets.json`; with no ASLR, the cache is valid between runs of the same build. `exe.md` §3.4 supplies single-match signatures for the home anchor and its helpers; the RE program keeps them in `re/db/signatures.json`. A signature that no longer matches disables that hook and surfaces it in status instead of crashing. On a new build, every signature is re-checked and runtime-confirmed (L3) items drop to L2 until re-traced (`re/PROTOCOL.md` §8).
- **Re-apply after a patch** is automatic on the next launch. Nothing modded persists in the game folder across an update: the loader files exist only during a session, and in-place data is restored by the journal.

---

## 11. In-game MOD SETTINGS design

**A. Entry points.**
- **F9 prompt "MOD SETTINGS" (data).** A `UIKL` listener on `Menu Right Stick Click` (F9 on keyboard, R3 on controller) fires route `mod_menu`, a clone of `options` in `routes_frontend_general.uir`. A `UITL` prompt uses a new `mod_menu` entry in `tool_tips.uidb` with literal `localise:false` text (`build_home_tile_mod.py` v2/v4).
- **Bottom-left panel (data, v4).** In `fe_home.contextual_panels.offline_contextual_panel`: `subtitle` becomes the literal "Launched with Swofty Mod Launcher, press"; `tooltip_label` becomes "to open settings"; `tooltip_extra_2` points at a new `mod_menu_offline` tooltip entry, which supplies the device-correct F9/R3 glyph (glyphs come only from the tooltip path, `ui.md` §4b). In `contextual_decision`, the `offline` branch no longer enables `extra_2_listener`, so Extra2 can't trigger a reconnect from a modded session (§4.6), and the `default` branch also shows the panel. It renders as "Launched with Swofty Mod Launcher, press [F9] TO OPEN SETTINGS".
- **Home tile directly under Career (native).** A single safetyhook `MidHook` at `HOME_AFTER_CAREER` (0x141742fd6), right after the Career adder in the shared builder 0x14173df80. Per `exe.md` §3.4 it calls the game's own Career adder for `careerIdx+1`, re-types the pushed item's vtable (slot +0x20 populate → literal "MOD SETTINGS" with `title_localised=false`; slot +0x28 getAction → `{type 1, "mod_menu"}`), sets the item id to `mod_menu`, and increments `r12` and `r15` so the later tiles shift down by one and stay consistent. Pressing it fires the `mod_menu` route. The v3 static tile is dropped so the tile isn't duplicated. Fallback if the hook isn't ready: the v3 static tile, which renders at the bottom of the tile list rather than under Career (`ui.md` §4b).

**B. The settings screen (data plus a small native bridge).**
- The screen is a generated `.ui` added to `flow_frontend.erp` or a `screens/*.erp`, registered through `flows.ui`, `region_loading.uiconfig` and the `mod_menu` route. It is driven by a `swofty_mod.uidb` that `swofty.core` generates from `session.json`: the mod list (name, version, enabled) and per-mod options. Pure data covers listing, versions, option display and navigation. Adding a resource to a loaded ERP is untested, and the asset-group mapping isn't visible on disk (`ui.md` §5); until that works, `mod_menu` opens the game's Settings, as in v2–v4.
- **Writing a change needs the native bridge,** because data can't persist state or signal the launcher. A toggle or option fires a `UINT NotifyName=swofty.set` (or a `raise_command`). `swofty_core.dll` hooks the command/notify dispatch (near `FEHomeViewModel::onCommandRaised`, 0x141bb3c20) and:
  - for `binds: native` options, updates in-memory plugin state immediately;
  - for `binds: db` options and enable/disable, records the change in `state/requests/<profile>.json` and offers "Apply & restart", which sets `relaunch: true` (§4.4).
- The header shows "Profile X · N mods · Offline" from the UIDB.
- `ui_bridge.notify_map` in `session.json` maps each generated control's djb2 id to `(mod, option, value)`, so the native handler needs no hard-coding.

---

## 12. Native loader design

**Primary route:** a C `version.dll` proxy plus a C++23 `swofty_core.dll` with safetyhook, in `native/`.

- **`version.dll` (stage 0, `native/src/loader/version.c`).** At `DLL_PROCESS_ATTACH` it loads the real `version.dll` from the system directory and forwards all 17 exports through jump stubs resolved with `GetProcAddress` (not absolute-path `.def` forwarders, which Wine rejects). If the host isn't `F1_25.exe`, it stops there. Otherwise it checks the exe build (size, PE timestamp), runs the gate, waits for the game's first visible top-level window (then a 2 s settle), and loads `swofty/mods/core/*.dll` and then `swofty/mods/*.dll` in sorted order, calling each one's `SwoftyModInit(const SwoftyApi*)`. Plugin DLLs must match a content hash pinned in `session.json`; unpinned ones are refused.
- **Gate.** `SWOFTY_SESSION` names the session; the ticket and expiry, the exe hash, and the absence of any `EAAntiCheat.*` module or connection are checked. On any failure the stage-0 loads nothing, logs, and the game runs vanilla.
- **`swofty_core.dll` (`native/src/core/swofty_core.cpp`)**, initialised in order:
  1. **Resolver:** signature scan over `.xtls` with single-match enforcement; `targets.json` per build.
  2. **VFS:** `NtCreateFile`/`NtOpenFile` redirection, including the save redirect (§9.2).
  3. **OfflineGuard** (§4.6).
  4. **Swofty Mod:** the home-tile `MidHook`, the command/notify bridge, the window-title suffix.
- **VFS timing (open; decided in M1).** Stage 0 loads mods only after the first window, but the VFS must be active before the game opens any file it redirects. Either stage 0 installs the file hooks at attach (they are not hooks on game code), or the core loads early and defers only its code hooks.
- **`SwoftyApi`** (`packages/sdk/include/swofty_api.h`; version 1 has `log`, `module_base` and `scan`). The design extends it with `resolve_rel32`/`resolve_riprel`, `find_string`/`find_xref_lea`, safetyhook `InlineHook`/`MidHook`, `read_game_db`, `get_option`, `on_frame`, and a small game-API layer (NeUIString create, VM bool/string set) from `exe.md` §3.4's signatures. Capabilities declared in the manifest gate what each plugin may call.
- **Rule from `exe.md`:** hook only `.xtls`; call but never hook `.data1` bodies.

**Hook targets**

| Purpose | Target |
|---|---|
| Home tile | `MidHook` at `HOME_AFTER_CAREER` 0x141742fd6 (signature match + 0xF, `exe.md` §3.4) |
| Settings bridge | `FEHomeViewModel::onCommandRaised` 0x141bb3c20 |
| Window title | `CreateWindowExW` / `SetWindowTextW` |
| 24-car grid | None expected (DB-only, §2.2). Fallback site if ever needed: the selector's limit load at 0x1424a30a8 |
| 26-car grid | Whatever fixed-size per-car pools the car-count audits find (§14.1) |

**Hook gates (`re/PROTOCOL.md` §2).** A trace-only hook needs an L1 target and a passing signature. A behaviour hook needs every function and field it touches at L2, plus agreement from a second, independent verifier. A hook is enabled by default in the launcher only when all its items are L3 on the shipping build and its runtime test has passed.

**Status of `native/`** (`native/TEST.md`). Stage 0 implements forwarding, the host and build checks, the window wait, mod loading and the version-1 API. `swofty_core.dll` implements the home-tile `MidHook`, checked against a synthetic `F1_25.exe` under Wine (`native/test/run_wine_test.sh`). The gate, VFS, OfflineGuard and bridge are not written yet.

**Toolchain.** `native/build.sh` cross-builds from Linux with `zig cc` and `zig c++ -std=c++23` for `x86_64-windows-gnu`, statically linked, with safetyhook and Zydis vendored in `native/third_party/safetyhook/`. The build is deterministic.

**Assumptions and fallbacks**
- A1. `version.dll` loads and forwards correctly on Windows and under the prefix-scoped Proton override. *Fallback:* `dinput8.dll` (one used export, but on the input path), or `SwoftyHost.exe` create-suspended + inject.
- A2. In-process file redirection (VFS) is viable. *Fallback:* journaled apply/restore (§9), already proven via `modswap.py`.
- A3. `.xtls` code hooks installed after the first window don't trip Denuvo. *Evidence:* no code-section self-checksumming found; the staged runtime test in `exe.md` §2.3 (M0) decides.
- A4. No ASLR (confirmed), so the per-build address cache is valid.

**Why C++ now, Rust maybe later.** safetyhook (MIT) gives a `MidHook` with a full register context and automatic instruction relocation, which the home tile depends on, and `native/` already builds with it. A Rust `cdylib` loader is feasible but would re-implement a working component without an equally mature mid-hook story. Keep the C++ loader; revisit a Rust port if the plugin SDK should become Rust-first. The loader is thin and sits behind the `SwoftyApi` C ABI, so a later port is low-risk.

---

## 13. Repository layout and language choices

```
f1-modding/
  Cargo.toml                 Rust workspace
  crates/
    swofty-formats/          GVDB, ERP, !pkg, BXML, texarray
    swofty-core/             store, resolver, patcher, overlay, baseline + journal, Steam/Proton, session
    swofty-cli/              bin `swofty`: run, build, verify, restore, mods, profile, conflicts, doctor
    swofty-launcher/         GPUI app + gpui-component
    swofty-sdk/              shared Rust types (manifest, session schema)
  native/
    src/loader/              version.dll stage-0 proxy (C)
    src/core/                swofty_core.dll (C++23, safetyhook)
    include/swofty_api.h     plugin C ABI
    third_party/safetyhook/  safetyhook + Zydis, vendored
    test/                    Wine test with a synthetic F1_25.exe
    build.sh                 zig cross-build
  mods/
    swofty.core/             built-in data mod: F9 prompt, panel, settings screen, mod_menu route + tooltips
    swofty.f1-2026-grid24/   showcase: DB rows (B24)
  gameprofiles/
    f1_25.json               tracks, dataset names, table-hash aliases, signatures, file lists, caps
  tools/                     Python toolkit: research and byte-identity oracle (not shipped)
  re/                        RE protocol, knowledge base, tools, trace plugin, tests, traces (§14.1)
  research/                  research notes
  tests/                     round-trip corpora, golden overlays, Rust-vs-Python cross-checks
  docs/                      this design, manifest schema, author guide
```

`SwoftyHost.exe` (fallback) is not written yet; it would sit under `native/src/host/`.

**Language rationale**
- **Rust for all shipped host-side code** gives one toolchain, single static binaries with no runtime to install, easy builds of the Linux wrapper against an old glibc baseline for SteamOS, and GPUI for the Zed-like GUI.
- **The Python tools become the oracle.** CI runs `swofty-formats` and the Python tools over the same corpus (every `.ui`, the GVDB, sampled ERPs) and asserts byte-identical output. GVDB ports from `tools/gamedb.py`, ERP from `tools/erp.py`/`erpw.py`, and `!pkg` from `tools/pkg.py`/`uinode.py`. BXML ports from `work/grid/bxml.py`, which only decodes, so a BXML writer has no oracle. Texarray has no implementation yet; its writer is the open gap in Q10.
- **Ptasev's C# library is a format reference only** (`work/ego-engine-modding`); no code is reused.
- **GUI = GPUI + gpui-component.**
  - Both are Apache-2.0. GPUI is the framework Zed ships on Linux since July 2024 ([Zed blog](https://zed.dev/blog/zed-on-linux)) and on Windows since 15 October 2025, where it renders with DirectX 11 and DirectWrite ([Zed blog](https://zed.dev/blog/zed-for-windows-is-here)). Its Linux renderer was rewritten on wgpu in February 2026, replacing Blade, to fix freezes on NVIDIA and some Wayland compositors ([zed#46758](https://github.com/zed-industries/zed/pull/46758)).
  - `gpui-component` (Longbridge; the repo now redirects to [longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)) has 60+ widgets, targets macOS, Windows and Linux, and ships in Longbridge Pro. Version 0.7.3 (12 October 2026) pins `gpui-pre =0.3.9` ([crates.io](https://crates.io/crates/gpui-component)).
  - The Zed look must be recreated, not copied: Zed's own `ui` and `theme` crates are GPL-3.0-or-later ([ui/Cargo.toml](https://github.com/zed-industries/zed/blob/main/crates/ui/Cargo.toml)); only `gpui` and `gpui_platform` are Apache-2.0.
  - Mitigations for GPUI's risks (§16): pin one GPUI snapshot and one `gpui-component` version per launcher release; keep all GPUI use inside `swofty-launcher` behind a thin view layer; keep the CLI feature-complete without the GUI. If GPUI becomes untenable, the isolation lets us swap to another Rust GUI (for example `egui`) without touching core.
- **C/C++ for the loader** (§12), isolated behind the C ABI.

---

## 14. Milestones

- **M0 — Launch and injection truth (no mods).** The `swofty run` Linux wrapper; parse the Steam command; place a logging-only `version.dll` and a stub core; set the prefix override; launch `F1_25.exe` directly with the user's affinity and environment; reach the front end offline. Run the staged test from `exe.md` §2.3 (proxy log, then one `MidHook` logging `r12`). A GPUI spike: a `swofty-launcher` window with the session banner on Windows, Linux X11 and Wayland, and Steam Deck desktop mode. Tests: **T1** game behaviour with no anticheat service; **T2** Windows self-relaunch (decides the §4.3 contingency); **T3** Windows `%command%` wrapper support. *Demo:* Steam Play on Linux boots vanilla F1 25 with the proxy loaded (the log proves it), the banner shows "offline", and the loader files are gone after exit.
- **M1 — Formats, overlay and VFS.** `swofty-formats` GVDB and ERP pass the Python oracle on the real files; baseline store; overlay builder; VFS redirect (including the timing question in §12); boot with one replaced ERP from the overlay; verify-files passes. *Demo:* a reskinned loading screen with no game data file changed.
- **M2 — Loader and safety gate.** Gate (ticket, expiry, exe hash, anticheat absence, host check), journaled loader placement, OfflineGuard, window-title suffix, save isolation. *Demo:* the title reads "[Swofty Mod · Offline]"; saves land in the per-profile directory; the gate refuses and the game runs vanilla when any check fails; a vanilla Steam Play finds no loader files.
- **M3 — Home-menu Swofty Mod.** `!pkg` in `swofty-formats`; `swofty.core` generates the F9 prompt, the bottom-left panel and the settings screen from `session.json`; the home-tile `MidHook` lands. *Demo:* a MOD SETTINGS tile directly under Career, F9 opens the settings screen listing installed mods, and the bottom-left panel shows the launcher message.
- **M4 — Mod format and DB merge.** Manifest loader, resolver, DB merge with conflict report, ERP/`!pkg` ops, loose files, EGO track import; the launcher's library, profile and conflict views. *Demo:* install Okayama from its EGO zip through Swofty; two mods editing the same key produce a clear conflict report.
- **M5 — Native plugins and live settings.** Plugin host, C ABI, command/notify bridge, "Apply & restart". *Demo:* toggle a mod in game, Apply & restart, and the change takes effect; a sample third-party plugin loads under consent.
- **M6 — Grid showcases: 24 cars (DB-only, pending confirmation), then 26 cars (track and exe work).**
  - **M6a, 24 cars (12 teams).** The B24 rows (§5.2) through the mod format, with the row-count rule. The in-game B24 test confirms it; if it fails, the selector limit site (§12) is the hook fallback. *Demo:* a 24-car 2026 Grand Prix with T12 as an AI team in garage 12 while the player drives for another team, configured entirely from Swofty, with the grid counted at runtime.
  - **M6b, 26 cars (13 teams).** Track-side grid slots 25–26, garage 13 and pit box 13 on every track (`research/grid26-tracks.md`); a 13th team's DB rows; logos through the texarray writer or reused keys; and exe changes for any fixed-size per-car pools the car-count audits find (§14.1). *Demo:* a 26-car 2026 Grand Prix.
- **M7 — Polish.** The GPUI UI brought to the Zed bar, profiles, update re-baselining, Steam Deck Game Mode, `doctor` diagnostics, BXML and texarray writers, ModKit packaging and export.

Safety (M2) precedes any code mod (M3+). M6b depends on the reverse-engineering program's findings.

### 14.1 Cross-cutting: the reverse-engineering program

Runs alongside M0–M7 under `re/PROTOCOL.md`, which every agent working on the exe follows. The `re/` tree is being set up now.
- **Knowledge base.** Functions, globals, DB keys, string anchors, structs, subsystem maps and signatures live as plain text under `re/db/`, written only through `re/tools/kb.py` and linted by `kb_lint.py`. It describes exactly one exe build.
- **Evidence levels.** L0 guess; L1 static-inferred (a string, constant or pattern match); L2 static-proven (decompiled, callers listed, data flow written down); L3 runtime-confirmed (seen in a trace from a real session). A claim is only as strong as its weakest input, and L3 is per build. The levels gate hooks (§12).
- **Trace plugin.** A plugin in `swofty/mods/`, configured by `swofty/trace.json`, that logs hits and register or memory values at chosen `.xtls` sites and changes nothing. Runtime test requests are `re/tests/T###.md`, one question each, batched into one play session by the lead; logs are filed under `re/traces/`.
- **Car-count audits.** One audit per subsystem that sizes, caps or indexes the field, each finding checked by a different agent who re-derives it from the item alone (`re/PROTOCOL.md` §5). The first, `research/grid24-cap.md`, found the 22-car limit in data. Next are the per-car pools that bound a 26-car grid, starting from the users of the 0x88 participant record and the `session.cars` VM binding.

---

## 15. Security and safety stance

- **Mods load only in a session the launcher prepared.** `version.dll` and `swofty/` are in the game folder only during that session (journaled, removed on exit). The proxy acts only in `F1_25.exe`, and its gate requires `SWOFTY_SESSION` with a fresh, random, single-use, short-lived ticket and no anticheat module. On Windows, Steam's Play button and a double-click of the exe run vanilla, because neither the loader files nor a ticket are present. On Linux, Steam's Play goes through the wrapper, which creates the ticket, and Proton has no anticheat path.
- **The launcher cannot produce a modded online session.** On Linux this is structural. On Windows the modded and vanilla entry points are separate, and the modded one enforces offline in-process (OfflineGuard) and removes the reconnect action. If offline cannot be guaranteed, the loader unloads mods and continues vanilla.
- **No anticheat evasion is designed.** The loader never reports a false anticheat state and never runs mods alongside an active anticheat or online service. It only verifies that the session is already offline and anticheat-free, and otherwise fails closed. The policy module consumes the exe findings on running without the anticheat service; it never fabricates them.
- **Saves are isolated** from Steam Cloud and from vanilla saves, so modded progress can't sync or load into a vanilla or online session.
- **Native plugins need explicit, per-DLL consent**, with declared capabilities and content-hash pinning. Unpinned or unconsented DLLs don't load. Only `.xtls` code is hooked; `.data1` bodies are never patched.
- **Game integrity is preserved.** The VFS never writes the game's data files, the journal restores them in fallback mode, and the loader files are transient, so "verify files" passes.
- **Scope matches the user's authorisation:** single-player, offline, local, non-destructive modding with the anticheat and online off.

---

## 16. Risks and open questions

**Exe and reverse engineering** (each gates a specific choice):
- **Q1 (T1).** What does `F1_25.exe` do on Windows with no anticheat service present? Shapes any in-process offline action beyond verification (§4.6). Resolved on Linux by Proton.
- **Q2 (A2).** Is the VFS viable: path canonicalisation under Proton, and hooks in place before the game's first data-file open (§12)? Decides VFS or journal as the data default (§9.1).
- **Q3 (A1/A3).** Does the proxy load and forward cleanly under the Proton override and on Windows, and do `.xtls` hooks installed after the first window avoid Denuvo integrity triggers? The staged M0 test answers this; fallbacks are `dinput8.dll` or `SwoftyHost.exe`.
- **Q4. 24 cars in game (one confirmation left).** A and B stayed at 22 because their rows sat past the table counts (§2.2). Does B24 give 24 cars with the player on another team, and T12 in garage 12? If T12's AI cars appear but misbehave, the per-vehicle side tables keyed by label are next (`grid24-experiment.md` risk 5). Cosmetic: garage 12 carries the My Team dressing, coloured by the player-team system (risk 1). Decides whether M6a is DB-only.
- **Q5. 26 cars.** The selection and session layers are vector-based and the benchmark clamp is 26, but per-car pools (physics and AI, replay and results, the HUD timing tower) are not audited. No track layer has more than 24 grid slots, 12 garages or 12 pit boxes (`grid-assets.md` §3), so grid slots 25–26, garage 13 and pit box 13 must be authored on every track (`research/grid26-tracks.md`). Decides M6b's scope.
- **Q6.** The GVDB load-buffer cap (`GVDB_READ_ONLY_RGAME_VALUE_DB`). Sets the §7 size-guard threshold; needs the exe work or a runtime test.
- **Q7.** The eaid resolver (`exe.md` §5) is not analysed. A loose-file override path would let some asset mods skip ERP rebuilds.

**Platform:**
- **Q8 (T2/T3).** Windows Steam `%command%` wrapper behaviour and self-relaunch (§4.3).
- **Q9.** Flatpak Steam path visibility and the launch-option edit flow; mitigated by writing `$DATA` inside the sandbox data directory.
- **Q10.** Texture arrays: the logo arrays have 11 slices (`grid-assets.md` §1), and adding a slice needs a texarray writer that doesn't exist in any language yet. Until then, new teams reuse an existing logo key.
- **Q11.** Legal and ethical: offline only, no redistribution of game assets; mods ship operations, not copyrighted payloads, wherever possible.

**Stack (GPUI and the Rust ports):**
- **R1. Pre-1.0 API churn.** GPUI's README says it "is still pre-1.0. There will often be breaking changes between versions" ([gpui README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)). Distribution follows Zed's repo: the official `gpui` crate was last published as 0.2.2 on 22 October 2025 ([crates.io](https://crates.io/crates/gpui)), and the README's `gpui_platform` crate isn't on crates.io. Practical options are a pinned Zed git revision or `gpui-pre`, weekly snapshots of Zed's repo published by the `gpui-component` maintainer (0.3.9, 12 October 2026, "snapshot of zed@93d7456"; [crates.io](https://crates.io/crates/gpui-pre)). `gpui-component` itself moves fast (0.6.2 to 0.7.3 between 18 September and 12 October 2026). Mitigation: pin per release, isolate GPUI in `swofty-launcher`, budget upgrade time.
- **R2. Sparse documentation.** gpui.rs says GPUI "is tied to Zed" and points to the repository for docs and examples ([gpui.rs](https://www.gpui.rs/)). Expect to learn from Zed's and `gpui-component`'s source. Mitigation: the GUI stays a thin shell, and the CLI covers every function.
- **R3. Windows and Linux maturity.** Both platforms ship in a production editor, but Windows support is a year old (October 2025) and the Linux renderer is eight months old (wgpu, February 2026; sources in §13). Zed's own Windows launch asked for feedback on IME and keyboard layouts, multi-monitor setups and high-refresh displays. Mitigation: the M0 spike on Windows, X11, Wayland (including NVIDIA) and Steam Deck desktop mode, before any GUI work depends on it.
- **R4. The Zed look and licences.** Zed's `ui` and `theme` crates are GPL-3.0-or-later, so the launcher recreates the look with `gpui-component` and its own theme rather than copying Zed code.
- **R5. Format-port fidelity.** The Rust ports must match the Python oracle byte for byte. BXML (decoder-only oracle) and texarray (no oracle) need their own round-trip corpora.

**Resolved:** the proxy is not subject to the exe's signed-DLL checks (`exe.md` §2.1); early-bird APC injection matters only for the `SwoftyHost.exe` fallback; world selection is DB data and the world switch works at runtime; the 22-car limit is the entrant list's `_instancecount_`, not code (`grid24-cap.md`).

---

### Critical files for implementation

- `tools/gamedb.py`: GVDB reader/writer and FNV paths; the spec for `swofty-formats`' GVDB layer and the §7 merge, and its oracle.
- `tools/erp.py`, `tools/erpw.py`: ERP index and byte-identical rebuild; the spec for the ERP ops and the overlay builder.
- `tools/pkg.py`, `tools/uinode.py`: `!pkg` reader/writer and djb2 node ids; the basis for the `!pkg` patch representation (§5.3).
- `tools/build_home_tile_mod.py`: the proven F9, panel and route edits that become the generated `swofty.core` data (§11).
- `tools/modswap.py`: the pristine-verify and swap/restore pattern behind the baseline store, the §9 journal and loader placement.
- `native/src/loader/version.c`, `native/src/core/swofty_core.cpp`, `packages/sdk/include/swofty_api.h`, `native/build.sh`, `native/TEST.md`: the in-game loader as built.
- `re/PROTOCOL.md`: evidence levels, hook gates and runtime-test rules (§12, §14.1).

Supporting references: `research/exe.md` (§2 the `version.dll` route, §3 the home-tile `MidHook` and signatures, §4 the world and cap DB fields), `work/grid/bxml.py` (BXML decoder), `work/exe/decomp/` (the home builder 0x14173df80 and the dispatcher 0x141bb3c20), and `work/okayama/pkg/manifest.json` (the EGO track format the adapter consumes).
