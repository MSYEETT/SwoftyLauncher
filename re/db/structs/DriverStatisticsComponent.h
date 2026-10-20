// struct: DriverStatisticsComponent
// subsystem: career
// size: ?
// notes: game::championship::DriverStatisticsComponent (vtable 0x1456c1880; subclass DriverCareerPlayerDriverStatisticsComponent 0x1456c1530 shares the layout). driverStatistics::Data is embedded at +0x2a8 (its size is not proven, so its fields are listed here at component offsets). Raised by ce-drivers (career lane owner career-re is stale; the lead authorised --force).
struct DriverStatisticsComponent {
    /* 0x278 */ void *stat_context;              // L2 decomp:re/decomp/14113d120.c;xref:0x14113d36b,0x14113d1b0 context provider; its vtable slot +0xc8 fills the 4-byte stat context (call at 0x14113d378)
    /* 0x290 */ CareerTimeComponent *career_time; // L2 decomp:re/decomp/14113d120.c;xref:0x14113d441 career time component; progress x = min(1, clock / length)
    /* 0x2a8 */ uint8_t stats_data;              // L2 decomp:re/decomp/140db9300.c;decomp:re/decomp/14113d120.c;xref:0x140db966e,0x140db9627 first byte of the embedded driverStatistics::Data; the getters take &stats_data (lea rdx,[rcx+0x2a8])
    /* 0x2a9 */ uint8_t patch_gate;              // L2 decomp:re/decomp/14113d120.c;xref:0x14113d35b Data+1; patched curves are used only when this is 0 (cmp byte [rdi+1],0)
    /* 0x2ac */ uint32_t last_patch_applied_checksum;  // L2 decomp:re/decomp/141134c30.c;decomp:re/decomp/14113d120.c;xref:0x141134ff2,0x14113d362 save key last_patch_applied_checksum; Data+4 >= 2 enables patched curves
    /* 0x2b0 */ uint32_t last_patch_declined_checksum; // L2 decomp:re/decomp/141134c30.c;xref:0x14113505d save key last_patch_declined_checksum
    /* 0x2d0 */ void *stat_infos_begin;          // L2 decomp:re/decomp/14113d120.c;decomp:re/decomp/14a523160.c;xref:0x14113d3d0,0x14a5232b4 16-byte slots {StatInfo *, ?}
    /* 0x2d8 */ void *stat_infos_end;            // L2 decomp:re/decomp/14113d120.c;xref:0x14113d3d7 end of the stat info slots
    /* 0x310 */ DriverStatsEntry *driver_data_begin; // L2 decomp:re/decomp/141134c30.c;decomp:re/decomp/14a523160.c;xref:0x141135208,0x14a52317d save key driverData; 0x50-byte entries (Data+0x68)
    /* 0x318 */ DriverStatsEntry *driver_data_end;   // L2 decomp:re/decomp/141134c30.c;decomp:re/decomp/14a523160.c;xref:0x141135201,0x14a523181 end of driverData (Data+0x70)
};
