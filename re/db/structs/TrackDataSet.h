// struct: TrackDataSet
// subsystem: game
// size: 0x30  L1 decomp:re/decomp/142e44b50.c (vector growth /0x30);xref:0x142e60e60
// notes: one track_data.game_setup.TrackDataSet row in memory. Game_TrackData_GetTrack takes the first set whose masks hold the query's bits: mode bit = game setup mode stat_id (f1 0, f1_26 13), season bit = season statId (season_2025 7, season_2026 8), format bit = weekend format (default 0, sprint 1). The reader skips a set whose three masks equal an earlier set's.
struct TrackDataSet {
    /* 0x008 */ void *tracks_begin;           // L1 decomp:re/decomp/142e44b50.c;xref:0x142e60eb5 16-byte items {?, TrackDataTrack *} by track index (TrackDataRecord +0x01)
    /* 0x010 */ void *tracks_end;             // L1 xref:0x142e60eb9 end of the items
    /* 0x020 */ uint32_t mode_mask;           // L1 decomp:re/decomp/142e44b50.c;xref:0x142e60e7b bits of the set's game_setup_modes stat ids
    /* 0x024 */ uint32_t season_mask;         // L1 decomp:re/decomp/142e44b50.c;xref:0x142e60e90 bits of the set's seasons statIds
    /* 0x028 */ uint32_t format_mask;         // L1 decomp:re/decomp/142e44b50.c;xref:0x142e60ea4 bits of the set's weekend_formats
};
