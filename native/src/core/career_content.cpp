#include "runtime.hpp"

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <mutex>
#include <string>

#include "career_content_logic.hpp"

std::string settings_addon(const void *from, std::string &dll);

// Extra career contents (swofty/career_content.hpp). An addon registers a content from its init;
// the core then owns everything the stock code would refuse for it:
//
// Tile (site_FeTile_CareerContentEnd, the jmp that ends FeTile_Build_List case 6, r14 = the
// TileList): FeHome_Add_CareerTile appends button_N (template, VM node, list entry); its 0x148 home
// item is replaced, the way case 6 replaces items, by a career content item
// (FeTile_Construct_CareerItem) with the model tile's route (+0x178), the content's data group
// (+0x180) and suffix (+0x188), and a copy of the model item's vtable whose populate (slot 4) shows
// the content's title and description and whose press (slot 5) selects its car group
// (g_FeTile_SelectedCareerContent), takes the model tile's menu systems (+0x150..+0x178) when its
// own are unset, and calls the router FeTile_Route_CareerItem.
//
// Router (FeTile_Route_CareerItem, wrapped): every career tile press ends in it; the wrap records
// the pressed tile's data group (pressed()), fires career.content.pressed and, after the game's
// router, hands the route to the career wizard library (career_wizard.cpp). The wrap is a core
// mixin set of its own, shared by both libraries and offered at the first mixin commit after either
// of them needs it, so a content and a wizard entry registered at different stages never wrap the
// router twice.
//
// Gate (Game_Career_IsDriverCareerMode, wrapped): 1 only for the stock 2025 and 2026 driver-career
// modes, so a mode championship whose statId (+0x2a8) is a content's mode (game_modes statId of the
// row that saves to desc.mode) is passed.
//
// Slot predicate (vt_Game_DriverCareerSlotPredicate slot 2): asked for a content's data group, a
// stock false for that content's mode championship becomes the predicate's own data group check
// (career_content_logic.hpp). Questions for any other data group keep the game's answer.
//
// The tile, gate and predicate hooks are one core mixin set, offered with the router at the first
// mixin commit after a content was registered, so a game without registered contents runs none of
// them, and a build where one of them, the router or a binding the handlers call does not resolve
// turns every content off with a log line.

