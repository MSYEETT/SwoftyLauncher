// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career - Follow your driver's real career (progressive_follow.dll, DESIGN.md D8).
//
// The F1 1990 contract skip (f1-1990 native/src/contract_skip.cpp), aimed at the real calendar. When
// Career_Decide_ContractNegotiation picks the negotiation (link 0x5e3c3c2d: no contract, or it ends
// this season) in a Progressive career whose team moves are follow_driver, with one human and
// Career_Needs_DriverContract true, the player signs a one-season contract with the game's own
// Career_Build_DriverContract + Career_Sign_DriverContract (type 0, the screen's recommended target)
// and the negotiation scene passes straight through (Intro and Outro skip, Update leaves at once):
//   - no active contract (a new career): his current team, the start season's real team;
//   - a contract that ends this season: his real team of the next year, from progressive.dll
//     (ProgressiveFollowTeam, data/years.json pool). For another team the signing caches the
//     contract and the game's season processor (0x140d766b0) moves him at the season change.
// Past his real career (no seat in the next year, or after 2026) the stock negotiation stays, so
// moves are free from then on. A failed sign also leaves the stock scene. The mode is the career's
// store entry (ProgressiveFollowMode); before the first Team HQ of a new career, which has none yet,
// the profile option team_moves.

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <algorithm>
#include <cstring>
#include <string>

#include <swofty/career.hpp>
#include <swofty/career_content.hpp>
#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

SWOFTY_SETTING(std::string_view, team_moves, "follow_driver");

