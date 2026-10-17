// struct: EntrantsManager
// subsystem: session
// size: ?
// notes: mgr = *(this+0x278) in Sess_Setup_ChampionshipEntrants
struct EntrantsManager {
    /* 0x2e0 */ EntrantRecord *session_begin; // L2 decomp:re/decomp/14245fb70.c;doc:research/grid24-cap.md#2 selector output vector
    /* 0x2e8 */ EntrantRecord *session_end; // L2 decomp:re/decomp/14245fb70.c;doc:research/grid24-cap.md#2 cleared to begin before selection
    /* 0xe10 */ EntrantSetupMode *setup_mode; // L2 decomp:re/decomp/140ba3ad0.c;doc:research/grid24-cap.md#2
    /* 0xe18 */ void *selectable_list;  // L2 decomp:re/decomp/140ba3ad0.c;doc:research/grid24-cap.md#2 first selectable list; records at +0x10..+0x18
};
