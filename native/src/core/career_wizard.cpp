#include "career_ui.hpp"

#include <swofty/game.hpp>

#include <atomic>
#include <mutex>
#include <set>
#include <string>
#include <vector>

// Career wizard control (SwoftyCareerWizard). Keyed by the data group of the career content tile the
// wizard opened from; the wizard state carries it at +0x128 (copied from the tile item's +0x180).
//
// - Game_CareerWizard_StartsInF1(entrants, wizard data, data group): answers yes for a registered
//   group, so the wizard has no Skip to F1 / Start in F2 step and no team-icons step and the driver
//   select takes the F1 path.
// - FeTile_Route_CareerItem(item, out): every career tile press ends here. The core's career tile
//   route wrap (career_content.cpp), shared with the career content library, hands each press here
//   after the game's router: the tile's data group is recorded and, for a registered group, the
//   route the tile fires (out type 1) is replaced.
// - Game_CareerWizard_PushNextStep(wizard): runs right after the add-option body pushed the
//   player's option byte on wizard +0x100..+0x108 and pushes the next step byte on +0xe0..+0xe8.
//   Before it runs, a forced option rewrites that last option byte; after it, a replaced step
//   rewrites the step it pushed.
// - site_Game_CareerWizardConfirm_NextRoute: rdx is the route the confirm step picked, rsi the view
//   model (+0x270 the wizard state); a registered route swap changes rdx.
// - site_Game_DriverSelectHeader_BuildTabs: the call to the shared tab builder for the Choose
//   Driver header gets a copy of the content group (entries +0x08..+0x10, 0x30 bytes each, tab name
//   string +0x08 with its text at +0x09) without the hidden tabs, for the last tile's group while no
//   career of another group is loaded. Game_DriverSelectHeader_SetTabState then skips an index past
//   the header's tabs (vector at header +0x2b8, 0x70 each), which it has no bounds check for.
//
// Each control is a core mixin set of its own (manager.cpp), offered only once an addon registers an
// entry for it: at the next mixin commit for a registration from an addon's init, right after the
// ON_LOAD handlers for one made there. A game where no addon registers runs none of them, so an
// addon that wraps these functions itself keeps its wraps. A binding that does not resolve drops
// only that control.

