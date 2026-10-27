// struct: TrackDataRecord
// subsystem: game
// size: ?
// notes: a track of the TRACK_DATA system's track list (TrackDataSystem +0x28..+0x30, 16-byte entries {?, TrackDataRecord *}); the reader matches a TrackData row's track field against its name.
struct TrackDataRecord {
    /* 0x001 */ uint8_t index;                // L1 decomp:re/decomp/142e44b50.c;xref:0x142e60eb1 the slot of the track in every TrackDataSet's items
    /* 0x0d0 */ char *name;                   // L1 decomp:re/decomp/142e44b50.c the track id ("melbourne"), compared at 0x142e45930
};
