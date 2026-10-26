// struct: DriverCalendar
// subsystem: game
// size: 0xc8  L2 decomp:re/decomp/14126a1a0.c;decomp:re/decomp/141269180.c (ScheduleSeason +0x40 elements, stride 0xc8 at 0x14126bf4b and 200 in 0x141269180)
// notes: a driver's career calendar: vtable at +0x00 (slot +0x8 makes an activity instance, +0x10 notes it), the game mode at +0x10, the activity list at +0x48 (+0x50..+0x58 16-byte slots {allocator, instance}, instance day u16 +0x0a), kept by Game_CareerActivities_AddActivity. Raised by laneS.
struct DriverCalendar {
    /* 0x0c0 */ uint32_t driver; // L2 decomp:re/decomp/141269180.c;xref:0x1412691f0 the driver key the calendar belongs to (EntrantRecord +0x50 form): 0x141269180 finds a driver's calendar by it (0x1412691f0)
};
