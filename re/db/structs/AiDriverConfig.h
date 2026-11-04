// struct: AiDriverConfig
// subsystem: ai
// size: 0x2a0  L2 decomp:re/decomp/1405ff750.c;xref:0x140600458,0x140602a53 (memcpy length 0x2a0 into AiCarRecord+0x238)
// notes: the driver's ai_driver_config, filled by Ai_Parse_CedePositionConfig 0x14065dc50 (default driver at 0x14249a4f0) and copied by value into every AiCarRecord. Each field is an 8-byte slot holding a pointer P to a game database cell: rec = *P, value = *(float *)(rec + 8). The blocking group starts at D+0x40 (Ai_Parse_BlockingConfig); the four floats below are read by the blocking evaluator 0x140624e60 on every call through R+0x238+offset.
struct AiDriverConfig {
    /* 0x090 */ void **blocking_fraction_of_follow_speed;               // L2 decomp:re/decomp/140624e60.c;decomp:re/decomp/14065dc50.c;xref:0x1406251ce float, state 3 of the race start machine (R+0x2c8)
    /* 0x098 */ void **blocking_fraction_of_follow_speed_at_race_start; // L2 decomp:re/decomp/140624e60.c;xref:0x140625211 float, every other race start state (R+0x2d0)
    /* 0x0a0 */ void **blocking_max_blocking_distance;                  // L2 decomp:re/decomp/140624e60.c;xref:0x1406251b1 float, state 3 (R+0x2d8)
    /* 0x0a8 */ void **blocking_max_blocking_distance_at_race_start;    // L2 decomp:re/decomp/140624e60.c;xref:0x1406251f4 float, every other state (R+0x2e0)
};
