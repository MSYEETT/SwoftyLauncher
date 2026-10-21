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
// Team Select's carousel (0x141890e60) lists the UpgradesComponent's per-team R&D models. They are
// made (0x14a6d4570) for each entrant of the game mode (+0x2e0, 0x88 each, team +0x58, car +0x5c)
// whose team the championship (component +0x270, vt +0x1c0) holds and whose car the vehicle table
// (component +0x318, +0x328 per kind, indexed by stat id) has. At Choose Driver the probe logs the
// model count and those three inputs per entrant.
//
// The game mode's entrants are a copy of one parsed entrant list: Game_CareerSetup_ResetEntrants takes
// the list whose stat id the CareerSetupComponent holds (+0x2b8) from the entrants registry (+0x560)
// and copies its records. In a 1990 career every copied entrant reads team 0x3fff0000 and car 16383
// although f1_1990's rows name the 1990 teams and cars. The probe logs, for the 1990 lists (105-107)
// and the 2026 ones (67, 97), how many registry records carry a team, the entrants right after each
// reset and at each 1990 wizard step, and the championship's game setup mode chain.
//
// The season length component (k_Game_SeasonLengthComponentDesc) keeps the career's calendar: a
// preset (data +0x290 -> +0xc0, 0x30 each: id +8, type +0x18, setup mode / season / format masks
// +0x1c..+0x24) is applied when the career's setup mode, season and format (career vt+0xc8) pass its
// masks, and fills the calendar (+0x2b0..+0x2b8, 0x28 each) from the dynamic event data's track list
// with the preset's id (career +0xf58 -> +0xb8). The probe logs all of it before and after the 1990
// wizard applies preset 0 and at Choose Driver.

#include <windows.h>

#include <atomic>
#include <cstdio>
#include <string>

#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

void f1990_probe_season(const char *when, const void *season);

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

constexpr uint8_t kRegistryLists[] = {67, 97, 105, 106, 107};
constexpr uintptr_t kGameSetupRegistry = 0x146079b28;

std::atomic<uint32_t> g_probes, g_step_probes;

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

std::string records(uintptr_t b, uintptr_t e)
{
    long n = 0, teamed = 0, carred = 0;
    std::string first;
    for (uintptr_t p = b; p && p < e; p += 0x88, n++) {
        uint32_t team = rd<uint32_t>(p + 0x58), car = rd<uint32_t>(p + 0x5c);
        teamed += (team >> 16) != 0x3fff;
        carred += (car >> 16) != 0x3fff;
        if (n < 3) {
            char x[64];
            std::snprintf(x, sizeof x, " [driver %08x team %08x car %08x]", rd<uint32_t>(p + 0x50), team, car);
            first += x;
        }
    }
    char head[96];
    std::snprintf(head, sizeof head, "%ld record(s), %ld with a team, %ld with a car;", n, teamed, carred);
    return head + first;
}

std::string registry_lists(uintptr_t mgr)
{
    uintptr_t lb = rd<uintptr_t>(mgr + 0x560), le = rd<uintptr_t>(mgr + 0x568);
    size_t count = lb && le > lb ? (le - lb) / 0x10 : 0;
    std::string out;
    for (uint8_t stat : kRegistryLists) {
        uintptr_t list = stat < count ? rd<uintptr_t>(lb + stat * 0x10 + 8) : 0;
        uintptr_t named = list ? rd<uintptr_t>(list + 0x28) : 0;
        char b[96];
        std::snprintf(b, sizeof b, " | %u \"%s\" at 0x%llx: ", stat, named ? str_at(rd<uintptr_t>(named + 0x10)).c_str() : "?",
                      (unsigned long long)list);
        out += b;
        out += list ? records(rd<uintptr_t>(list + 0x10), rd<uintptr_t>(list + 0x18)) : std::string("none");
    }
    return out;
}

std::string name_at(uintptr_t obj, size_t off)
{
    uintptr_t named = obj ? rd<uintptr_t>(obj + off) : 0;
    return named ? str_at(rd<uintptr_t>(named + 0x10)) : std::string("?");
}

