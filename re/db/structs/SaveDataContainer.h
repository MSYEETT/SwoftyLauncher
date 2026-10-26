// struct: SaveDataContainer
// subsystem: core
// size: ?
// notes: neon save-data node (base ctor Ne_Construct_SaveDataContainer 0x14364d570). Obfuscated bools are {byte, key} pairs, true when byte != (key & 1). The save root save_game sits at SaveDataSystem+0x20 (vt_Game_SaveDataRootContainer).
struct SaveDataContainer {
    /* 0x000 */ void **vtable;                      // L2 decomp:re/decomp/143654650.c;xref:0x1436548fe the root stores vt_Game_SaveDataRootContainer here; slot +0x10 serialise, +0x18 deserialise, +0x20 reset
    /* 0x100 */ uint8_t retain_unknown;             // L2 decomp:re/decomp/14364f3a0.c;decomp:re/decomp/14364e000.c;xref:0x14364d72f,0x14364fef0,0x14364e22f,0x14364e8e9 keep child bags no registered child claims (load) and write them back (save); value = (byte ^ retain_unknown_key) & 1; false on the root
    /* 0x108 */ uint64_t retain_unknown_key;        // L2 decomp:re/decomp/14364f3a0.c;decomp:re/decomp/14364e000.c;xref:0x14364d72f,0x14364fef7,0x14364e237,0x14364e8f1 random key of the retain_unknown pair, written with it by the base ctor
    /* 0x1c8 */ uint64_t retained_count;            // L2 decomp:re/decomp/14364f3a0.c;decomp:re/decomp/14364e000.c;xref:0x14364d82f,0x14365089b,0x14364e24a,0x143650285 number of retained unknown child bags (map at +0x190); zeroed by the ctor and by Game_SaveData_ClearRetained on each load
};
