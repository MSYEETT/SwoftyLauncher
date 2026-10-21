// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - read-only probe of the Choose Driver populate (vt_Game_DriverCareerDriverSelectViewModel
// slot 13, 0x141a5adc0), to find why a 1990 wizard lists no drivers. Logs the first populates of any
// wizard so a 1990 run can be compared with a 2026 one; it changes nothing.
//
// The populate formats F1CAREER_/F2CAREER_/ICONS_ + the wizard suffix (wizard +0x150), looks each
// name up in the entrants' official map (+0x688, name -> entrant list stat id) and reads the lists
// from +0x560. Before that it gives up (no drivers) unless the save slot's game mode
// ([[vm +0x448] +0x88] +0x260) has type 0x14604ec60 in its chain and a non-null +0xe18; with the
// component 0x14605bed8 it collects the three lists' drivers, without it a default list.
//
// Team Select asks the contracts manager for the picked driver's offers (Game_DriverContracts_BuildOffers),
// which ranks the driver's team (Game_DriverContracts_RankTeam) and offers nothing when the rank is not
// valid. The probe logs the offer list size, the game mode's championships and UpgradesComponent teams,
// and each rank asked for while the offers are built.
//
// Team Select's carousel (0x141890e60) lists the UpgradesComponent's per-team R&D models. They are
// made (0x14a6d4570) for each entrant of the game mode (+0x2e0, 0x88 each, team +0x58, car +0x5c)
// whose team the championship (component +0x270, vt +0x1c0) holds and whose car the vehicle table
// (component +0x318, +0x328 per kind, indexed by stat id) has. At Choose Driver the probe logs the
// model count and those three inputs per entrant.

#include <windows.h>

#include <atomic>
#include <cstdio>
#include <string>

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

