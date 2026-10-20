// struct: CareerEntity
// subsystem: career
// size: ?
// notes: the championship entity (the career entity in a career): GameManager +0xc0 -> GameModeContext +0x88 -> ChampionshipHolder +0x260. Holds the ECS component registry (embedded at +0x1030, vtable slot +0x10 = get(registry, &type_desc, search_parents)), the entrant records, and the entity's championships. Built by the factory 0x1407d2820. Raised by ce-econ (career lane owner career-re is stale; the lead authorised --force); fields +0x1040..+0x1070 from the season lane's Career_Set_CurrentChampionship row.
struct CareerEntity {
    /* 0x000 */ void **vtable;                       // L2 decomp:re/decomp/1414c5f80.c;xref:0x1414c5f9c,0x1414c5fa2 player card reads the vtable and calls slot +0x28 (GetType) on the entity
    /* 0x2e0 */ EntrantRecord *entrants_begin;       // L2 decomp:re/decomp/1414c7450.c;decomp:re/decomp/141229e00.c;xref:0x1414c75a4,0x141229e16,0x14122931d entrant records, 0x88 stride (player card finds the local player's entrant; RP generation finds the human entrant of a team)
    /* 0x2e8 */ EntrantRecord *entrants_end;         // L2 decomp:re/decomp/1414c7450.c;decomp:re/decomp/141229e00.c;xref:0x1414c75ab,0x1414c75de,0x141229e1d end of the entrant records
    /* 0x2f8 */ uint8_t entrant_update_busy;         // L2 decomp:re/decomp/148618ee0.c;xref:0x148618efc,0x148618f39,0x1486190da set to 1 while Career_Set_EntrantData rewrites an entrant row, back to 0 when done
    /* 0x1030 */ void **component_registry;          // L2 decomp:re/decomp/140d6b6f0.c;decomp:re/decomp/1414c5f80.c;decomp:re/decomp/1407d2820.c;xref:0x140d6b6fd,0x140d6b704,0x1414c5fe5,0x1407d3a81 embedded component registry (its vtable pointer; the factory stores it); &component_registry is the registry object passed to vtable slot +0x10 get
    /* 0x1040 */ void *championships_begin;          // L2 decomp:re/decomp/1407d2820.c;decomp:re/decomp/1407bdfb0.c;xref:0x1407d3a9f,0x1407bdfd8,0x1407c28c1 16-byte entries {?, Championship*} pushed by the factory after the Championship ctor
    /* 0x1048 */ void *championships_end;            // L2 decomp:re/decomp/1407d2820.c;decomp:re/decomp/1407bdfb0.c;xref:0x1407d450a,0x1407bdfdf,0x1407c28c8 end of the championship entries
    /* 0x1058 */ void *current_championship;         // L2 decomp:re/decomp/1407c26f0.c;decomp:re/decomp/1407bdfb0.c;decomp:re/decomp/1412291c0.c;xref:0x1407c27af,0x1407be183,0x141229268,0x1434fcfc9,0x1407d3e73 the current Championship (vt_Career_Championship), written by Career_Set_CurrentChampionship; read by the RP weekly award and the championship walkers
    /* 0x1068 */ void **championship_order_begin;    // L2 decomp:re/decomp/1407c26f0.c;decomp:re/decomp/1407bdfb0.c;xref:0x1407c2700,0x1407be032,0x1407d3ab1 order list of Championship*; 0x1407c26f0 makes the first unfinished one current
    /* 0x1070 */ void **championship_order_end;      // L2 decomp:re/decomp/1407c26f0.c;decomp:re/decomp/1407bdfb0.c;xref:0x1407c2710,0x1407be039 end of the order list
};
