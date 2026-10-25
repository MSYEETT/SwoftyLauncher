// struct: DriverCareerContractsManagerComponent
// subsystem: career
// size: ?
// notes: game::championship::DriverCareerContractsManagerComponent (vtable 0x145690090, Driver Career). Its signing 0x140d766b0 (reached from vtable slot +0x188 0x140d76570) posts MessageDriverMoveConfirmed and then calls Career_Set_EntrantData_Thunk for each moved driver; it keeps no driverMoveData log. Raised by ce-drivers; car_numbers raised by ce-swap-number (the lead authorised --force).
struct DriverCareerContractsManagerComponent {
    /* 0x270 */ void *weekend_source;             // L2 decomp:re/decomp/140d766b0.c;xref:0x140d76f2a,0x140d77366,0x140d77523 its vtable slot +0x108 returns the weekend index put in the move message (call at 0x140d76f34)
    /* 0x278 */ CareerEntity *entity;             // L2 decomp:re/decomp/140d766b0.c;xref:0x140d766cf,0x140d767b0,0x140d7705a the championship entity passed to Career_Set_EntrantData_Thunk
    /* 0x350 */ DriverCareerContractOwner *contract_owner; // L2 decomp:re/decomp/140d766b0.c;xref:0x140d7687d,0x140d77721 holder of the contract list (+0x298)
    /* 0x358 */ CareerTimeComponent *career_time; // L2 decomp:re/decomp/140d766b0.c;xref:0x140d76f3a,0x140d77377,0x140d77533 clock read at +0x2b0 for the move day (clock / 1440)
    /* 0x368 */ CarNumberComponent *car_numbers;  // L2 decomp:re/decomp/140d6f890.c;decomp:re/decomp/140d766b0.c;xref:0x140d6f9a9,0x140d7813f the CarNumberComponent: vtable slot +0x78 (0x140d6f890) stores the entity component registry get with its descriptor 0x14605ade0 here (0x140d6f9a9); the signing passes it, when not null, to Career_DriverNumbers_CheckExisting with each queued signing's driver (read 0x140d7813f, call 0x140d7814e)
};
