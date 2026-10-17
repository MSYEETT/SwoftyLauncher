# protection: Denuvo layout, cert checks, hook safety

Owner: see `subsystems.tsv`. Write-up: research/exe.md §1–2.

## Map
- Real `.text` is `.xtls` (113,920 `.pdata` functions); 15,828 functions have bodies relocated into `.data1`, reached through 5-byte `jmp rel32` thunks left in `.xtls`. Rows: `*_Thunk` in `.xtls` (callable), `*_Body` in `.data1` (never hook or patch; kb_lint enforces the naming and no signatures).
- Entry point `Prot_Entry_Stub` (.sbss); TLS callbacks `g_Prot_TlsCallbacks`.
- WinVerifyTrust callers: `Prot_Unk_1409c6220`, `Prot_Verify_LogitechSigner`, `Prot_Unk_1452e60a0`. None touch version/dinput8/winmm.
- VirtualProtect callers: `Prot_Unk_1443f44a0` and mid-function 0x14534b68c.
- Runtime (T000 run1): a MidHook in `.xtls` installed 2 s after the main window, ran through two home builds without a crash. Integrity checking of `.xtls` remains unproven either way; keep hooks in `.xtls`, few, and installed late.

## Inbox
