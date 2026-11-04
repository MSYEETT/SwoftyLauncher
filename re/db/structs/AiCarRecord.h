// struct: AiCarRecord
// subsystem: ai
// size: 0x508  L2 decomp:re/decomp/1405ff750.c;xref:0x1405ffbc6,0x1406004d2,0x140624edf (imul 0x508, add [S+0xb0],0x508)
// notes: one per vehicle slot, in the vector at AiSystem+0xa8. The driver config D is the 0x2a0-byte by-value copy at +0x238 (memcpy at 0x140600461 and 0x140602a61 from the driver's registered config or the default driver config built by 0x14249a4f0). Every config slot is an 8-byte pointer P to a game database cell: rec = *P, value = *(float *)(rec + 8), rec may be 0 (the reader then uses 0). The blocking evaluator 0x140624e60 re-reads slot, P, rec and value on every call. Slot address = R + 0x238 + the AiDriverConfig offset.
struct AiCarRecord {
    /* 0x000 */ uint8_t vehicle_index;                                 // L2 decomp:re/decomp/1405ff750.c;decomp:re/decomp/140624e60.c;xref:0x1406001a0,0x14060273f vehicle slot index (mov [rdi],bpl at the push; the evaluator indexes by the car's byte at +0x1d and the vector stride 0x508)
    /* 0x230 */ void *handling_config;                                 // L2 decomp:re/decomp/1405ff750.c;decomp:re/decomp/140624e60.c;xref:0x14060044a,0x140602a40,0x1406251e3 vehicle AiHandlingConfig V (read as [R+0x230] at 0x1406251e3)
    /* 0x238 */ AiDriverConfig driver_config;                          // L2 decomp:re/decomp/1405ff750.c;decomp:re/decomp/14249a4f0.c;decomp:re/decomp/14065dc50.c;xref:0x140600461,0x140602a61 driver config D copied by value (0x2a0 bytes, AiDriverConfig)
};
