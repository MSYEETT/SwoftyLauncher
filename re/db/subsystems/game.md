# game: game:: code named from pretty-function strings

Owner: see `subsystems.tsv`. Source: the bulk naming pass of 2026-10-17.

## Map
- Every row here is L1. Its name comes from a `__PRETTY_FUNCTION__` or log string that the function references (`str:` anchor plus `xref:` sites); the full C++ name is in `notes` as `cxx=`.
- A function was named only when it references exactly one such name and no other function references that name. Names shared by several functions are kept as anchors only, since they usually mean an inlined callee.
- The string can still belong to a callee inlined into the named function. Read the decompile before raising a row to L2.
- Rows move to a feature lane (`session`, `fe_home`, ...) when someone takes them on; rename to `<Prefix>_<Verb>_<Noun>` at that point.
- neon:: names from the same pass are in `core` (prefix `Ne`, class `Ne` prefix dropped). The exe has no `ego::` strings.

## Inbox
- 2026-10-18 22:13 telemetry: telemetry lane: session-live is tracked from two verified unique .xtls signatures. Game_GameModule_OnSessionEnter 0x14080c300 (session start): 41 57 41 56 41 54 56 57 53 48 81 EC 38 02 00 00 48 89 CE 80 3D AA F7 83 05 01 75 6D . Game_SessionStateFinaliseResultsSync_OnResultsFinalised 0x1409d2b20 (session end/results): 56 57 53 48 83 EC 30 4C 89 C7 48 89 D0 48 89 CE 4C 8B 02 48 8D 15 5E C1 67 05 . native/src/core/telemetry.cpp mid-hooks both (telemetry feature, experimental) to flip the telemetry available() gate. Both match exactly once and resolve to the KB addresses.
