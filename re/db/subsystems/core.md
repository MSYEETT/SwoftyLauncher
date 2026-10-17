# core: neon engine basics

Owner: see `subsystems.tsv`. Write-up: research/exe.md §1.

## Map
- `NeStr` (`structs/NeStr.h`): +0 djb2 hash, +4 refcount (u16), +6 24-bit length/flags, chars from +9. A null pointer means "" (hash 0x1505). `Ne_Make_StrFromCStr(NeStr** out, const char*, flags, len)` creates or reuses one (T000: works from a mod thread at the home build).
- Allocators (`structs/NeAllocator.h`): vt+0x18 `alloc(self, size, align)`, vt+0x38 `free(self, p)`. `g_Ne_DefaultAllocator` (0x146ca54a8) is used for frees in the adders.
- `Ne_Format_String` (0x1441630f0) is the sprintf used for `button_%d`.
- Build facts: clang unity build `F1_2025_VS2022_win64_steam_master_dx12_clang_unity.exe`; no RTTI; `__PRETTY_FUNCTION__` strings in asserts recover class and method names (search strings_ascii.txt for `game::`).

## Open
- NeStr release path (for mods that should not leak refs).

## Inbox
- 2026-10-17 13:04 bulk-names: Bulk naming pass 2026-10-17: 78 neon:: functions added at L1 as Ne_<Class>_<Method> (class Ne prefix dropped), named from pretty-function strings; cxx= in notes holds the full name. See subsystems/game.md for the rules used.
- 2026-10-17 16:24 gamelog: Ne_Log_Thunk (0x1406d4c80) is the channel logger; every call site is gated by g_Ne_LogGate (enabled byte + 373-slot hash set, step 13 + H%13), empty in retail. Neon UI route actions don't log: they return errors through Ne_Format_String into an out NeStr. native/src/gamelog (swofty_gamelog.dll) overwrites the logger, opens chosen channel gates from log_channels.tsv (re/tools/logchan.py), and logs route errors from a read-only site at Ne_Format_String entry. Verified V0014-V0020.