namespace {

using namespace swofty::game;

constexpr const char *kGroup = "f1_career_progressive";
constexpr uint32_t kLinkNegotiate = 0x5e3c3c2d;
constexpr uint32_t kLinkContinueFallback = 0x7c9b1ec4;
constexpr uint32_t kNoKey = 0x3fff0000u;
constexpr uint32_t kHumanBit = 0x10000000u;
constexpr uintptr_t kRatingRanges = 0x318; // DriverCareerContractsManagerComponent: rating ranges begin/end
constexpr uintptr_t kContractRating = 0x18; // built contract record: overall rating
constexpr uintptr_t kContractSeasons = 0x20;
constexpr uintptr_t kContractEnd = 0x24;

struct RatingRange {
    uint32_t min, max, minus, plus;
    int32_t recommended;
};
static_assert(sizeof(RatingRange) == 20);

bool g_skip = false;
bool g_update_skipped = false;

template <class T> T rdv(uintptr_t a, T def = T{})
{
    T v{};
    return swofty::mem::read(a, v) ? v : def;
}

bool same_key(uint32_t a, uint32_t b)
{
    return ((a ^ b) & 0xffff00ffu) == 0;
}

uint32_t continue_link()
{
    uint32_t v = g_Career_FlowLinkContinue.address() ? *g_Career_FlowLinkContinue : 0;
    return v ? v : kLinkContinueFallback;
}

int32_t recommended_target(int32_t base, uintptr_t contracts)
{
    if (base < 0)
        base = 0;
    uintptr_t b = rdv<uintptr_t>(contracts + kRatingRanges), e = rdv<uintptr_t>(contracts + kRatingRanges + 8);
    if (b && e > b && (e - b) % sizeof(RatingRange) == 0 && (e - b) / sizeof(RatingRange) < 64 &&
        swofty::mem::readable(b, e - b))
        for (uintptr_t p = b; p < e; p += sizeof(RatingRange)) {
            RatingRange r = rdv<RatingRange>(p);
            if (uint32_t(base) < r.min || uint32_t(base) > r.max)
                continue;
            int64_t lo = std::max<int64_t>(0, int64_t(base) - r.minus);
            int64_t hi = std::min<int64_t>(100, int64_t(base) + r.plus);
            return int32_t(std::clamp<int64_t>(int64_t(base) + r.recommended, std::min(lo, hi), hi));
        }
    return std::min(base, 100);
}

bool human_row(uintptr_t contracts, EntrantRecord &out)
{
    uintptr_t entity = rdv<uintptr_t>(contracts + offsetof(DriverCareerContractsManagerComponent, entity));
    uintptr_t b = entity ? rdv<uintptr_t>(entity + offsetof(CareerEntity, entrants_begin)) : 0;
    uintptr_t e = entity ? rdv<uintptr_t>(entity + offsetof(CareerEntity, entrants_end)) : 0;
    if (!b || e <= b || (e - b) % sizeof(EntrantRecord) || (e - b) / sizeof(EntrantRecord) > 256)
        return false;
    int hits = 0;
    for (uintptr_t p = b; p < e; p += sizeof(EntrantRecord)) {
        EntrantRecord r = rdv<EntrantRecord>(p);
        if ((r.ordinal_flags & kHumanBit) && r.id_hi != 0x3fff) {
            out = r;
            ++hits;
        }
    }
    return hits == 1;
}

uintptr_t contract_record(uintptr_t contracts, uint32_t driver)
{
    uintptr_t owner = rdv<uintptr_t>(contracts + offsetof(DriverCareerContractsManagerComponent, contract_owner));
    uintptr_t b = owner ? rdv<uintptr_t>(owner + offsetof(DriverCareerContractOwner, contracts_begin)) : 0;
    uintptr_t e = owner ? rdv<uintptr_t>(owner + offsetof(DriverCareerContractOwner, contracts_end)) : 0;
    for (uintptr_t p = b; p && p < e; p += 16)
        if (uintptr_t rec = rdv<uintptr_t>(p + 8); rec && same_key(rdv<uint32_t>(rec + offsetof(DriverContract, driver)), driver))
            return rec;
    return 0;
}

int follow_mode()
{
    static int (*fn)() = nullptr;
    static bool looked = false;
    if (!looked) {
        HMODULE m = GetModuleHandleW(L"progressive.dll");
        fn = m ? reinterpret_cast<int (*)()>(reinterpret_cast<void *>(GetProcAddress(m, "ProgressiveFollowMode"))) : nullptr;
        looked = m != nullptr;
    }
    int r = fn ? fn() : -1;
    return r >= 0 ? r : std::string(team_moves.get()) == "follow_driver" ? 1 : 0;
}

int follow_team(uint32_t driver_stat, uint32_t *team_stat, int *year)
{
    static int (*fn)(uint32_t, uint32_t *, int *) = nullptr;
    static bool looked = false;
    if (!looked) {
        HMODULE m = GetModuleHandleW(L"progressive.dll");
        fn = m ? reinterpret_cast<int (*)(uint32_t, uint32_t *, int *)>(
                     reinterpret_cast<void *>(GetProcAddress(m, "ProgressiveFollowTeam")))
               : nullptr;
        looked = m != nullptr;
    }
    return fn ? fn(driver_stat, team_stat, year) : -1;
}

bool sign(uintptr_t contracts)
{
    EntrantRecord human{};
    if (!human_row(contracts, human)) {
        swofty::log("[progressive] follow: no single human entrant; the contract scene stays");
        return false;
    }
    uint32_t driver;
    std::memcpy(&driver, &human.id_lo, sizeof driver);
    uintptr_t rec = contract_record(contracts, driver);
    if (!rec) {
        swofty::log("[progressive] follow: the player has no contract record; the contract scene stays");
        return false;
    }
    bool active = rdv<uint8_t>(rec + offsetof(DriverContract, active)) != 0;
    uint32_t team = human.team_stat_id;
    int year = 0;
    const char *why = "his current team (new career)";
    if (active) {
        uint32_t next = 0;
        int r = follow_team(driver >> 16, &next, &year);
        if (r <= 0) {
            swofty::log("[progressive] follow: %s; the stock negotiation stays",
                        r < 0 ? "progressive.dll is not loaded" : "no real seat for the next year (or past 2026), moves are free");
            return false;
        }
        team = next << 16;
        why = same_key(team, human.team_stat_id) ? "his real team next year (stays)" : "his real team next year (moves)";
    }
    if ((team & 0xffff0000u) == kNoKey) {
        swofty::log("[progressive] follow: no team to sign with; the contract scene stays");
        return false;
    }
    alignas(16) uint8_t buf[0x80] = {};
    Career_Build_DriverContract(reinterpret_cast<void *>(contracts), buf, driver, team, 0, 0, 1);
    int32_t base = rdv<int32_t>(reinterpret_cast<uintptr_t>(buf) + kContractRating);
    int32_t target = recommended_target(base, contracts);
    std::memset(buf, 0, sizeof buf);
    Career_Build_DriverContract(reinterpret_cast<void *>(contracts), buf, driver, team, 0, target, 1);
    Career_Sign_DriverContract(reinterpret_cast<void *>(contracts), buf, driver, kNoKey, 1);
    bool ok = !Career_Needs_DriverContract(reinterpret_cast<void *>(contracts)) ||
              rdv<uint8_t>(contract_record(contracts, driver) + offsetof(DriverContract, active)) != 0;
    swofty::log("[progressive] follow: signed driver %u with team %u (%s%s%s), rating %d, target %d, %d season(s), ends "
                "%d: %s",
                driver >> 16, team >> 16, why, year ? ", " : "", year ? std::to_string(year).c_str() : "", base, target,
                rdv<int32_t>(reinterpret_cast<uintptr_t>(buf) + kContractSeasons),
                rdv<int32_t>(reinterpret_cast<uintptr_t>(buf) + kContractEnd),
                ok ? "signed or cached for the season change" : "NOT signed");
    return ok;
}

} // namespace

