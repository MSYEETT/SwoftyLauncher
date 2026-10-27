// struct: TrackDataTrack
// subsystem: game
// size: 0x290  L1 decomp:re/decomp/142e44b50.c (alloc 0x290 at 0x142e459a5)
// notes: one track of one TrackDataSet (TrackData row, sub-tables the set lacks come from its parent_data_set at read time). Fuel fields from the reader's TrackGeneralData / TrackFuelSettings reads; RaceFuelSettings row layout (0x34): +0x00 percent_100, +0x04 laps_1, +0x08 laps_3, +0x0c laps_5, +0x10 percent_25, +0x14 percent_35, +0x18 percent_50, +0x1c percent_100, +0x20 laps_10, +0x24 laps_15, +0x28 laps_20, +0x2c laps_2, +0x30 laps_9 (floats). Readers: Game_TrackData_GetTrack callers read +0xf0 (0x142d1d751, 0x1436aea77: fuel in tank -= laps * burn) and +0x1a0 (0x140ad66a7).
struct TrackDataTrack {
    /* 0x000 */ TrackDataRecord *record;      // L1 decomp:re/decomp/142e44b50.c the track's record from TrackDataSystem +0x28
    /* 0x0f0 */ float fuel_burn_per_lap;      // L1 decomp:re/decomp/142e44b50.c;xref:0x142e46fc8,0x1436aea77 TrackGeneralData.fuel_burn_per_lap
    /* 0x100 */ uint8_t race_fuel[0x9c];      // L1 decomp:re/decomp/142e44b50.c;xref:0x142e4756a RaceFuelSettings rows, 0x34 bytes each, up to 3
    /* 0x1a0 */ uint64_t race_fuel_count;     // L1 decomp:re/decomp/142e44b50.c;xref:0x140ad66a7 RaceFuelSettings row count
    /* 0x1b4 */ float qualifying_one_shot;    // L1 decomp:re/decomp/142e44b50.c TrackFuelSettings Qualifying.one_shot
    /* 0x1bc */ float qualifying_short;       // L1 decomp:re/decomp/142e44b50.c TrackFuelSettings Qualifying.short
    /* 0x1c0 */ float qualifying_full;        // L1 decomp:re/decomp/142e44b50.c TrackFuelSettings Qualifying.full
};