void probe_championship(const char *when, uintptr_t career)
{
    uintptr_t champ = rd<uintptr_t>(career + 0x1058);
    uintptr_t xb = rd<uintptr_t>(champ + 0x520), xe = rd<uintptr_t>(champ + 0x528);
    std::string out;
    char b[160];
    for (uintptr_t xp = xb; xp && xp < xe && out.size() < 1400; xp += 8) {
        uintptr_t x = rd<uintptr_t>(xp);
        long ys = long(rd<uint64_t>(x + 0x70));
        std::snprintf(b, sizeof b, " | setup mode \"%s\" 0x%llx, %ld entrant setup mode(s):", name_at(x, 0xd8).c_str(),
                      (unsigned long long)x, ys);
        out += b;
        for (long i = 0; i < ys && i < 8; i++) {
            uintptr_t y = rd<uintptr_t>(x + 0x10 + i * 8);
            long lists = long(rd<uint64_t>(y + 0x120));
            uintptr_t domain = rd<uintptr_t>(y + 0x18);
            long domain_teams = domain ? long((rd<uintptr_t>(domain + 0x68) - rd<uintptr_t>(domain + 0x60)) / 4) : -1;
            std::snprintf(b, sizeof b, " [0x%llx team kinds %x/%x/%x, %ld list(s), domain teams %ld:", (unsigned long long)y,
                          rd<uint32_t>(y + 0x24c), rd<uint32_t>(y + 0x250), rd<uint32_t>(y + 0x254), lists, domain_teams);
            out += b;
            for (long l = 0; l < lists && l < 4; l++) {
                uintptr_t list = rd<uintptr_t>(y + 0x20 + l * 8);
                std::snprintf(b, sizeof b, " \"%s\" ", name_at(list, 0x28).c_str());
                out += b;
                out += list ? records(rd<uintptr_t>(list + 0x10), rd<uintptr_t>(list + 0x18)) : std::string("none");
            }
            out += "]";
        }
    }
    uintptr_t reg = rd<uintptr_t>(rebase(kGameSetupRegistry));
    std::string modes;
    long count = 0;
    for (uintptr_t p = rd<uintptr_t>(reg + 0x48), e = rd<uintptr_t>(reg + 0x50); p && p < e && count < 64; p += 0x10) {
        uintptr_t mode = rd<uintptr_t>(p + 8);
        if (!mode)
            continue;
        count++;
        modes += " " + name_at(mode, 0xd8);
    }
    swofty::log("[f1-1990] probe: championship at %s: career 0x%llx flag %u, championship 0x%llx with %ld game setup mode(s)%s; "
                "game setup registry %ld:%s",
                when, (unsigned long long)career, unsigned(rd<uint8_t>(career + 0x10c8)), (unsigned long long)champ,
                xb && xe >= xb ? long((xe - xb) / 8) : -1L, out.c_str(), count, modes.c_str());
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
    swofty::log("[f1-1990] probe: registry lists%s", registry_lists(mgr).c_str());
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
    probe_championship("choose driver", gm);
    f1990_probe_season("choose driver", reinterpret_cast<void *>(component(gm, swofty::game::k_Game_SeasonLengthComponentDesc.address())));
}


}

namespace {

std::atomic<uint32_t> g_hub_probes;

void probe_hub(const char *when, uintptr_t career)
{
    if (!career || g_hub_probes++ >= 12)
        return;
    uintptr_t champ = rd<uintptr_t>(career + 0x1058);
    uintptr_t sb = rd<uintptr_t>(champ + 0x548), se = rd<uintptr_t>(champ + 0x550);
    uint32_t index = rd<uint32_t>(champ + 0x560);
    long seasons = sb && se >= sb ? long((se - sb) / 0x10) : -1;
    uintptr_t season = seasons > long(index) ? rd<uintptr_t>(sb + index * 0x10 + 8) : 0;
    uintptr_t wb = rd<uintptr_t>(season + 0x288), we = rd<uintptr_t>(season + 0x290);
    long weekends = wb && we >= wb ? long((we - wb) / 8) : -1;
    std::string list;
    for (uintptr_t w = wb; w && w < we && list.size() < 500; w += 8) {
        uintptr_t weekend = rd<uintptr_t>(w);
        char b[96];
        std::snprintf(b, sizeof b, " %s(track %u state %u)", str_at(rd<uintptr_t>(weekend + 0x28)).c_str(),
                      rd<uint8_t>(weekend + 0x278), rd<uint32_t>(weekend + 0x23c));
        list += b;
    }
    uintptr_t time = component(career, swofty::game::k_Career_CareerTimeComponentDesc.address());
    std::string dates;
    for (uintptr_t d = rd<uintptr_t>(time + 0x298), de = rd<uintptr_t>(time + 0x2a0); d && d < de && dates.size() < 300; d += 0x10) {
        char b[48];
        std::snprintf(b, sizeof b, " [%08x %08x %08x %08x]", rd<uint32_t>(d), rd<uint32_t>(d + 4), rd<uint32_t>(d + 8),
                      rd<uint32_t>(d + 12));
        dates += b;
    }
    swofty::log("[f1-1990] probe: hub at %s: career 0x%llx, championship 0x%llx with %ld season(s), season %u \"%s\" with %ld "
                "weekend(s), current %u:%s; career time 0x%llx current_minutes %u (day %u), season dates%s",
                when, (unsigned long long)career, (unsigned long long)champ, seasons, index,
                season ? str_at(rd<uintptr_t>(season + 0x220)).c_str() : "?", weekends, rd<uint32_t>(season + 0x2a0),
                list.c_str(), (unsigned long long)time, rd<uint32_t>(time + 0x2b0), rd<uint32_t>(time + 0x2b0) / 1440,
                dates.c_str());
}

void on_career(const SwoftyEvent *ev, void *)
{
    probe_hub(ev->name, reinterpret_cast<uintptr_t>(swofty::career::entity()));
}

}