namespace {

namespace content = runtime::content;
namespace mem = swofty::mem;
using swofty::game::Action;
using swofty::game::HomeItem;
using swofty::game::NeAllocator;
using swofty::game::NeStr;
using swofty::game::Tile;
using swofty::game::TileList;

constexpr const char *MIXIN = "swofty_core.dll (career contents)";
constexpr const char *ROUTE_MIXIN = "swofty_core.dll (career tile route)";
constexpr uint32_t LOG_FIRST = 6;

using AddTile = void(TileList *, uint8_t);
using Construct = HomeItem *(HomeItem *, NeAllocator *, const char *);
using Populate = void(HomeItem *, void *, uint32_t);
using Route = Action *(HomeItem *, Action *);
using Gate = bool(const void *);
using Predicate = bool(void *, void *);
using MakeStr = uint32_t(NeStr **, const char *, uint32_t, uintptr_t);
using SetString = void *(void *, void *, const char *, NeStr **, int);
using RegistryGet = void *(void *, const void *, int);
using GroupIndex = uint8_t(void *);

struct Slot {
    content::Desc desc;
    std::string owner;
    std::atomic<uint32_t> state{SWOFTY_CAREER_CONTENT_QUEUED};
    std::atomic<bool> resolved{false};
    std::atomic<int64_t> mode{-1};
    std::atomic<int64_t> group{-1};
    std::atomic<uint32_t> tiles{0}, presses{0}, gates{0}, slots{0};
    std::string why;
    void **vt = nullptr;
    content::Retry retry;
};

struct Bound {
    uintptr_t site, route, gate, predicate_vt, add_tile, construct, populate_2026, populate_2025, customisation,
        selected, setup_desc, make_str, set_string, vm_registry;
};

std::mutex lock;
Slot slots[content::MAX_CONTENTS];
std::atomic<size_t> count{0};
std::atomic<bool> offered{false}, live{false}, settled{false}, off{false};
std::atomic<bool> route_wanted{false}, route_offered{false}, route_settled{false}, route_live{false};
std::string off_why;
Bound b{};
Route *route_orig;
Gate *gate_orig;
Predicate *predicate_orig;
std::atomic<TileList *> last_list{nullptr};
std::mutex pressed_lock;
std::string pressed_group;
bool pressed_any;

std::string text(const char *s)
{
    return s ? s : "";
}

int copy_out(const std::string &s, char *out, size_t cap)
{
    if (!out || !cap)
        return SWOFTY_ASSET_INVALID;
    size_t n = s.size() < cap - 1 ? s.size() : cap - 1;
    std::memcpy(out, s.data(), n);
    out[n] = 0;
    return n == s.size() ? SWOFTY_ASSET_OK : SWOFTY_ASSET_INVALID;
}

void turn_off(Slot &s, const std::string &why)
{
    {
        std::lock_guard<std::mutex> g(lock);
        s.why = why;
    }
    s.state = SWOFTY_CAREER_CONTENT_OFF;
    swofty::log("[career] content '%s' (%s) is off: %s", s.desc.id.c_str(), s.owner.c_str(), why.c_str());
}

size_t used()
{
    return count.load(std::memory_order_acquire);
}

bool db_text(const std::string &key, std::string &out, void *)
{
    const SwoftyDb *db = runtime::db_table();
    SwoftyDbValue v{};
    if (db->get(key.c_str(), &v) != SWOFTY_ASSET_OK || !v.s)
        return false;
    out = v.s;
    return true;
}

bool db_integer(const std::string &key, int64_t &out, void *)
{
    return runtime::db_table()->int_at(key.c_str(), &out) == SWOFTY_ASSET_OK;
}

bool db_list_stat(const std::string &label, int64_t &out, void *)
{
    const SwoftyDb *db = runtime::db_table();
    SwoftyDbCursor c{};
    SwoftyDbValue v{};
    if (db->row("entrants.entrant_lists", label.c_str(), &c) != SWOFTY_ASSET_OK ||
        db->cell(&c, "stat_id", &v) != SWOFTY_ASSET_OK)
        return false;
    out = v.i;
    return true;
}

void resolve(Slot &s, bool now)
{
    if (s.resolved || s.state != SWOFTY_CAREER_CONTENT_LIVE || !runtime::db_table()->available())
        return;
    std::lock_guard<std::mutex> g(lock);
    uint64_t t = GetTickCount64();
    if (s.resolved || (!now && !s.retry.due(t)))
        return;
    content::Db db{db_text, db_integer, db_list_stat, nullptr};
    content::Requirements r = content::requirements(s.desc, db);
    s.mode = r.mode;
    s.group = r.group;
    if (!r.ok) {
        s.why = r.why;
        if (s.retry.failed(t, r.why))
            swofty::log("[career] content '%s' (%s) not ready: %s; no tile and its mode keeps the stock gate until it is, "
                        "checked again every %llu ms",
                        s.desc.id.c_str(), s.owner.c_str(), r.why.c_str(), (unsigned long long)content::RETRY_MS);
        return;
    }
    s.why.clear();
    s.resolved = true;
    if (!r.warn.empty())
        swofty::log("[career] content '%s': %s", s.desc.id.c_str(), r.warn.c_str());
    swofty::log("[career] content '%s' ready%s: mode %s statId %lld, data group %s (entrant list stat_id %lld), suffix %s "
                "(F1CAREER_%s%s%s)",
                s.desc.id.c_str(), s.retry.tries ? (" after " + std::to_string(s.retry.tries + 1) + " checks").c_str() : "",
                s.desc.mode.c_str(), (long long)r.mode, s.desc.data_group.c_str(), (long long)r.group,
                s.desc.suffix.c_str(), s.desc.suffix.c_str(), r.f2 ? ", F2CAREER" : "", r.icons ? ", ICONS" : "");
}

void resolve_all(bool now)
{
    for (size_t i = 0; i < used(); i++)
        resolve(slots[i], now);
}

bool ready(const Slot &s)
{
    return s.state == SWOFTY_CAREER_CONTENT_LIVE && s.resolved;
}

Slot *by_mode(const void *championship)
{
    if (!championship || !mem::readable(static_cast<const uint8_t *>(championship) + content::MODE_STAT_ID, 4))
        return nullptr;
    uint32_t mode = content::at<uint32_t>(championship, content::MODE_STAT_ID);
    for (size_t i = 0; i < used(); i++)
        if (ready(slots[i]) && slots[i].mode == int64_t(mode))
            return &slots[i];
    return nullptr;
}

Slot *by_vtable(const void *item)
{
    void **vt = item && mem::readable(item, 8) ? *static_cast<void **const *>(item) : nullptr;
    for (size_t i = 0; vt && i < used(); i++)
        if (slots[i].vt == vt)
            return &slots[i];
    return nullptr;
}

void *setup_component(void *championship, void *)
{
    auto *reg = static_cast<uint8_t *>(championship) + content::CHAMP_REGISTRY;
    void **vt = mem::readable(reg, 8) ? *reinterpret_cast<void ***>(reg) : nullptr;
    if (!vt || !mem::in_image(vt) || !mem::readable(vt + 2, 8) || !mem::in_image(vt[2]))
        return nullptr;
    void *c = reinterpret_cast<RegistryGet *>(vt[2])(reg, reinterpret_cast<const void *>(b.setup_desc), 1);
    return c && mem::object_like(c) ? c : nullptr;
}

int group_index(void *component, void *)
{
    void **vt = *static_cast<void ***>(component);
    auto *slot = reinterpret_cast<uint8_t *>(vt) + content::SETUP_INDEX_SLOT;
    void *fn = mem::readable(slot, 8) ? *reinterpret_cast<void **>(slot) : nullptr;
    return fn && mem::in_image(fn) ? reinterpret_cast<GroupIndex *>(fn)(component) : -1;
}

bool readable(const void *p, size_t n, void *)
{
    return mem::readable(p, n);
}

const content::Access ACCESS{setup_component, group_index, readable, nullptr};

const char *nestr(const NeStr *s)
{
    return s && mem::readable(s, content::NESTR_CHARS + 1) ? reinterpret_cast<const char *>(s) + content::NESTR_CHARS
                                                           : nullptr;
}

void set_field(void *field, const char *text)
{
    reinterpret_cast<MakeStr *>(b.make_str)(static_cast<NeStr **>(field), text, 0, 0);
}

void set_text(void *node, const char *name, const char *text)
{
    NeStr *s = nullptr;
    set_field(&s, text);
    reinterpret_cast<SetString *>(b.set_string)(*reinterpret_cast<void **>(b.vm_registry), node, name, &s, 1);
    set_field(&s, nullptr);
}

uintptr_t model_populate(uint32_t model)
{
    return model == content::MODEL_2025 ? b.populate_2025 : b.populate_2026;
}

bool is_model(const HomeItem *item, uint32_t model)
{
    void *const *vt = item && mem::readable(item, 8) ? *reinterpret_cast<void *const *const *>(item) : nullptr;
    return vt && mem::readable(vt + content::ITEM_POPULATE_SLOT, 8) &&
           reinterpret_cast<uintptr_t>(vt[content::ITEM_POPULATE_SLOT]) == model_populate(model);
}

HomeItem *find_in(TileList *list, uint32_t model, const HomeItem *self, bool *has_self)
{
    HomeItem *found = nullptr;
    if (has_self)
        *has_self = false;
    if (!list || !list->items_begin || list->items_end <= list->items_begin)
        return nullptr;
    for (auto *e = list->items_begin; e < list->items_end; e++) {
        HomeItem *item = e->tile ? e->tile->item : nullptr;
        if (has_self && item == self)
            *has_self = true;
        if (!found && is_model(item, model))
            found = item;
    }
    return found;
}

void populate(HomeItem *item, void *node, uint32_t a7)
{
    Slot *s = by_vtable(item);
    uint32_t model = s ? s->desc.model : content::MODEL_2026;
    reinterpret_cast<Populate *>(model_populate(model))(item, node, a7);
    auto *vm = static_cast<uint8_t *>(item->button_node.node);
    if (!s || !vm)
        return;
    set_text(vm + 8, "title", s->desc.title.c_str());
    set_text(vm + 8, "description", s->desc.description.c_str());
    if (model == content::MODEL_2026 || s->desc.badge) {
        auto **vt = *reinterpret_cast<void ***>(item);
        reinterpret_cast<void (*)(HomeItem *, int)>(vt[content::ITEM_NOTIFICATION_SLOT])(item, int(s->desc.badge));
    }
}

bool adopt_systems(HomeItem *item, const Slot &s)
{
    auto *raw = reinterpret_cast<uint8_t *>(item);
    if (content::at<void *>(raw, content::ITEM_SYSTEM_PROBE))
        return true;
    bool mine = false;
    HomeItem *model = find_in(last_list.load(), s.desc.model, item, &mine);
    if (!mine || !model || !content::at<void *>(model, content::ITEM_SYSTEM_PROBE))
        return false;
    std::memcpy(raw + content::ITEM_SYSTEMS, reinterpret_cast<const uint8_t *>(model) + content::ITEM_SYSTEMS,
                content::ITEM_SYSTEMS_SIZE);
    swofty::log("[career] content '%s': its tile took the %s tile's menu systems", s.desc.id.c_str(),
                s.desc.model == content::MODEL_2025 ? "2025" : "2026");
    return true;
}

Action *press(HomeItem *item, Action *out)
{
    Slot *s = by_vtable(item);
    auto *system = *reinterpret_cast<const uint8_t *const *>(b.customisation);
    const uint8_t *eb = nullptr, *ee = nullptr;
    if (system && mem::readable(system + content::SYSTEM_ENTRIES_BEGIN, 0x10)) {
        eb = content::at<const uint8_t *>(system, content::SYSTEM_ENTRIES_BEGIN);
        ee = content::at<const uint8_t *>(system, content::SYSTEM_ENTRIES_END);
        if (!eb || ee < eb || !mem::readable(eb, size_t(ee - eb)))
            eb = ee = nullptr;
    }
    content::Pick pick = s ? content::pick(eb, ee, s->desc.car_group, s->desc.model) : content::Pick{};
    *reinterpret_cast<uint32_t *>(b.selected) = pick.id;
    if (s && s->presses++ < LOG_FIRST)
        swofty::log("[career] content '%s' pressed: car group 0x%08x (%s)", s->desc.id.c_str(), pick.id,
                    pick.kind == content::Pick::exact      ? "its car_group"
                    : pick.kind == content::Pick::fallback ? "the model tile's pick"
                                                           : "no content entry");
    if (!s || !adopt_systems(item, *s)) {
        out->type = 1;
        out->str = nullptr;
        swofty::log("[career] content '%s': the tile has no menu systems and the model tile has none to copy; press "
                    "ignored",
                    s ? s->desc.id.c_str() : "?");
        return out;
    }
    return reinterpret_cast<Route *>(b.route)(item, out);
}

HomeItem *build_item(NeAllocator *alloc, HomeItem *model, Slot &s)
{
    bool y25 = s.desc.model == content::MODEL_2025;
    auto alloc_fn = reinterpret_cast<void *(*)(NeAllocator *, size_t, size_t)>(alloc->vtable->alloc);
    auto *item = static_cast<HomeItem *>(alloc_fn(alloc, y25 ? content::ITEM_SIZE_2025 : content::ITEM_SIZE_2026, 0x10));
    if (!item)
        return nullptr;
    const char *id = nestr(model->id);
    reinterpret_cast<Construct *>(b.construct)(item, alloc, id ? id : (y25 ? "career_default_content" : "career_dlc_content"));
    auto *raw = reinterpret_cast<uint8_t *>(item);
    const char *route = nestr(content::at<const NeStr *>(model, content::ITEM_ROUTE));
    set_field(raw + content::ITEM_ROUTE, route && *route ? route : "driver_career");
    if (!y25) {
        const char *ent = nestr(content::at<const NeStr *>(model, content::ITEM_ENTITLEMENT_NAME));
        std::memset(raw + content::ITEM_ENTITLEMENT, 0, 0x10);
        set_field(raw + content::ITEM_ENTITLEMENT_NAME,
                  !s.desc.entitlement.empty() ? s.desc.entitlement.c_str() : ent && *ent ? ent : "f1_2026");
    }
    set_field(raw + content::ITEM_DATA_GROUP, s.desc.data_group.c_str());
    set_field(raw + content::ITEM_SUFFIX, s.desc.suffix.c_str());
    raw[content::ITEM_IS_DLC] = 0;
    if (!s.vt) {
        auto **src = *reinterpret_cast<void ***>(model);
        auto **copy = new void *[content::ITEM_VTABLE_SLOTS + 1];
        std::memcpy(copy, src - 1, (content::ITEM_VTABLE_SLOTS + 1) * sizeof(void *));
        copy[1 + content::ITEM_POPULATE_SLOT] = reinterpret_cast<void *>(&populate);
        copy[1 + content::ITEM_PRESS_SLOT] = reinterpret_cast<void *>(&press);
        s.vt = copy + 1;
    }
    *reinterpret_cast<void ***>(item) = s.vt;
    return item;
}

void add_tile(TileList *list, Slot &s, uint8_t index)
{
    HomeItem *model = find_in(list, s.desc.model, nullptr, nullptr);
    if (!model) {
        turn_off(s, std::string("the content list has no ") + (s.desc.model == content::MODEL_2025 ? "2025" : "2026") +
                        " tile to model the tile on");
        return;
    }
    size_t before = size_t(list->items_end - list->items_begin);
    reinterpret_cast<AddTile *>(b.add_tile)(list, index);
    size_t after = list->items_begin ? size_t(list->items_end - list->items_begin) : 0;
    Tile *tile = after == before + 1 ? list->items_end[-1].tile : nullptr;
    auto *tile_alloc = tile ? content::at<NeAllocator *>(tile, content::TILE_ALLOCATOR) : nullptr;
    if (!tile || !tile->item || !tile_alloc) {
        swofty::log("[career] content '%s': the tile adder gave %zu -> %zu items; no tile", s.desc.id.c_str(), before,
                    after);
        return;
    }
    auto *alloc = content::at<NeAllocator *>(list, content::LIST_ALLOCATOR);
    HomeItem *item = alloc ? build_item(alloc, model, s) : nullptr;
    if (!item) {
        swofty::log("[career] content '%s': could not build its content item; button_%u stays a Career tile",
                    s.desc.id.c_str(), unsigned(index));
        return;
    }
    HomeItem *old = tile->item;
    tile->item = item;
    std::memcpy(reinterpret_cast<uint8_t *>(tile) + content::TILE_ALLOCATOR, &alloc, sizeof alloc);
    reinterpret_cast<void (*)(HomeItem *, int)>(reinterpret_cast<void *const *>(old->vtable)[0])(old, 0);
    reinterpret_cast<void (*)(NeAllocator *, void *)>(tile_alloc->vtable->free)(tile_alloc, old);
    if (s.tiles++ < LOG_FIRST)
        swofty::log("[career] content '%s': tile added as button_%u (items %zu -> %zu)", s.desc.id.c_str(),
                    unsigned(index), before, after);
}

void site_entry(SwoftyContext *c)
{
    auto *list = reinterpret_cast<TileList *>(c->r14);
    if (!list || !mem::readable(list, sizeof(TileList)))
        return;
    resolve_all(true);
    last_list = list;
    uint8_t index = 3;
    for (size_t i = 0; i < used(); i++)
        if (ready(slots[i]))
            add_tile(list, slots[i], index++);
}

Action *route_entry(HomeItem *item, Action *out)
{
    Slot *s = by_vtable(item);
    const char *group = item && mem::readable(reinterpret_cast<uint8_t *>(item) + content::ITEM_DATA_GROUP, 8)
                            ? nestr(content::at<const NeStr *>(item, content::ITEM_DATA_GROUP))
                            : nullptr;
    std::string g = group ? group : "";
    {
        std::lock_guard<std::mutex> l(pressed_lock);
        pressed_group = g;
        pressed_any = true;
    }
    if (used())
        runtime::events_fire(SWOFTY_EVENT_CAREER_CONTENT_PRESSED, s ? int64_t(s - slots) : -1, 0, g.c_str());
    Action *r = route_orig(item, out);
    runtime::wizard_tile_routed(g, out);
    return r;
}

bool gate_entry(const void *championship)
{
    Slot *s = used() ? by_mode(championship) : nullptr;
    if (!s)
        return gate_orig(championship);
    if (s->gates++ < LOG_FIRST)
        swofty::log("[career] content '%s': its mode (statId %lld) passes the driver-career gate", s->desc.id.c_str(),
                    (long long)s->mode.load());
    return true;
}

bool predicate_entry(void *self, void *championship)
{
    bool stock = predicate_orig(self, championship);
    if (stock || !championship || !used())
        return stock;
    Slot *s = by_mode(championship);
    auto *holder = s && mem::readable(static_cast<uint8_t *>(self) + 8, 8) ? content::at<const uint8_t *>(self, 0x08)
                                                                           : nullptr;
    const char *want = holder && mem::readable(holder, 8) ? content::at<const char *>(holder, 0x00) : nullptr;
    if (!want || !mem::readable(want, 1) || s->desc.data_group != want)
        return false;
    bool ok = content::data_group_is(championship, want, ACCESS);
    if (s->slots++ < LOG_FIRST) {
        content::GroupSeen seen = content::data_group(championship, ACCESS);
        swofty::log("[career] content '%s': save-slot predicate said no for its mode; data group %d of %zu is \"%s\", "
                    "wanted \"%s\": %s",
                    s->desc.id.c_str(), seen.index, seen.count, seen.name ? seen.name : "(none)", want,
                    ok ? "match" : "no match");
    }
    return ok;
}

uintptr_t need(const char *id, std::string &missing)
{
    uintptr_t a = swofty::api->resolve(id);
    if (!a)
        missing += std::string(missing.empty() ? "" : ", ") + id;
    return a;
}

bool bind_all(std::string &why)
{
    std::string missing;
    bool y25 = false, y26 = false;
    for (size_t i = 0; i < used(); i++)
        (slots[i].desc.model == content::MODEL_2025 ? y25 : y26) = true;
    b.site = need("site_FeTile_CareerContentEnd", missing);
    b.route = need("FeTile_Route_CareerItem", missing);
    b.gate = need("Game_Career_IsDriverCareerMode", missing);
    b.predicate_vt = need("vt_Game_DriverCareerSlotPredicate", missing);
    b.add_tile = need("FeHome_Add_CareerTile", missing);
    b.construct = need("FeTile_Construct_CareerItem", missing);
    b.populate_2026 = y26 ? need("FeTile_CareerDlcItem_Populate", missing) : 0;
    b.populate_2025 = y25 ? need("FeTile_CareerDefaultItem_Populate", missing) : 0;
    b.customisation = need("g_FeTile_CustomisationSystem", missing);
    b.selected = need("g_FeTile_SelectedCareerContent", missing);
    b.setup_desc = need("k_Career_CareerSetupComponentDesc", missing);
    b.make_str = need("Ne_Make_StrFromCStr", missing);
    b.set_string = need("Vm_Set_String", missing);
    b.vm_registry = need("g_Vm_Registry", missing);
    if (missing.empty())
        return true;
    why = "needs " + missing + ", which this build does not resolve";
    return false;
}

std::string caller(const void *from)
{
    std::string dll, id = settings_addon(from, dll);
    return id.empty() ? (dll.empty() ? std::string("unknown module") : dll) : id;
}

int api_available()
{
    return runtime::enabled("career", nullptr) && !off.load();
}

int api_add(const SwoftyCareerContentDesc *d)
{
    std::string who = caller(__builtin_return_address(0));
    if (!d || d->size < sizeof(SwoftyCareerContentDesc)) {
        swofty::log("[career] content from %s refused: no descriptor, or its size %u is below %zu", who.c_str(),
                    d ? d->size : 0, sizeof(SwoftyCareerContentDesc));
        return SWOFTY_ASSET_INVALID;
    }
    if (!runtime::enabled("career", nullptr)) {
        swofty::log("[career] content '%s' from %s refused: the career feature is off", text(d->id).c_str(), who.c_str());
        return SWOFTY_ASSET_UNAVAILABLE;
    }
    content::Desc desc{text(d->id),   text(d->title), text(d->description), text(d->data_group),
                       text(d->suffix), text(d->mode), text(d->entitlement), d->model, d->car_group, d->badge};
    std::lock_guard<std::mutex> g(lock);
    size_t n = used();
    std::string why = content::check(desc, [&](const char *field, const std::string &v) {
        for (size_t i = 0; i < n; i++) {
            const content::Desc &o = slots[i].desc;
            if ((!strcmp(field, "id") && o.id == v) || (!strcmp(field, "data_group") && o.data_group == v) ||
                (!strcmp(field, "suffix") && o.suffix == v))
                return true;
        }
        return false;
    });
    if (why.empty() && n >= content::MAX_CONTENTS)
        why = "the core holds at most 8 contents";
    if (!why.empty()) {
        swofty::log("[career] content '%s' from %s refused: %s", desc.id.c_str(), who.c_str(), why.c_str());
        return SWOFTY_ASSET_INVALID;
    }
    Slot &s = slots[n];
    s.desc = desc;
    s.owner = who;
    uint32_t state = off ? SWOFTY_CAREER_CONTENT_OFF : live ? SWOFTY_CAREER_CONTENT_LIVE : SWOFTY_CAREER_CONTENT_QUEUED;
    if (off)
        s.why = off_why;
    s.state = state;
    count.store(n + 1, std::memory_order_release);
    swofty::log("[career] content '%s' from %s registered (tile %zu after the stock ones, %s model, data group %s, suffix "
                "%s, mode %s): %s",
                desc.id.c_str(), who.c_str(), n + 1, desc.model == content::MODEL_2025 ? "2025" : "2026",
                desc.data_group.c_str(), desc.suffix.c_str(), desc.mode.c_str(),
                state == SWOFTY_CAREER_CONTENT_LIVE ? "live (hooks already installed)"
                : state == SWOFTY_CAREER_CONTENT_OFF ? ("off: " + off_why).c_str()
                                                     : "queued for the next mixin commit");
    return off ? SWOFTY_ASSET_UNAVAILABLE : SWOFTY_ASSET_OK;
}

int api_info(const char *id, SwoftyCareerContentInfo *out)
{
    if (!id || !out)
        return SWOFTY_ASSET_INVALID;
    for (size_t i = 0; i < used(); i++) {
        Slot &s = slots[i];
        if (s.desc.id != id)
            continue;
        resolve(s, true);
        std::lock_guard<std::mutex> g(lock);
        *out = {};
        out->index = uint32_t(i);
        out->state = s.state;
        out->mode_stat_id = s.mode;
        out->data_group_index = s.group;
        out->tiles = s.tiles;
        out->presses = s.presses;
        copy_out(s.desc.id, out->id, sizeof out->id);
        copy_out(s.desc.data_group, out->data_group, sizeof out->data_group);
        copy_out(s.why, out->why, sizeof out->why);
        return SWOFTY_ASSET_OK;
    }
    return SWOFTY_ASSET_INVALID;
}

int api_data_group(const void *career, char *out, size_t cap)
{
    if (!out || !cap)
        return SWOFTY_ASSET_INVALID;
    out[0] = 0;
    if (!runtime::enabled("career", nullptr))
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!b.setup_desc)
        b.setup_desc = swofty::api->resolve("k_Career_CareerSetupComponentDesc");
    if (!b.setup_desc)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!career)
        career = runtime::career_table()->entity();
    if (!career)
        return SWOFTY_ASSET_LATE;
    content::GroupSeen seen = content::data_group(career, ACCESS);
    if (!seen.name)
        return SWOFTY_ASSET_INVALID;
    return copy_out(seen.name, out, cap);
}

