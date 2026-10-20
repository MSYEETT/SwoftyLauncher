// struct: Session
// subsystem: career
// size: 0xa0 L2 decomp:re/decomp/140ae8880.c;decomp:re/decomp/140aece00.c (Weekend.sessions are indexed and destroyed in steps of 0xa0)
// notes: game::championship::Session, one session of a Weekend (serialised by 0x140af3750: completion, ruleset, id, results_list, penalties). Raised by ce-season with the lead's authorisation.
struct Session {
    /* 0x008 */ uint32_t session_id;                 // L2 decomp:re/decomp/140ae8880.c;decomp:re/decomp/140af3750.c;xref:0x140ae8913 the session id the standings key their sessions by
    /* 0x010 */ uint32_t completion;                 // L2 decomp:re/decomp/140aecf80.c;decomp:re/decomp/140af3750.c;xref:0x140aed2ef,0x140af8a43 2 = complete (cmp dword [x+0x10],2)
    /* 0x020 */ EntrantRecord *participants_begin;   // L2 decomp:re/decomp/140ae8880.c;decomp:re/decomp/140af4660.c;xref:0x140ae8ec4,0x140af4701 the session's entrants (results_list order)
    /* 0x028 */ EntrantRecord *participants_end;     // L2 decomp:re/decomp/140ae8880.c;decomp:re/decomp/140af4660.c;xref:0x140ae8ec8,0x140af46fd end of the entrants
};