SWOFTY_ON_LOAD
{
    swofty::events::subscribe(SWOFTY_EVENT_CAREER_LOADED, on_career, nullptr, nullptr);
}

void f1990_probe_season(const char *when, const void *season)
{
    uintptr_t c = reinterpret_cast<uintptr_t>(season);
    if (!c)
        return;
    uintptr_t data = rd<uintptr_t>(c + 0x290), career = rd<uintptr_t>(c + 0x278);
    std::string presets;
    for (uintptr_t p = rd<uintptr_t>(data + 0xc0), e = rd<uintptr_t>(data + 0xc8); p && p < e && presets.size() < 600; p += 0x30) {
        char b[96];
        std::snprintf(b, sizeof b, " [type %u id %016llx masks %08x/%08x/%08x]", rd<uint8_t>(p + 0x18),
                      (unsigned long long)rd<uint64_t>(p + 8), rd<uint32_t>(p + 0x1c), rd<uint32_t>(p + 0x20),
                      rd<uint32_t>(p + 0x24));
        presets += b;
    }
    uint8_t want[8] = {};
    auto **cvt = swofty::mem::object_like(career) ? reinterpret_cast<void **>(rd<uintptr_t>(career)) : nullptr;
    if (cvt && swofty::mem::in_image(reinterpret_cast<uintptr_t>(cvt[0xc8 / 8])))
        reinterpret_cast<void (*)(uintptr_t, uint8_t *)>(cvt[0xc8 / 8])(career, want);
    uintptr_t cb = rd<uintptr_t>(c + 0x2b0), ce = rd<uintptr_t>(c + 0x2b8);
    long events = cb && ce >= cb ? long((ce - cb) / 0x28) : -1;
    std::string first;
    for (uintptr_t e = cb; e && e < ce && first.size() < 300; e += 0x28) {
        char b[64];
        std::snprintf(b, sizeof b, " %llx/%llx", (unsigned long long)rd<uint64_t>(e + 8), (unsigned long long)rd<uint64_t>(e + 0x10));
        first += b;
    }
    uintptr_t dyn = rd<uintptr_t>(career + 0xf58);
    uintptr_t mb = rd<uintptr_t>(dyn + 0xb8), me = rd<uintptr_t>(dyn + 0xc0);
    std::string lists;
    for (uintptr_t m = mb; m && m < me && lists.size() < 600; m += 0x28) {
        char b[64];
        uintptr_t tb = rd<uintptr_t>(m + 0x10), te = rd<uintptr_t>(m + 0x18);
        std::snprintf(b, sizeof b, " %016llx:%ld", (unsigned long long)rd<uint64_t>(m), tb && te >= tb ? long((te - tb) / 0x28) : -1L);
        lists += b;
    }
    swofty::log("[f1-1990] probe: season at %s: component 0x%llx, career wants setup mode %u(%u) season %u(%u) format %u(%u); "
                "presets%s; calendar %ld event(s) (count field %d):%s; dynamic event data 0x%llx track lists%s",
                when, (unsigned long long)c, want[0], want[1], want[2], want[3], want[4], want[5], presets.c_str(), events,
                rd<int>(c + 0x2c8), first.c_str(), (unsigned long long)dyn, lists.c_str());
}

void f1990_probe_entrants(const char *when, const void *career)
{
    uintptr_t c = reinterpret_cast<uintptr_t>(career);
    if (!c || g_step_probes++ >= 24)
        return;
    swofty::log("[f1-1990] probe: entrants at %s: career 0x%llx, +0xe18 0x%llx, %s", when, (unsigned long long)c,
                (unsigned long long)rd<uintptr_t>(c + 0xe18), records(rd<uintptr_t>(c + 0x2e0), rd<uintptr_t>(c + 0x2e8)).c_str());
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
