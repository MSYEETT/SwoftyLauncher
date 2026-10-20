// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - no contract scene at the start of a 1990 Driver Career.
//
// A new Driver Career reaches ContractNegotiationsDecision with no signed contract for the player and
// routes (link 0x5e3c3c2d) to DriverCareerContractNegotiations: Intro (agent cutscene), Update (the
// negotiation screen) and Outro. In the 1990 career the decision wrap signs the contract the screen
// would offer by default (the player's current team, type 0, one season, the recommended target)
// with the game's own BuildContract/SignContract, then lets the three states pass straight through:
// Intro and Outro skip their enter and return the continue link, Update enters as an idle state and
// leaves without the screen. A failed sign leaves the stock scene in place.

#include <cstdio>
#include <cstring>
#include <string>

#include <swofty/career.hpp>
#include <swofty/db.hpp>
#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "contract_target.hpp"

namespace {

constexpr uint32_t kLinkNegotiate = 0x5e3c3c2d;
constexpr uint32_t kLinkContinueFallback = 0x7c9b1ec4;

bool g_skip;
bool g_update_skipped;
int g_profile = -1;

template <class T> T at(const void *p, size_t off)
{
    T v;
    std::memcpy(&v, static_cast<const uint8_t *>(p) + off, sizeof v);
    return v;
}

uint32_t continue_link()
{
    uint32_t v = *swofty::game::g_Career_FlowLinkContinue;
    return v ? v : kLinkContinueFallback;
}

bool db_text_is(const std::string &key, const char *want)
{
    swofty::db::Value v{};
    if (swofty::db::get(key, &v) != swofty::db::Result::ok || !v.s)
        return false;
    return !std::strcmp(v.s, want);
}

// 1 = the 1990 profile's database (Driver Career 2026 saves to driver_career_1990 or runs the
// F1_career_1990 event list), 0 = another database, -1 = not readable yet.
int is_1990_profile()
{
    if (g_profile >= 0)
        return g_profile;
    if (!swofty::db::available())
        return -1;
    int64_t count = 0;
    if (swofty::db::int_at("game_modes._ModeChampionship_instancecount_", &count) != swofty::db::Result::ok ||
        count <= 0 || count > 256)
        return -1;
    for (int64_t i = 0; i < count; i++) {
        std::string row = "game_modes.ModeChampionship_" + std::to_string(i);
        if (db_text_is(row + ".save_data.name", "driver_career_1990") ||
            db_text_is(row + ".save_data_0.name", "driver_career_1990") || db_text_is(row + ".event_list", "F1_career_1990")) {
            swofty::log("[f1-1990] contract: 1990 career database found (%s)", row.c_str());
            return g_profile = 1;
        }
    }
    swofty::log("[f1-1990] contract: no ModeChampionship row saves to driver_career_1990; the contract scene stays");
    return g_profile = 0;
}

void *contracts_component(void *state)
{
    auto *ctx = at<uint8_t *>(state, 0xca0);
    auto *holder = ctx ? at<uint8_t *>(ctx, 0x88) : nullptr;
    auto *career = holder ? at<uint8_t *>(holder, 0x260) : nullptr;
    if (!career)
        return nullptr;
    void *registry = career + 0x1030;
    auto **vt = *static_cast<void ***>(registry);
    if (!vt || !vt[2])
        return nullptr;
    auto get = reinterpret_cast<void *(*)(void *, const void *, int)>(vt[2]);
    return get(registry, reinterpret_cast<const void *>(swofty::game::k_Career_DriverCareerContractsManagerComponentDesc.address()), 1);
}

bool same_key(uint32_t a, uint32_t b)
{
    return (a & 0xff) == (b & 0xff) && (a >> 16) == (b >> 16);
}

const uint8_t *human_entrant(const uint8_t *contracts)
{
    auto *entity = at<const uint8_t *>(contracts, 0x278);
    if (!entity)
        return nullptr;
    auto *b = at<const uint8_t *>(entity, 0x2e0), *e = at<const uint8_t *>(entity, 0x2e8);
    for (; b && b < e; b += 0x88)
        if ((b[0x4b] & 0x10) && at<uint16_t>(b, 0x52) != 0x3fff)
            return b;
    return nullptr;
}

const uint8_t *contract_record(const uint8_t *contracts, uint32_t driver)
{
    auto *owner = at<const uint8_t *>(contracts, 0x350);
    if (!owner)
        return nullptr;
    auto *b = at<const uint8_t *>(owner, 0x298), *e = at<const uint8_t *>(owner, 0x2a0);
    for (; b && b < e; b += 0x10) {
        auto *rec = at<const uint8_t *>(b, 0x08);
        if (rec && same_key(at<uint32_t>(rec, 0x50), driver))
            return rec;
    }
    return nullptr;
}

bool sign_recommended(uint8_t *contracts)
{
    const uint8_t *human = human_entrant(contracts);
    if (!human) {
        swofty::log("[f1-1990] contract: no human entrant; the contract scene stays");
        return false;
    }
    uint32_t driver = at<uint32_t>(human, 0x50), team = at<uint32_t>(human, 0x58);
    const uint8_t *rec = contract_record(contracts, driver);
    if (!rec || rec[0x2c]) {
        swofty::log("[f1-1990] contract: driver %u/%u has %s; the contract scene stays", driver & 0xff, driver >> 16,
                    rec ? "a signed contract" : "no contract record");
        return false;
    }
    alignas(16) uint8_t buf[0x80] = {};
    swofty::game::Career_Build_DriverContract(contracts, buf, driver, team, 0, 0, 1);
    int32_t base = at<int32_t>(buf, 0x18);
    auto *rb = at<const f1990::RatingRange *>(contracts, 0x318), *re = at<const f1990::RatingRange *>(contracts, 0x320);
    int32_t target = f1990::recommended_target(base, rb, re);
    std::memset(buf, 0, sizeof buf);
    swofty::game::Career_Build_DriverContract(contracts, buf, driver, team, 0, target, 1);
    swofty::game::Career_Sign_DriverContract(contracts, buf, driver, 0x3fff0000, 1);
    rec = contract_record(contracts, driver);
    bool signed_now = rec && rec[0x2c];
    swofty::log("[f1-1990] contract: signed driver %u/%u to team %u/%u, rating %d, target %d, %d season(s), ends %d: record %s",
                driver & 0xff, driver >> 16, team & 0xff, team >> 16, base, target, at<int32_t>(buf, 0x20),
                at<int32_t>(buf, 0x24), signed_now ? "signed" : "NOT signed");
    return signed_now;
}

}

SWOFTY_WRAP(Career_Decide_ContractNegotiation, uint32_t *, void *state, uint32_t *link)
{
    uint32_t *r = orig(state, link);
    g_skip = false;
    if (!link || *link != kLinkNegotiate || is_1990_profile() != 1)
        return r;
    swofty::career::State cs{};
    if (swofty::career::state(&cs) == swofty::career::Result::ok && cs.humans > 1) {
        swofty::log("[f1-1990] contract: %u human players; the contract scene stays", cs.humans);
        return r;
    }
    auto *contracts = static_cast<uint8_t *>(contracts_component(state));
    if (!contracts || !swofty::game::Career_Needs_DriverContract(contracts))
        return r;
    g_skip = sign_recommended(contracts);
    if (g_skip)
        swofty::log("[f1-1990] contract: skipping the negotiation scene (continue link 0x%08x)", continue_link());
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
    swofty::log("[f1-1990] contract: negotiation scene skipped");
    return link;
}
