// struct: EntrantsManager
// subsystem: session
// size: ?
// notes: mgr = *(this+0x278) in Sess_Setup_ChampionshipEntrants
struct EntrantsManager {
    /* 0x2b0 */ GameSetupMode *game_setup_mode; // L1 decomp:re/decomp/142bebd80.c;xref:0x142bebf3f,0x142becd45,0x142bef8f7,0x142befa63 the current game setup mode, read by the DRS system through [[drs+0xb0]+0x88]+0x260
    /* 0x2e0 */ EntrantRecord *session_begin; // L2 decomp:re/decomp/14245fb70.c;doc:research/grid24-cap.md#2 selector output vector
    /* 0x2e8 */ EntrantRecord *session_end; // L2 decomp:re/decomp/14245fb70.c;doc:research/grid24-cap.md#2 cleared to begin before selection
    /* 0xe10 */ EntrantSetupMode *setup_mode; // L2 decomp:re/decomp/140ba3ad0.c;doc:research/grid24-cap.md#2
    /* 0xe18 */ void *selectable_list;  // L2 decomp:re/decomp/140ba3ad0.c;doc:research/grid24-cap.md#2 first selectable list; records at +0x10..+0x18
};