int api_pressed(char *out, size_t cap)
{
    if (!out || !cap)
        return SWOFTY_ASSET_INVALID;
    out[0] = 0;
    if (!live)
        return SWOFTY_ASSET_UNAVAILABLE;
    std::lock_guard<std::mutex> l(pressed_lock);
    if (!pressed_any)
        return SWOFTY_ASSET_LATE;
    return copy_out(pressed_group, out, cap);
}

const SwoftyCareerContent table = {api_available, api_add, api_info, api_data_group, api_pressed};

}

extern "C" __declspec(dllexport) const SwoftyCareerContent *SwoftyCoreCareerContent(void)
{
    return &table;
}

namespace runtime {

const SwoftyCareerContent *career_content_table()
{
    return &table;
}

std::vector<CoreMixins> career_content_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !used() || off)
        return out;
    offered = true;
    std::string why;
    if (route_settled && !route_live)
        why = "the career tile route hook did not install (see the [core] lines for " + std::string(ROUTE_MIXIN) + ")";
    if (!why.empty() || !bind_all(why)) {
        off_why = why;
        off = true;
        for (size_t i = 0; i < used(); i++)
            turn_off(slots[i], why);
        return out;
    }
    out.push_back({MIXIN,
                   {{"site_FeTile_CareerContentEnd", SWOFTY_MIXIN_SITE, 100, (void *)&site_entry, nullptr},
                    {"Game_Career_IsDriverCareerMode", SWOFTY_MIXIN_WRAP, 100, (void *)&gate_entry, (void **)&gate_orig},
                    {"vt_Game_DriverCareerSlotPredicate", SWOFTY_MIXIN_WITH_ARG(SWOFTY_MIXIN_VTABLE, 2), 100,
                     (void *)&predicate_entry, (void **)&predicate_orig}}});
    tile_route_want();
    swofty::log("[career] contents: %zu registered; offering the career content hooks", used());
    return out;
}