namespace {

namespace cu = runtime::career_ui;
namespace mem = swofty::mem;
using swofty::game::Action;
constexpr uintptr_t NESTR_CHARS = 0x09;

constexpr const char *START_MIXIN = "swofty_core.dll (wizard start in F1)";
constexpr const char *STEP_MIXIN = "swofty_core.dll (wizard steps)";
constexpr const char *CONFIRM_MIXIN = "swofty_core.dll (wizard confirm route)";
constexpr const char *TABS_MIXIN = "swofty_core.dll (wizard driver tabs)";
constexpr uintptr_t WIZARD_GROUP = 0x128, STEPS = 0xe0, OPTIONS = 0x100, VM_WIZARD = 0x270, HEADER_TABS = 0x2b8;
constexpr size_t TAB_ITEM = 0x70, CONTENT_TAB = 0x30, MAX_TABS = 64;

using StartsInF1 = bool(void *entrants, void *wizard_data, const char *group);
using PushStep = void(void *wizard);
using BuildTabs = int64_t(void *tabs, void *alloc, void *node, const void *group);
using SetTabState = void(void *header, uint64_t tab, uint8_t content_visible, uint8_t enabled);

enum Kind { START, TILE_ROUTE, STEP, OPTION, CONFIRM_ROUTE, TAB };

struct Entry {
    Kind kind;
    std::string group, from, to;
    uint32_t a, b;
    HMODULE owner;
};

StartsInF1 *orig_starts;
PushStep *orig_push;
BuildTabs *orig_tabs;
SetTabState *orig_tab_state;

std::mutex lock;
std::vector<Entry> entries;
std::set<std::string> interned;
std::string last_tile;
std::vector<uint8_t> tab_copy;
std::atomic<bool> any{false};
std::atomic<uint32_t> logged[8];

struct Part {
    const char *name;
    std::atomic<bool> wanted{false}, offered{false}, settled{false}, live{false}, late_logged{false};
};

constexpr int ROUTE = 1, TABS = 4;
Part parts[5] = {{START_MIXIN}, {nullptr}, {STEP_MIXIN}, {CONFIRM_MIXIN}, {TABS_MIXIN}};
const char *const part_names[5] = {"start in F1", "tile route", "steps and options", "confirm route", "driver tabs"};

const char *intern(const std::string &s)
{
    return interned.insert(s).first->c_str();
}

void note(int which, const char *fmt, const char *a = "", const char *b = "", const char *c = "")
{
    if (logged[which]++ < 6)
        swofty::log(fmt, a, b, c);
}

std::string read_cstr(uintptr_t p)
{
    std::string s;
    if (p)
        mem::c_string(p, s, 128);
    return s;
}

std::string nestr(uintptr_t p)
{
    return p ? read_cstr(p + NESTR_CHARS) : std::string();
}

std::string wizard_group(const void *wizard)
{
    uintptr_t g = 0;
    return wizard && mem::read(reinterpret_cast<uintptr_t>(wizard) + WIZARD_GROUP, g) ? read_cstr(g) : std::string();
}

template <class F> bool find(Kind kind, const std::string &group, F &&match)
{
    if (!any.load() || group.empty())
        return false;
    std::lock_guard<std::mutex> g(lock);
    for (auto it = entries.rbegin(); it != entries.rend(); ++it)
        if (it->kind == kind && it->group == group && match(*it))
            return true;
    return false;
}

bool starts_in_f1(void *entrants, void *wizard_data, const char *group)
{
    bool stock = orig_starts(entrants, wizard_data, group);
    std::string g = read_cstr(reinterpret_cast<uintptr_t>(group));
    if (!stock && find(START, g, [](const Entry &) { return true; })) {
        note(0, "[wizard] %s starts in F1 (no F2 choice, no team-icons step)", g.c_str());
        return true;
    }
    return stock;
}

bool last_byte(const void *wizard, uintptr_t vec, uintptr_t &at, size_t &count)
{
    uintptr_t b = 0, e = 0;
    if (!mem::read(reinterpret_cast<uintptr_t>(wizard) + vec, b) || !mem::read(reinterpret_cast<uintptr_t>(wizard) + vec + 8, e) ||
        !b || e <= b || e - b > 256)
        return count = 0, false;
    count = e - b;
    at = e - 1;
    return true;
}

void push_step(void *wizard)
{
    if (!any.load() || !wizard)
        return orig_push(wizard);
    std::string g = wizard_group(wizard);
    uintptr_t at = 0;
    size_t before = 0;
    uint32_t with = 0;
    if (last_byte(wizard, OPTIONS, at, before)) {
        uint8_t option = *reinterpret_cast<uint8_t *>(at);
        if (find(OPTION, g, [&](const Entry &e) { return e.a == option && (with = e.b, true); }) && with != option) {
            *reinterpret_cast<uint8_t *>(at) = uint8_t(with);
            char a[8], b[8];
            snprintf(a, sizeof a, "%u", option), snprintf(b, sizeof b, "%u", with);
            note(2, "[wizard] %s: option %s recorded as %s", g.c_str(), a, b);
        }
    }
    last_byte(wizard, STEPS, at, before);
    orig_push(wizard);
    size_t after = 0;
    if (!last_byte(wizard, STEPS, at, after) || after != before + 1)
        return;
    uint8_t step = *reinterpret_cast<uint8_t *>(at);
    if (find(STEP, g, [&](const Entry &e) { return e.a == step && (with = e.b, true); }) && with != step) {
        *reinterpret_cast<uint8_t *>(at) = uint8_t(with);
        char a[8], b[8];
        snprintf(a, sizeof a, "%u", step), snprintf(b, sizeof b, "%u", with);
        note(3, "[wizard] %s: step %s replaced by step %s", g.c_str(), a, b);
    }
}

void confirm_route(SwoftyContext *c)
{
    if (!any.load() || !c->rdx || !c->rsi)
        return;
    uintptr_t wizard = 0;
    if (!mem::read(c->rsi + VM_WIZARD, wizard) || !wizard)
        return;
    std::string g = wizard_group(reinterpret_cast<void *>(wizard)), route = read_cstr(c->rdx);
    const char *to = nullptr;
    if (find(CONFIRM_ROUTE, g, [&](const Entry &e) {
            if (!e.from.empty() && e.from != route)
                return false;
            to = intern(e.to);
            return true;
        })) {
        c->rdx = reinterpret_cast<uintptr_t>(to);
        note(4, "[wizard] %s: confirm opens %s instead of %s", g.c_str(), to, route.c_str());
    }
}

std::string tab_name(const uint8_t *entry)
{
    uintptr_t s = 0;
    return mem::read(reinterpret_cast<uintptr_t>(entry) + 0x08, s) && s ? read_cstr(s + 9) : std::string();
}

int64_t build_tabs(void *tabs, void *alloc, void *node, const void *group)
{
    std::string g;
    {
        std::lock_guard<std::mutex> lk(lock);
        g = last_tile;
    }
    if (!any.load() || !group || g.empty() || !find(TAB, g, [](const Entry &) { return true; }))
        return orig_tabs(tabs, alloc, node, group);
    std::string loaded = cu::loaded_group();
    if (!loaded.empty() && loaded != g)
        return orig_tabs(tabs, alloc, node, group);
    struct Group {
        uint8_t head[0x08];
        const uint8_t *begin, *end;
    } copy;
    if (!mem::read(group, copy) || !copy.begin || copy.end < copy.begin || size_t(copy.end - copy.begin) % CONTENT_TAB ||
        size_t(copy.end - copy.begin) / CONTENT_TAB > MAX_TABS) {
        note(5, "[wizard] %s: the Choose Driver header group does not look like a content group; its tabs stay", g.c_str());
        return orig_tabs(tabs, alloc, node, group);
    }
    std::lock_guard<std::mutex> lk(lock);
    tab_copy.clear();
    size_t dropped = 0, total = size_t(copy.end - copy.begin) / CONTENT_TAB;
    for (const uint8_t *p = copy.begin; p < copy.end; p += CONTENT_TAB) {
        std::string name = tab_name(p);
        bool hide = false;
        for (auto &e : entries)
            hide |= e.kind == TAB && e.group == g && e.from == name;
        if (hide)
            dropped++;
        else
            tab_copy.insert(tab_copy.end(), p, p + CONTENT_TAB);
    }
    if (!dropped)
        return orig_tabs(tabs, alloc, node, group);
    copy.begin = tab_copy.data();
    copy.end = tab_copy.data() + tab_copy.size();
    char a[16], b[16];
    snprintf(a, sizeof a, "%zu", total - dropped), snprintf(b, sizeof b, "%zu", total);
    note(6, "[wizard] %s: Choose Driver header tabs %s of %s", g.c_str(), a, b);
    return orig_tabs(tabs, alloc, node, &copy);
}

void tab_state(void *header, uint64_t tab, uint8_t content_visible, uint8_t enabled)
{
    uintptr_t b = 0, e = 0;
    if (header && mem::read(reinterpret_cast<uintptr_t>(header) + HEADER_TABS, b) &&
        mem::read(reinterpret_cast<uintptr_t>(header) + HEADER_TABS + 8, e) && b && e >= b &&
        tab >= size_t(e - b) / TAB_ITEM) {
        char a[24];
        snprintf(a, sizeof a, "%llu", (unsigned long long)tab);
        note(7, "[wizard] Choose Driver tab %s state skipped, past the header's tabs", a);
        return;
    }
    orig_tab_state(header, tab, content_visible, enabled);
}

bool part_live(int i)
{
    return i == ROUTE ? runtime::tile_route_state() == runtime::TileRoute::live : parts[i].live.load();
}

bool part_usable(int i)
{
    if (i == ROUTE)
        return runtime::tile_route_state() != runtime::TileRoute::off;
    return parts[i].live || (!parts[i].settled && !runtime::core_mixins_closed());
}

int need(int i, bool want)
{
    if (!runtime::enabled("career", nullptr))
        return SWOFTY_ASSET_UNAVAILABLE;
    if (want) {
        parts[i].wanted = true;
        if (i == ROUTE || i == TABS) {
            parts[ROUTE].wanted = true;
            runtime::tile_route_want();
        }
    }
    if (part_usable(i) && (i != TABS || part_usable(ROUTE)))
        return SWOFTY_ASSET_OK;
    if (want && runtime::core_mixins_closed() && !parts[i].offered && !parts[i].late_logged.exchange(true))
        swofty::log("[wizard] %s registered after the last mixin commit; its hook is not installed", part_names[i]);
    return SWOFTY_ASSET_UNAVAILABLE;
}

int put(Kind kind, const char *group, std::string from, const char *to, uint32_t a, uint32_t b, bool on, HMODULE owner,
        int part)
{
    if (!cu::valid_name(group) || (to && *to && !cu::valid_name(to)))
        return SWOFTY_ASSET_INVALID;
    {
        std::lock_guard<std::mutex> g(lock);
        std::erase_if(entries, [&](const Entry &e) {
            return e.kind == kind && e.group == group && e.from == from && e.a == a && e.owner == owner;
        });
        if (on)
            entries.push_back({kind, group, std::move(from), to ? to : "", a, b, owner});
        any = !entries.empty();
    }
    return need(part, on);
}

int api_available()
{
    if (!runtime::enabled("career", nullptr))
        return 0;
    bool tried = false;
    for (int i = 0; i < 5; i++) {
        if (part_live(i))
            return 1;
        tried |= i == ROUTE ? runtime::tile_route_state() == runtime::TileRoute::off : parts[i].settled.load();
    }
    return !tried;
}

int api_group(char *out, size_t cap)
{
    std::lock_guard<std::mutex> g(lock);
    return cu::copy_out(last_tile, out, cap);
}

int api_starts_in_f1(const char *group, int on)
{
    return put(START, group, "", nullptr, 0, 0, on, cu::caller(__builtin_return_address(0)), 0);
}

int api_tile_route(const char *group, const char *from, const char *to)
{
    return put(TILE_ROUTE, group, from ? from : "", to, 0, 0, to && *to, cu::caller(__builtin_return_address(0)), 1);
}

int api_replace_step(const char *group, uint32_t step, uint32_t with)
{
    if (step > 0xff || with > 0xff)
        return SWOFTY_ASSET_INVALID;
    return put(STEP, group, "", nullptr, step, with, with != step, cu::caller(__builtin_return_address(0)), 2);
}

int api_force_option(const char *group, uint32_t option, uint32_t with)
{
    if (option > 0xff || with > 0xff)
        return SWOFTY_ASSET_INVALID;
    return put(OPTION, group, "", nullptr, option, with, with != option, cu::caller(__builtin_return_address(0)), 2);
}

int api_confirm_route(const char *group, const char *from, const char *to)
{
    return put(CONFIRM_ROUTE, group, from ? from : "", to, 0, 0, to && *to, cu::caller(__builtin_return_address(0)), 3);
}

int api_hide_driver_tab(const char *group, const char *tab, int on)
{
    if (!cu::valid_name(tab))
        return SWOFTY_ASSET_INVALID;
    return put(TAB, group, tab, nullptr, 0, 0, on, cu::caller(__builtin_return_address(0)), 4);
}

int api_clear(const char *group)
{
    if (!cu::valid_name(group))
        return SWOFTY_ASSET_INVALID;
    HMODULE owner = cu::caller(__builtin_return_address(0));
    std::lock_guard<std::mutex> g(lock);
    std::erase_if(entries, [&](const Entry &e) { return e.group == group && e.owner == owner; });
    any = !entries.empty();
    return SWOFTY_ASSET_OK;
}

const SwoftyCareerWizard table = {api_available,     api_group,         api_starts_in_f1,
                                  api_tile_route,    api_replace_step,  api_force_option,
                                  api_confirm_route, api_hide_driver_tab, api_clear};

}