namespace {

constexpr uintptr_t kImageBase = 0x140000000;
constexpr uintptr_t kTypeRoot = 0x146129ca8;
constexpr uintptr_t kTypeBase = 0x146129ca0;
constexpr uintptr_t kTypeDriverCareer = 0x14604ec60;
constexpr struct {
    uintptr_t va;
    const char *what;
} kComponents[] = {
    {0x14605bed8, "list-source"}, {0x146060a08, "my-team-probe"}, {0x14605b128, "career"},
    {0x146060948, "recognition"}, {0x14605b008, "career-setup"},  {0x1460609c8, "option-0x17"},
};

std::atomic<uint32_t> g_probes, g_offer_probes, g_rank_probes;
thread_local bool t_in_offers;

uintptr_t rebase(uintptr_t va)
{
    static const uintptr_t base = reinterpret_cast<uintptr_t>(GetModuleHandleW(nullptr));
    return base + (va - kImageBase);
}

uintptr_t unbase(uintptr_t p)
{
    return p ? p - rebase(kImageBase) + kImageBase : 0;
}

template <class T> T rd(uintptr_t a)
{
    T v{};
    if (a)
        swofty::mem::read(a, v);
    return v;
}

std::string str_at(uintptr_t p)
{
    std::string s;
    if (p)
        swofty::mem::c_string(p, s, 128);
    return s;
}

void walk_map(uintptr_t node, uintptr_t mgr, std::string &out, int depth)
{
    if (!node || depth > 32 || out.size() > 1500 || rd<uint8_t>(node + 0x19))
        return;
    walk_map(rd<uintptr_t>(node), mgr, out, depth + 1);
    uint8_t stat = rd<uint8_t>(node + 0x78);
    uintptr_t lb = rd<uintptr_t>(mgr + 0x560), le = rd<uintptr_t>(mgr + 0x568);
    uintptr_t list = lb && stat < (le - lb) / 0x10 ? rd<uintptr_t>(lb + stat * 0x10 + 8) : 0;
    long entrants = list ? long((rd<uintptr_t>(list + 0x18) - rd<uintptr_t>(list + 0x10)) / 0x88) : -1;
    char line[160];
    std::snprintf(line, sizeof line, " %s=%u(%ld)", str_at(rd<uintptr_t>(node + 0x30)).c_str(), stat, entrants);
    out += line;
    walk_map(rd<uintptr_t>(node + 0x10), mgr, out, depth + 1);
}

uintptr_t component(uintptr_t owner, uintptr_t desc)
{
    uintptr_t reg = owner ? owner + 0x1030 : 0;
    auto **rvt = reinterpret_cast<void **>(rd<uintptr_t>(reg));
    return rvt ? reinterpret_cast<uintptr_t (*)(uintptr_t, uintptr_t, int)>(rvt[2])(reg, desc, 1) : 0;
}

void probe_team_models(uintptr_t gm)
{
    uintptr_t up = component(gm, swofty::game::k_Career_UpgradesComponentDesc.address());
    if (!up) {
        swofty::log("[f1-1990] probe: R&D teams: the game mode has no UpgradesComponent");
        return;
    }
    uintptr_t champ = rd<uintptr_t>(up + 0x270), owner = rd<uintptr_t>(up + 0x278), ctx = rd<uintptr_t>(up + 0x318);
    long models = long((rd<uintptr_t>(up + 0x2a0) - rd<uintptr_t>(up + 0x298)) / 8);
    uintptr_t eb = rd<uintptr_t>(owner + 0x2e0), ee = rd<uintptr_t>(owner + 0x2e8);
    long entrants = eb && ee >= eb ? long((ee - eb) / 0x88) : -1;
    swofty::log("[f1-1990] probe: R&D teams: UpgradesComponent 0x%llx has %ld team model(s); championship 0x%llx, "
                "game mode 0x%llx (%s), vehicle ctx 0x%llx, %ld entrant(s)",
                (unsigned long long)up, models, (unsigned long long)champ, (unsigned long long)owner,
                owner == gm ? "this one" : "another", (unsigned long long)ctx, entrants);
    auto **cvt = swofty::mem::object_like(champ) ? reinterpret_cast<void **>(rd<uintptr_t>(champ)) : nullptr;
    std::string line;
    int shown = 0;
    for (uintptr_t e = eb; e && e < ee && shown < 30; e += 0x88, shown++) {
        uint32_t team = rd<uint32_t>(e + 0x58), vehicle = rd<uint32_t>(e + 0x5c);
        bool in = cvt && swofty::mem::in_image(reinterpret_cast<uintptr_t>(cvt[0x1c0 / 8])) &&
                  reinterpret_cast<bool (*)(uintptr_t, uint32_t)>(cvt[0x1c0 / 8])(champ, team);
        uint8_t kind = uint8_t(vehicle);
        uint16_t id = uint16_t(vehicle >> 16);
        uintptr_t table = kind < 3 && ctx ? rd<uintptr_t>(ctx + 0x328 + kind * 0x10) : 0;
        long size = table ? long((rd<uintptr_t>(table + 0x10) - rd<uintptr_t>(table + 8)) / 0x10) : -1;
        uintptr_t car = table && id < size ? rd<uintptr_t>(rd<uintptr_t>(table + 8) + 8 + id * 0x10) : 0;
        char b[96];
        std::snprintf(b, sizeof b, " [team %08x %s, car %u/%u of %ld %s]", team, in ? "in" : "NOT in", kind, id, size,
                      car ? "found" : "MISSING");
        line += b;
        if (line.size() > 900) {
            swofty::log("[f1-1990] probe: R&D teams:%s", line.c_str());
            line.clear();
        }
    }
    if (!line.empty())
        swofty::log("[f1-1990] probe: R&D teams:%s", line.c_str());
}

void probe_before(uintptr_t vm)
{
    uintptr_t wizard = rd<uintptr_t>(vm + 0x438), mgr = rd<uintptr_t>(vm + 0x440);
    std::string options;
    for (uintptr_t p = rd<uintptr_t>(wizard + 0x100), e = rd<uintptr_t>(wizard + 0x108); p && p < e && options.size() < 96; p++) {
        char b[8];
        std::snprintf(b, sizeof b, " %u", rd<uint8_t>(p));
        options += b;
    }
    swofty::log("[f1-1990] probe: choose driver: wizard suffix \"%s\" group \"%s\" options [%s ]",
                str_at(rd<uintptr_t>(wizard + 0x150)).c_str(), str_at(rd<uintptr_t>(wizard + 0x128)).c_str(), options.c_str());
    uintptr_t lb = rd<uintptr_t>(mgr + 0x560), le = rd<uintptr_t>(mgr + 0x568);
    std::string map;
    uintptr_t head = rd<uintptr_t>(mgr + 0x688);
    walk_map(head ? rd<uintptr_t>(head + 8) : 0, mgr, map, 0);
    swofty::log("[f1-1990] probe: entrant lists %ld; official map (name=stat(entrants)):%s", lb && le > lb ? long((le - lb) / 0x10) : -1L,
                map.c_str());
    uintptr_t slot = rd<uintptr_t>(rd<uintptr_t>(vm + 0x448) + 0x88);
    uintptr_t gm = slot ? rd<uintptr_t>(slot + 0x260) : 0;
    if (!gm || !swofty::mem::object_like(gm)) {
        swofty::log("[f1-1990] probe: slot 0x%llx has no game mode object (0x%llx)", (unsigned long long)slot, (unsigned long long)gm);
        return;
    }
    auto **vt = reinterpret_cast<void **>(rd<uintptr_t>(gm));
    uintptr_t type = reinterpret_cast<uintptr_t (*)(uintptr_t)>(vt[5])(gm);
    std::string chain;
    bool driver = false;
    if (type != rebase(kTypeBase))
        for (uintptr_t t = type + 8, n = 0; t && n < 16; t = rd<uintptr_t>(t + 0x10), n++) {
            char b[24];
            std::snprintf(b, sizeof b, " 0x%llx", (unsigned long long)unbase(t));
            chain += b;
            if (t == rebase(kTypeDriverCareer)) {
                driver = true;
                break;
            }
            if (t == rebase(kTypeRoot))
                break;
        }
    std::string comps;
    uintptr_t reg = gm + 0x1030;
    auto **rvt = reinterpret_cast<void **>(rd<uintptr_t>(reg));
    for (auto &k : kComponents) {
        void *got = rvt ? reinterpret_cast<void *(*)(uintptr_t, uintptr_t, int)>(rvt[2])(reg, rebase(k.va), 1) : nullptr;
        comps += std::string(" ") + k.what + (got ? "=yes" : "=no");
    }
    swofty::log("[f1-1990] probe: game mode 0x%llx vtable 0x%llx type 0x%llx chain%s -> %s; +0xe18 0x%llx; components%s",
                (unsigned long long)gm, (unsigned long long)unbase(reinterpret_cast<uintptr_t>(vt)),
                (unsigned long long)unbase(type), chain.c_str(), driver ? "driver career type" : "NOT the driver career type",
                (unsigned long long)rd<uintptr_t>(gm + 0xe18), comps.c_str());
    probe_team_models(gm);
}

std::string upgrade_teams(uintptr_t gm, size_t &count)
{
    count = 0;
    uintptr_t reg = gm ? gm + 0x1030 : 0;
    auto **rvt = reinterpret_cast<void **>(rd<uintptr_t>(reg));
    uintptr_t comp = rvt ? reinterpret_cast<uintptr_t (*)(uintptr_t, uintptr_t, int)>(rvt[2])(
                               reg, swofty::game::k_Career_UpgradesComponentDesc.address(), 1)
                         : 0;
    if (!comp)
        return " (no UpgradesComponent)";
    std::string keys;
    for (uintptr_t p = rd<uintptr_t>(comp + 0x298), e = rd<uintptr_t>(comp + 0x2a0); p && p < e && count < 64; p += 8, count++) {
        char b[16];
        std::snprintf(b, sizeof b, " %08x", rd<uint32_t>(rd<uintptr_t>(p) + 0xc));
        keys += b;
    }
    return keys;
}

}