SWOFTY_WRAP(Career_Decide_ContractNegotiation, uint32_t *, void *state, uint32_t *link)
{
    uint32_t *r = orig(state, link);
    g_skip = false;
    if (!link || *link != kLinkNegotiate || !swofty::career::is(kGroup))
        return r;
    if (follow_mode() != 1) {
        swofty::log("[progressive] follow: this career chooses its team; the stock negotiation stays");
        return r;
    }
    swofty::career::State cs{};
    if (swofty::career::state(&cs) == swofty::career::Result::ok && cs.humans > 1)
        return r;
    auto contracts = reinterpret_cast<uintptr_t>(
        swofty::career::component_by_desc(k_Career_DriverCareerContractsManagerComponentDesc.get()));
    if (!contracts || !Career_Needs_DriverContract(reinterpret_cast<void *>(contracts)))
        return r;
    g_skip = sign(contracts);
    if (g_skip)
        swofty::log("[progressive] follow: skipping the negotiation scene (continue link 0x%08x)", continue_link());
    return r;
}

SWOFTY_WRAP(Career_ContractIntro_OnEnter, uint64_t, void *state)
{
    return g_skip ? 1 : orig(state);
}

SWOFTY_WRAP(Career_ContractIntro_OnUpdate, uint32_t *, void *state, uint32_t *link)
{
    if (!g_skip)
        return orig(state, link);
    *link = continue_link();
    return link;
}

SWOFTY_WRAP(Career_ContractUpdate_OnEnter, uint64_t, void *state)
{
    if (!g_skip)
        return orig(state);
    auto *s = static_cast<uint8_t *>(state);
    s[0xd58] = 1;
    s[0xd5a] = 0;
    s[0xd5b] = 0;
    std::memset(s + 0xd60, 0, 0x10);
    g_update_skipped = true;
    return 1;
}

SWOFTY_WRAP(Career_ContractUpdate_OnUpdate, uint32_t *, void *state, uint32_t *link)
{
    if (!g_update_skipped)
        return orig(state, link);
    *link = continue_link();
    return link;
}

SWOFTY_WRAP(Career_ContractUpdate_OnExit, uint64_t, void *state)
{
    if (!g_update_skipped)
        return orig(state);
    g_update_skipped = false;
    return 1;
}

SWOFTY_WRAP(Career_ContractOutro_OnEnter, uint64_t, void *state)
{
    return g_skip ? 1 : orig(state);
}

SWOFTY_WRAP(Career_ContractOutro_OnUpdate, uint32_t *, void *state, uint32_t *link)
{
    if (!g_skip)
        return orig(state, link);
    *link = continue_link();
    g_skip = false;
    swofty::log("[progressive] follow: negotiation scene skipped");
    return link;
}

SWOFTY_ADDON()
