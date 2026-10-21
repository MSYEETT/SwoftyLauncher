# game: game:: code named from pretty-function strings

Owner: see `subsystems.tsv`. Source: the bulk naming pass of 2026-10-17.

## Map
- Every row here is L1. Its name comes from a `__PRETTY_FUNCTION__` or log string that the function references (`str:` anchor plus `xref:` sites); the full C++ name is in `notes` as `cxx=`.
- A function was named only when it references exactly one such name and no other function references that name. Names shared by several functions are kept as anchors only, since they usually mean an inlined callee.
- The string can still belong to a callee inlined into the named function. Read the decompile before raising a row to L2.
- Rows move to a feature lane (`session`, `fe_home`, ...) when someone takes them on; rename to `<Prefix>_<Verb>_<Noun>` at that point.
- neon:: names from the same pass are in `core` (prefix `Ne`, class `Ne` prefix dropped). The exe has no `ego::` strings.
- Hand-raised rows (not from the bulk pass): `g_Game_Root` (0x14601db08), the root object whose +0xa0 context holds the UI module at +0x28 (see core.md, fullscreen video), and `g_Game_FlowLinkStay` (0x146cb2280), the link a flow state's onUpdate writes to stay put; `g_Career_FlowLinkContinue` follows it at 0x146cb2284.

## Inbox
- 2026-10-18 22:13 telemetry: telemetry lane: session-live is tracked from two verified unique .xtls signatures. Game_GameModule_OnSessionEnter 0x14080c300 (session start): 41 57 41 56 41 54 56 57 53 48 81 EC 38 02 00 00 48 89 CE 80 3D AA F7 83 05 01 75 6D . Game_SessionStateFinaliseResultsSync_OnResultsFinalised 0x1409d2b20 (session end/results): 56 57 53 48 83 EC 30 4C 89 C7 48 89 D0 48 89 CE 4C 8B 02 48 8D 15 5E C1 67 05 . native/src/core/telemetry.cpp mid-hooks both (telemetry feature, experimental) to flip the telemetry available() gate. Both match exactly once and resolve to the KB addresses.
- 2026-10-19 13:27 ea-port: EA port: game.session_enter/race_started/unlockable_owned_state and vm.flow_update are in the parallel EA set re/db/signatures.ea.json with section .rdata and pending-dump patterns; EA patterns need a decrypted-memory dump (on-disk EA exe fully packed, see core.md).
- 2026-10-19 13:56 ai-re: ai lane: Game_RivalryManager_SetTrace (0x141261010) + _2 (0x141261170) are cross-referenced from subsystems/ai.md as the championship rivals surface; left in the game lane (they look like stat/telemetry trace set/clear, not on-track behaviour). Re-home to ai if you decompile them and they drive race behaviour.
- 2026-10-19 14:03 ui-re: re-homed UI rows out of game into feature lanes: StateFrontend OnStateEnter/RecalcTransitionState/GoToFrontend + FrontendTransitionViewModel::onCommandRaised + SessionDirector/Results addTab*Screen -> fe_flow; HudTyrePerformanceViewModel::setToOpen -> hud; GameEadpModule::showNotification + UIConnectionManager::broadcastNotifications + UITutorialElementBehaviour -> fe_popup; Player add/remove InputProvider + GameInputModule tooltip -> fe_input. New names in those lanes.
