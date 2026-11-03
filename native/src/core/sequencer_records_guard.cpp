#include "runtime.hpp"
#include "sequencer_records_logic.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>
#include <mutex>
#include <string>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// Career events sequencer record guard (re/db/globals.tsv site_Game_CareerEventsSequencerFirstRecord).
//
// Game_CareerEventsSequencer_Initialise (0x140c71e80) runs as a career session is entered. After the career hub scenes
// it adds the team mate asset groups: it looks the session's event up in the career event table (CareerEventData +0x2e8
// .. +0x2f0, records of 0x88 bytes), then collects every record of the table with the same +0x58 and +0x5a bytes as that
// record and the game's equality (0x1424ceb70 fills a pointer vector at [rsp+0x98], begin at [rsp+0xa0]), and reads the
// first collected record (mov rdx,[rax] at 0x140c72ea0). The collection skips records whose +0x5e is 0x3fff, so an event
// whose record is not in the table, or is one of those, collects nothing: begin is 0 and the read faults with
// ACCESS_VIOLATION reading 0x0 at F1_25.exe+0xc72ea0 as the first session of the career is entered (a 2014 and a 2016
// Progressive Career with a third-party calendar package).
//
// The cleanup after the asset group call already tests the vector's begin for 0 (test rdx,rdx / je 0x140c72ee2). At the
// site a null rax resumes at that join with the asset groups of that event left out, which only costs the team mate
// scene assets.
//
// The site is found by byte scan, which works without an exported KB row.
//
// SWOFTY_SEQUENCER_RECORDS_GUARD=0 turns it off.

namespace {

namespace sr = runtime::sequencer_records;

const runtime::Sig S_FIRST{"site_Game_CareerEventsSequencerFirstRecord",
                           "48 8B 84 24 A0 00 00 00 48 8B 10 48 83 C2 50 4C 89 E9 E8 ?? ?? ?? ?? 48 8B 4C 24 38", 0xc72ea0,
                           runtime::Sig::addr, 0, 8, 0};

constexpr uint8_t JOIN[] = {0x48, 0x8B, 0xAC, 0x24, 0x90, 0x00, 0x00, 0x00};

uintptr_t join_at = 0;
bool said = false;
std::mutex said_lock;
safetyhook::MidHook h_first;

void first_record(safetyhook::Context &c)
{
    if (!sr::skips(c.rax))
        return;
    c.rip = join_at;
    bool first;
    {
        std::lock_guard<std::mutex> hold(said_lock);
        first = sr::says(c.rax, said);
    }
    if (first)
        swofty::log("[career] events sequencer: the session's event has no matching record in the career event table "
                    "(its track or calendar entry is missing); the team mate asset groups are left out");
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_SEQUENCER_RECORDS_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[career] events sequencer record guard off (SWOFTY_SEQUENCER_RECORDS_GUARD=%s)", v);
        return;
    }
    uintptr_t at = runtime::find(S_FIRST, "career", "career");
    if (!at)
        return;
    join_at = sr::resume(at);
    if (!runtime::readable(reinterpret_cast<const void *>(join_at), sizeof JOIN) ||
        memcmp(reinterpret_cast<const void *>(join_at), JOIN, sizeof JOIN)) {
        swofty::log("[career] events sequencer record guard: not hooked: the join after the asset group call is not where "
                    "this build keeps it");
        return;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), first_record);
    if (!h) {
        swofty::log("[career] events sequencer record guard: the hook failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_first = std::move(*h);
    swofty::log("[career] events sequencer record guard armed: a session whose event has no record in the career event "
                "table enters without the team mate asset groups instead of faulting");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("events sequencer record guard", setup);
}
