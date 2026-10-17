# Native layer test: MOD SETTINGS tile under CAREER

## Bundle

`native/dist/` mirrors the game dir:

| File | Size | MD5 |
|---|---|---|
| `version.dll` | 117760 | `f92dc7bed81e8a71eef6ce58a8ce05f7` |
| `swofty/mods/core/swofty_core.dll` | 832512 | `9f79ccfc96d8d68f9ea25427d6cecb9b` |
| `swofty/mods/` | empty dir | |

Data side: UI variant **v6** (`work/ui/mod_v6`). v6 is v5 minus the static `fe_home.mod_tile`. It keeps the `mod_menu` route, the F9 listener, the "Mod Settings" prompt and the offline panel. Do not install v5 together with the DLLs, or MOD SETTINGS shows twice: native under CAREER, and static at the bottom.

| v6 file (`2025_asset_groups/ui_package/`) | MD5 |
|---|---|
| `flow_frontend.erp` | `b4aba940a05d3c6d3f30f0934f10b94f` |
| `screens/fe_home.erp` | `44e8e6985ae6e5723ac15314050612d1` |
| `persistent.erp` | `cb4122c9956783d351342f4518f5a1b6` (same as v5) |
| `databases.erp` | `204223b525bb5b26f736ef4cd2e5c2a1` (same as v5) |

The bundle targets F1_25.exe sha256 `d873a1c8…865ae` (408063872 bytes, PE timestamp 1789118616). That is the installed exe as of 2026-10-17.

Rebuild: `native/build.sh` (zig 0.16 via `nix shell nixpkgs#zig`, target `x86_64-windows-gnu`). The build is deterministic.