void tile_route_want()
{
    route_wanted = true;
}

std::vector<CoreMixins> tile_route_mixins()
{
    std::vector<CoreMixins> out;
    if (!route_wanted || route_offered)
        return out;
    route_offered = true;
    out.push_back({ROUTE_MIXIN,
                   {{"FeTile_Route_CareerItem", SWOFTY_MIXIN_WRAP, 100, (void *)&route_entry, (void **)&route_orig}}});
    return out;
}

void tile_route_commit()
{
    if (!route_offered || route_settled)
        return;
    route_live = core_mixins_active(ROUTE_MIXIN);
    route_settled = true;
    swofty::log("[career] career tile route: %s", route_live ? "armed" : "off");
}

TileRoute tile_route_state()
{
    return route_live ? TileRoute::live
           : route_settled || (route_wanted && core_mixins_closed()) ? TileRoute::off
                                                                     : TileRoute::pending;
}

void career_content_commit()
{
    if (!offered || settled || !route_settled)
        return;
    settled = true;
    if (core_mixins_active(MIXIN) && route_live) {
        live = true;
        for (size_t i = 0; i < used(); i++)
            if (slots[i].state == SWOFTY_CAREER_CONTENT_QUEUED)
                slots[i].state = SWOFTY_CAREER_CONTENT_LIVE;
        swofty::log("[career] contents: hooks installed; %zu content(s) live, their data is checked once the database "
                    "has loaded",
                    used());
        return;
    }
    off_why = "the career content hooks did not install (see the [core] lines for " +
              std::string(route_live ? MIXIN : ROUTE_MIXIN) + ")";
    off = true;
    for (size_t i = 0; i < used(); i++)
        if (slots[i].state != SWOFTY_CAREER_CONTENT_OFF)
            turn_off(slots[i], off_why);
}

void career_content_tick()
{
    if (live && used())
        resolve_all(false);
}

}
