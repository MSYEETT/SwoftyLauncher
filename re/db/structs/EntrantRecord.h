// struct: EntrantRecord
// subsystem: session
// size: 0x88 L2 decomp:re/decomp/14245fb70.c candidate and session vectors step 0x88
struct EntrantRecord {
    /* 0x04b */ uint8_t flags;          // L1 doc:research/grid24-cap.md#3 bit 0x10 excludes the record from the AI count
    /* 0x05c */ uint32_t vehicle_ref;   // L1 doc:research/grid24-cap.md#2 passed as &e+0x5c to Sess_Find_Vehicle; width unproven
};