namespace runtime {

const SwoftyCareerWizard *wizard_table()
{
    return &table;
}

std::vector<CoreMixins> wizard_mixins()
{
    std::vector<CoreMixins> out;
    if (!enabled("career", nullptr))
        return out;
    auto offer = [&](int i) { return parts[i].wanted && !parts[i].offered.exchange(true); };
    if (offer(0))
        out.push_back(
            {START_MIXIN,
             {{"Game_CareerWizard_StartsInF1", SWOFTY_MIXIN_WRAP, 100, (void *)&starts_in_f1, (void **)&orig_starts}}});
    if (offer(2))
        out.push_back(
            {STEP_MIXIN,
             {{"Game_CareerWizard_PushNextStep", SWOFTY_MIXIN_WRAP, 100, (void *)&push_step, (void **)&orig_push}}});
    if (offer(3))
        out.push_back(
            {CONFIRM_MIXIN,
             {{"site_Game_CareerWizardConfirm_NextRoute", SWOFTY_MIXIN_SITE, 100, (void *)&confirm_route, nullptr}}});
    if (offer(4))
        out.push_back({TABS_MIXIN,
                       {{"site_Game_DriverSelectHeader_BuildTabs", SWOFTY_MIXIN_REDIRECT, 100, (void *)&build_tabs,
                         (void **)&orig_tabs},
                        {"Game_DriverSelectHeader_SetTabState", SWOFTY_MIXIN_WRAP, 100, (void *)&tab_state,
                         (void **)&orig_tab_state}}});
    if (parts[ROUTE].wanted)
        parts[ROUTE].offered = true;
    return out;
}

void wizard_commit()
{
    bool any = false;
    for (int i = 0; i < 5; i++) {
        Part &p = parts[i];
        if (!p.offered || p.settled || (i == ROUTE && tile_route_state() == TileRoute::pending))
            continue;
        p.live = i == ROUTE ? tile_route_state() == TileRoute::live : core_mixins_active(p.name);
        p.settled = any = true;
    }
    auto state = [](int i) { return parts[i].live ? "armed" : parts[i].settled ? "off" : "not used"; };
    if (any)
        swofty::log("[wizard] setup: start in F1 %s, tile route %s, steps and options %s, confirm route %s, driver tabs %s",
                    state(0), state(1), state(2), state(3), state(4));
}

void wizard_tile_routed(const std::string &g, void *o)
{
    auto *out = static_cast<Action *>(o);
    {
        std::lock_guard<std::mutex> lk(lock);
        last_tile = g;
    }
    if (!out || out->type != 1)
        return;
    std::string route = nestr(reinterpret_cast<uintptr_t>(out->str)), to;
    if (find(TILE_ROUTE, g, [&](const Entry &e) {
            if (!e.from.empty() && e.from != route)
                return false;
            to = e.to;
            return true;
        })) {
        swofty::game::Ne_Make_StrFromCStr(&out->str, to.c_str(), 0, 0);
        note(1, "[wizard] %s tile opens %s instead of %s", g.c_str(), to.c_str(), route.c_str());
    }
}

}