Offline check: `native/test/run_wine_test.sh` builds a synthetic `F1_25.exe` that carries the four signatures and the real home-block instruction sequence. It runs that exe under Wine 11 with the bundle and asserts the tile order, renumbering, label, id and action. It runs the exe three times:
- `trigger`: the IAT trigger fires on `RegisterClassExW`, and the first home build right after `CreateWindowExW` already has the tile.
- `fallback`: the window functions are called around the IAT, so mods load when the window becomes visible.
- `vm`: a `RegisterClassExW` call from a writable+exec section (like Denuvo's `.data1` thunks) is skipped, and the later game-code call fires.

Every mode also checks the vanilla build before mods load, and that the IAT slots are pristine afterwards.

## 1. Install (game closed)

```sh
export F1_25_DIR="$HOME/.local/share/Steam/steamapps/common/F1 25"
python3 tools/modswap.py install work/ui/mod_v6 work/experiments/grid24/<db-variant> --native native/dist
python3 tools/modswap.py status
```

`--native` copies the DLLs and records them in `<game>/swofty/.modswap-native.json`. It refuses to overwrite a `version.dll` that it did not install.

## 2. DLL override (Proton only, once per prefix)

Wine uses its builtin `version.dll` unless told otherwise. Scope the override to `F1_25.exe` so that other processes in the prefix keep the builtin.

With the game and its wineserver stopped (`pgrep -af 'wineserver|F1_25'` prints nothing), append the stanza to `user.reg`:

```sh
python3 - <<'EOF'
import os, shutil, time
p = os.path.expanduser('~/.local/share/Steam/steamapps/compatdata/3059520/pfx/user.reg')
key = '[Software\\\\Wine\\\\AppDefaults\\\\F1_25.exe\\\\DllOverrides]'
s = open(p).read()
assert key not in s, 'already present'
shutil.copy2(p, p + '.swofty-bak')
open(p, 'a').write(f'\n{key} {int(time.time())}\n"version"="native,builtin"\n')
EOF
```

The resulting stanza in `user.reg` is:

```
[Software\\Wine\\AppDefaults\\F1_25.exe\\DllOverrides] 1792200000
"version"="native,builtin"
```

Equivalent `reg` command, run inside the game's Proton context:

```
proton run reg add "HKCU\Software\Wine\AppDefaults\F1_25.exe\DllOverrides" /v version /t REG_SZ /d native,builtin /f
```

Alternative with no registry edit: prefix the launch options with `WINEDLLOVERRIDES="version=n,b"`. Both `proton run` children inherit it. The proxy does nothing inside any process other than `F1_25.exe`, apart from forwarding.

Native Windows needs no override. Copy `dist/` into the game folder and launch with EA AntiCheat off.

## 3. Launch and read `<game>/swofty/loader.log`

Launch exactly as usual. The previous run's log is kept as `loader.prev.log`.

The log has these stages, in order:

1. **Proxy attached**, at process start, before the game's entry point:
   ```
   swofty loader (version.dll proxy) attached, pid …
   host: Z:\…\F1 25\F1_25.exe
   real version.dll: C:\windows\system32\version.dll …
   forwards: 16 of 17 resolved            (17 of 17 on Windows; Wine lacks GetFileVersionInfoByHandle)
   exe: size 408063872 (analysed 408063872), PE timestamp 1789118616 (…) -> MATCHES the analysed build
   exe: image base …, 23 sections         (followed by the section list; .xtls is the code)
   trigger armed: game IAT slot USER32!RegisterClassExW at 0000000145d3cba0 (was …)
   trigger armed: game IAT slot USER32!CreateWindowExW at 0000000145d3ca68 (was …)
   fallback watcher thread started
   ```
2. **Trigger.** The game's window function (`0x1439efdb0`, plain `.xtls`) registers `EGO_WindowClass` about 2 s after attach. Mods load synchronously inside that call, before the window or any UI exists. Both IAT slots are restored first:
   ```
   trigger: USER32!RegisterClassExW from 00000001439f00a2 (.xtls) class 'EGO_WindowClass' on thread …, N ms after attach; loading mods now
   trigger disarmed: USER32!RegisterClassExW slot restored to …
   trigger disarmed: USER32!CreateWindowExW slot restored to …
   ```
   - A `trigger skipped: … (.data1) is not plain game code, still waiting` line before it is expected if a Denuvo VM thunk registers a class first. Two such thunks call this slot.
   - If neither slot fires from game code, the watcher loads mods as soon as a window is visible, with no settle delay: `trigger: main window visible (fallback, IAT trigger never fired) …`. In that case the first home screen may still miss the tile.
3. **Mods load**, on the trigger thread:
   ```
   mods: …\swofty\mods\core: 1 dll(s)
   mod swofty_core.dll: loaded at …, calling SwoftyModInit
   scan .xtls [45 8D 67 01 …] -> 1 hit at 0x141742fc7 (base+0x1742fc7)
   [core] signature home_anchor: 1 hit(s)          (likewise tile_list_builder, item_base_populate, nestr_from_cstr)
   [core] builder base+0x173df80, anchor base+0x1742fc7, hook site base+0x1742fd6
   [core] career_adder base+0x1746710, base_populate base+0x1697b90, nestr_from base+0x41974e0
   [core] registry global base+0x6ca54f8 (= <non-null> now), vm_set_string base+0x846270, vm_set_bool base+0x975db0
   [core] MidHook installed at 0x141742fd6 …
   mod swofty_core.dll: SwoftyModInit returned 1 (ok)
   mods: …\swofty\mods: no .dll files
   mods: done, 1 mod(s) loaded, 1 initialised
   fallback watcher: trigger already fired, exiting
   ```
   The `base+` values above are the expected ones for this build. Absolute addresses shift if the image is relocated; the `base+` offsets do not.
4. **Home menu built.** This line appears on every home visit:
   ```
   [core] hook #1: list(r14)=… r12=0x… r15=0x… rsi=… rip=…
   [core] hook #1: built mod vtable … from career vtable …        (first call only)
   [core] hook #1: added button_<career+1> (items n -> n+1) item=…, id … -> … 'mod_menu', r12 -> …, r15 -> …
   [core] populate #1 item=… node=… a7=… vm=…: title='MOD SETTINGS' description set, localised=false
   ```
5. **Select pressed** on the tile:
   ```
   [core] getAction #1 item=… -> {type 1, route 'mod_menu'}
   ```

## 4. What the user should see

- Home tiles: (BRAKING POINT), CAREER, **MOD SETTINGS**, F1 THE MOVIE (greyed), F1 WORLD, then the divider and the secondaries.
- Down from CAREER focuses MOD SETTINGS, and down again continues to the movie tile or F1 WORLD. Up returns to CAREER.
- The tile uses the same template as CAREER. Its label reads MOD SETTINGS (literal, not localised). The description, where the screen shows one, reads "Configure the mods installed by Swofty Mod Launcher".
- Enter (or A) on MOD SETTINGS plays the confirm sound and opens Settings, the same way the Options route does. Back returns to home.
- The F9 prompt "Mod Settings" (bottom right) and the offline panel text are unchanged from v5.
- When MOD SETTINGS is focused, the background plane has no specific case (`mod_menu` is not in `fe_backgrounds.ui`), so expect the default plane or the previous one.

## 5. Failure modes

| Symptom / log | Meaning | Next step |
|---|---|---|
| No `swofty/loader.log` | Proxy not loaded: override missing, or the DLL is not next to F1_25.exe | Check the stanza. Launch with `PROTON_LOG=1 WINEDEBUG=+loaddll` and grep `~/steam-3059520.log` for `version.dll`: it should say `native` |
| `real version.dll: (not loaded)` or `forwards: 0 of 17` | The system version.dll could not be loaded; game calls to it return 0 | Report the log; the game may still run |
| `-> DIFFERS from the analysed build` | The exe changed (game update) | Expect the signature lines to fail; the game runs vanilla |
| `trigger: game IAT slot USER32!… not found or not writable` | The exe has no `USER32.dll` import by that name (different build) | The fallback watcher still loads mods when the window shows |
| `trigger: main window visible (fallback, …)` | The game never called the armed slots from plain code | Tile may be missing on the first home screen only; report the `trigger skipped` lines |
| `fallback watcher: no trigger and no window after 300 s, mods NOT loaded` | Neither the trigger nor a visible window happened | Report; nothing was hooked |
| `signature X: N hit(s) -> FAIL` then `installing nothing` | A pattern is missing or ambiguous | Game runs vanilla. Signatures need updating for this build |
| `unexpected bytes at …` / `not inside the builder` / `derived-offset check failed` | A pattern matched but the code around it changed | Same as above; nothing installed |
| `MidHook at … FAILED: error type T` | safetyhook could not decode, relocate or allocate | Nothing installed; report T and the inline type |
| `MidHook installed` but no `hook #` lines after reaching home | The builder took another path, or (fallback only) the home menu was built before the hook went in | Open another screen and come back to home; report if it still does not appear |
| `adder result unexpected` | The items vector did not grow by one Career item | A second plain CAREER tile may show. Report the log |
| `populate … no VM node` | Base populate did not bind a node | The tile shows the Career label. Report |
| Tile present, label CAREER, `populate #` lines present | The VM ignores late `title` writes | Report; populate needs another approach |
| Label shows `MOD SETTINGS` in the wrong case, or a raw `lng_` tag | `title_localised=false` not honoured | Cosmetic; report |
| Enter does nothing, `getAction #` logged | Route `mod_menu` missing | Install v6 (or v2–v5) data; check `modswap.py status` |
| Enter does nothing, no `getAction #` line | Input gating (vt+0x70/+0x30) rejected the item | Report |
| Crash before the menu, last line in stages 1–3 | Proxy, trigger or loader problem | Bisect: remove `swofty/mods/core/swofty_core.dll` (proxy only), relaunch |
| Crash on reaching home, last line `hook #` | Hook callback fault | Report the last 20 lines |
| Crash minutes later or in a race, with the hook installed | Possible Denuvo integrity reaction to the patched `.xtls` bytes | Retest proxy-only, then with the core; compare |

A proxy-only run (core DLL removed) logs `mods: …\mods\core: no .dll files`. It is the baseline for anything that crashes.

## 6. Rollback

```sh
python3 tools/modswap.py restore
```

This restores the original ERPs and DB, and deletes `version.dll` and `swofty/mods/core/swofty_core.dll` if they still match the installed MD5s. It also removes the `swofty` folders once they are empty. Logs are kept, so `swofty/` stays until they are deleted by hand.

Registry: leaving the override in place without a `version.dll` is harmless, because Wine falls back to the builtin. To remove it, copy back `user.reg.swofty-bak` (with the game stopped), or delete the two-line stanza. Steam "Verify integrity of game files" restores the ERPs and DB too, but does not delete the added DLLs.