SWOFTY_WRAP(Game_DriverContracts_BuildOffers, void, void *contracts, uint32_t driver)
{
    uintptr_t c = reinterpret_cast<uintptr_t>(contracts);
    long before = long(rd<uintptr_t>(c + 0x3d0) - rd<uintptr_t>(c + 0x3c8));
    t_in_offers = true;
    orig(contracts, driver);
    t_in_offers = false;
    if (g_offer_probes++ >= 8)
        return;
    long after = long(rd<uintptr_t>(c + 0x3d0) - rd<uintptr_t>(c + 0x3c8));
    uintptr_t gm = rd<uintptr_t>(c + 0x278);
    size_t teams = 0;
    std::string keys = upgrade_teams(gm, teams);
    long champs = long((rd<uintptr_t>(gm + 0x1070) - rd<uintptr_t>(gm + 0x1068)) / 8);
    swofty::log("[f1-1990] probe: contract offers for driver %08x: offer list %ld -> %ld byte(s); game mode 0x%llx, "
                "%ld championship(s), UpgradesComponent teams %zu:%s",
                driver, before, after, (unsigned long long)gm, champs, teams, keys.c_str());
}

SWOFTY_WRAP(Game_DriverContracts_RankTeam, int *, int *out, void *standings, void *game_mode, uint32_t team, uint64_t season,
            void *alloc)
{
    int *r = orig(out, standings, game_mode, team, season, alloc);
    if (t_in_offers && g_rank_probes++ < 8)
        swofty::log("[f1-1990] probe: team %08x rank %d valid %u", team, out ? out[0] : -1,
                    out ? unsigned(reinterpret_cast<const uint8_t *>(out)[4]) : 0u);
    return r;
}

SWOFTY_VTABLE(vt_Game_DriverCareerDriverSelectViewModel, 13, void *, void *vm)
{
    bool probe = g_probes++ < 6;
    if (probe)
        probe_before(reinterpret_cast<uintptr_t>(vm));
    void *r = orig(vm);
    if (probe) {
        uintptr_t b = rd<uintptr_t>(reinterpret_cast<uintptr_t>(vm) + 0x2c0), e = rd<uintptr_t>(reinterpret_cast<uintptr_t>(vm) + 0x2c8);
        swofty::log("[f1-1990] probe: choose driver populated %ld driver card(s)", b && e >= b ? long((e - b) / 0x170) : -1L);
    }
    return r;
}
