// struct: DriverContract
// subsystem: career
// size: ?
// notes: a driver's contract object as Career_Set_DriverContractState updates it. Raised by ce-drivers (the lead authorised --force).
struct DriverContract {
    /* 0x050 */ uint32_t driver;        // L2 decomp:re/decomp/140d766b0.c;xref:0x140d7778d driver key the Driver Career signing matches the outgoing driver against
};
