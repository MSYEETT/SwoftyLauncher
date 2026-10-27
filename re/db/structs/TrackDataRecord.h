// struct: TrackDataRecord
// subsystem: game
// size: ?
// notes: a track of the TRACK_DATA system's track list (TrackDataSystem +0x28..+0x30, 16-byte entries {?, TrackDataRecord *}); the reader matches a TrackData row's track field against its name. Both fields are read the same way by the reader, the getter and 0x140b78000/0x142d04ee0, but the code that writes them (or fills the system's +0x28 list) is not located yet, so the verify rows are partial (V1168, V1169).
struct TrackDataRecord {
    /* 0x001 */ uint8_t index;                // L1 decomp:re/decomp/142e44b50.c;xref:0x142e45939,0x142e60eb1,0x142d04eed the slot of the track in every TrackDataSet's items
    /* 0x0d0 */ char *name;                   // L1 decomp:re/decomp/142e44b50.c;xref:0x142e45926 the track id ("melbourne"), compared at 0x142e45930
};
