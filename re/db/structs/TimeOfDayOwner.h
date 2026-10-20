// struct: TimeOfDayOwner
// subsystem: session
// size: ?
// notes: the object at GameWeatherSystem+0x1da8 that setupSession passes to the dataset build thunks
// (Sess_Tod_BuildSessionDataset_Thunk, 0x14479c560); class name unknown. Raised by ce-weather.
struct TimeOfDayOwner {
    /* 0x2d0 */ TimeOfDaySystem *tod;   // L2 decomp:re/decomp/155755570.c;xref:0x1557555ed,0x1438b776e the TimeOfDaySystem the thunk bodies build into (same object as GameWeatherSystem+0x1d98)
};
