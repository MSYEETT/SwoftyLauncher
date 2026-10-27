// struct: TrackDataSystem
// subsystem: game
// size: 0x230  L1 xref:0x1407e70f8 (alloc size in GameModule init 0x1407e5420)
// notes: the TRACK_DATA system: every track_data.game_setup.TrackDataSet in memory, built by the reader 0x142e44b50 and queried by Game_TrackData_GetTrack 0x142e60e00. GameModule+0x280 and CareerEntity+0xf58 point at it (0x1407e712f, 0x14036fe70). Used by addons/progressive/native/src/fuel_logic.hpp.
struct TrackDataSystem {
    /* 0x000 */ void **vtable;                // L1 xref:0x142e58ab6 vt_Game_TrackDataSystem 0x1457ed380
    /* 0x010 */ void *arena;                  // L1 xref:0x142e58a45 the TRACK_DATA memory arena the ctor is given
    /* 0x068 */ TrackDataSet *sets_begin;     // L1 decomp:re/decomp/142e44b50.c;xref:0x142e60e0e the track data sets, 0x30 bytes each, in database row order
    /* 0x070 */ TrackDataSet *sets_end;       // L1 decomp:re/decomp/142e44b50.c;xref:0x142e60e12 end of the sets
};
