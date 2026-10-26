#include "runtime.hpp"

#include <safetyhook.hpp>
#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cctype>
#include <cstring>
#include <string>
#include <vector>

// Team HQ hub tabs (swofty/hub.hpp, SwoftyHub). An addon's settings.json entry carries `hub_tab`
// {name, title, options, data_group} (from its manifest); careers filed under data_group get one more
// Team HQ laptop tab after STANDINGS with one page whose content template is swofty_hub_<name>.
//
// The laptop's tabs are built natively by Career_Build_HubLaptop (0x141c3f130) when the hub loads, one
// block per primary tab. A primary tab is allocated through the laptop's allocator (laptop +0x2a8,
// vt+0x18, 0xb68 bytes), constructed by 0x1414c0860 (tab, laptop+0x28, laptop+0x268, laptop, content
// "tabbing_secondary_controller", header "tabbing_secondary_tab", lng name, icon, 1) and pushed with its
// key onto the laptop's tab vector object (laptop +0x2b0 {allocator, begin +0x2b8, end +0x2c0,
// capacity}, 0x18-byte entries {u32 key, allocator, tab}) by 0x141e5dfb0, which takes the {allocator,
// tab} pair over. A page (sub-tab) is allocated through the tab's allocator (tab +0x270, 0x660 bytes),
// constructed by the tab view model constructor 0x1414bdac0 (page, laptop+0x28, laptop+0x268, laptop,
// content template, lng name, 0, "", page type, flag) and added by 0x1414c13e0 (tab, page). The STANDINGS
// block (0x141c420dc..0x141c4261e) is the pattern. The new tab is added at 0x141c42623, where every
// branch of the STANDINGS block joins and the builder starts registering each tab's view model node,
// so the new tab gets the same registration as the stock ones.
//
// The page is a plain tab view model whose vtable is copied with slot 10 (onCommandRaised) replaced:
// swofty_next:<addon>:<option> / swofty_prev:<addon>:<option> raised by its rows step the option the
// way ADDON SETTINGS does (the state's command through settings_command), anything else goes to the
// game's handler; the launcher's page raises them on its template root, so they reach this view model.
// The page's data is published at view_model_data.hud_swofty_hub_<name>: title, count and o<k>.label /
// value (the current state's text) / description, again after every change of one of its options. The tab's title is the lng key lng_swofty_hub_<name>, set to the title in capitals.
//
// The R&D tab block is skipped by the builder when the laptop's r_and_d_visible property (laptop
// +0x920, read through [rsp+0x168]) does not hold a true bool: `cmp byte [rax], 0; je 0x141c41cf2`
// at 0x141c41bac. While an addon holds hide_rnd, rax points at a zero byte there, so the builder takes
// its own skip. The laptop's other R&D lookups by the tab key 0xb88a669 (0x141c43aa5 availability,
// 0x141c44d79 / 0x141c44dd9 / 0x141c44df0 the tab jump) already handle the tab being absent, which is
// the stock state when r_and_d_visible is false.

bool settings_command(uint32_t hash);
bool settings_state(const std::string &addon, const std::string &option, size_t &current,
                    std::vector<std::string> *texts, std::vector<std::string> *commands, std::vector<std::string> *values);
bool ui_vm_set_path(const std::string &path, int type, const void *value);

namespace {

struct HubOption {
    std::string id, label, description;
};

struct HubTab {
    std::string addon, name, title, group, lng, content, data;
    std::vector<HubOption> options;
    uint32_t key = 0;
};

constexpr uintptr_t LAPTOP_ARG2 = 0x28, LAPTOP_ALLOC = 0x2a8, LAPTOP_ARG3 = 0x268, LAPTOP_TABS = 0x2b0,
                    LAPTOP_TABS_END = 0x2c0, TAB_ALLOC = 0x270;
constexpr size_t TAB_SIZE = 0xb68, TAB_ALIGN = 0x10, PAGE_SIZE = 0x660, PAGE_ALIGN = 8, VT_COPY = 64, VT_COMMAND = 10;
constexpr uint32_t PAGE_TYPE = 5;
constexpr char ICON[] = "overrides.career.laptop.page_icons[4]";

const runtime::Sig S_RND_GATE{"laptop R&D gate", "80 38 00 0F 84 ?? ?? ?? ?? 4C 8B BE 68 02 00 00", 0x1c41bac};
const runtime::Sig S_TABS_END{"laptop tabs end", "48 89 DA E8 ?? ?? ?? ?? 4C 8B AE B8 02 00 00", 0x1c42623,
                              runtime::Sig::addr, 0, 8, 0};
constexpr const char *P_STANDINGS = "E8 ?? ?? ?? ?? 48 89 5C 24 60 4C 89 6C 24 68 48 8D 94 24 B0 00 00 00 4C 8D 44 24 60 "
                                    "48 8B 8C 24 00 01 00 00 E8 ?? ?? ?? ?? 48 8B 86 C0 02 00 00 48 8B 68 F8";
const runtime::Sig S_TAB_CTOR{"laptop tab constructor", P_STANDINGS, 0x14c0860, runtime::Sig::rel32, 0, 1, 5};
const runtime::Sig S_TAB_PUSH{"laptop tab push", P_STANDINGS, 0x1e5dfb0, runtime::Sig::rel32, 0x24, 0x25, 5};
const runtime::Sig S_PAGE_CTOR{"tab view model constructor",
                               "C6 44 24 48 00 C7 44 24 40 05 00 00 00 48 C7 44 24 30 00 00 00 00 48 89 D9 4C 89 EA 4D "
                               "89 F8 49 89 F1 E8 ?? ?? ?? ?? 48 8D 05",
                               0x14bdac0, runtime::Sig::rel32, 0x22, 0x23, 5};
const runtime::Sig S_PAGE_ADD{"laptop tab add page", "48 89 DA E8 ?? ?? ?? ?? 4C 8B BE 68 02 00 00", 0x14c13e0,
                              runtime::Sig::rel32, 3, 4, 5};

using Alloc_t = void *(void *allocator, size_t size, size_t align);
using TabCtor_t = void(void *tab, void *a2, void *a3, void *laptop, const char *content, const char *header,
                       const char *name, const char *icon, uint8_t flag);
using Push_t = void(void *tabs, const uint32_t *key, void *owned);
using PageCtor_t = void(void *page, void *a2, void *a3, void *owner, const char *content, const char *name, void *a7,
                        const char *a8, uint32_t page_type, uint8_t flag);
using AddPage_t = void(void *tab, void *page);
using Command_t = uint64_t(void *vm, swofty::game::NeStr **name, const void *data);

std::vector<HubTab> tabs;
std::atomic<bool> hide{false}, live{false};
const uint8_t zero_byte = 0;
uintptr_t a_tab_ctor, a_push, a_page_ctor, a_add_page;
safetyhook::MidHook h_end, h_rnd;
void **page_vt, **page_vt_src;
Command_t *page_cmd;
std::atomic<uint32_t> told{0};

void note(const char *fmt, const char *a = "", const char *b = "", const char *c = "")
{
    if (told.fetch_add(1) < 48)
        swofty::log(fmt, a, b, c);
}

template <class T> T at(uintptr_t p)
{
    T v{};
    swofty::mem::read(p, v);
    return v;
}

void publish(const HubTab &t)
{
    std::string base = t.data;
    if (!ui_vm_set_path(base + ".title", 0, t.title.c_str())) {
        note("[hub] %s: %s did not resolve; the page shows no data", t.name.c_str(), base.c_str());
        return;
    }
    int32_t count = int32_t(t.options.size());
    ui_vm_set_path(base + ".count", 2, &count);
    for (size_t k = 0; k < t.options.size(); k++) {
        const HubOption &o = t.options[k];
        size_t current = 0;
        std::vector<std::string> texts;
        settings_state(t.addon, o.id, current, &texts, nullptr, nullptr);
        std::string row = base + ".o" + std::to_string(k);
        ui_vm_set_path(row + ".label", 0, o.label.c_str());
        ui_vm_set_path(row + ".value", 0, current < texts.size() ? texts[current].c_str() : "");
        ui_vm_set_path(row + ".description", 0, o.description.c_str());
    }
}

bool step(const HubTab &t, const std::string &option, bool next)
{
    size_t current = 0;
    std::vector<std::string> commands;
    if (!settings_state(t.addon, option, current, nullptr, &commands, nullptr) || commands.empty())
        return false;
    size_t n = commands.size(), want = next ? (current + 1) % n : (current + n - 1) % n;
    uint32_t h = 5381;
    for (const char *s = commands[want].c_str(); *s; ++s)
        h = h * 33 + uint8_t(*s);
    return settings_command(h);
}

uint64_t page_command(void *vm, swofty::game::NeStr **name, const void *data)
{
    std::string text;
    if (name && *name)
        swofty::mem::c_string(reinterpret_cast<uintptr_t>(*name) + 9, text, 160);
    bool next = text.rfind("swofty_next:", 0) == 0, prev = text.rfind("swofty_prev:", 0) == 0;
    if (next || prev) {
        std::string rest = text.substr(12);
        size_t colon = rest.find(':');
        std::string addon = rest.substr(0, colon), option = colon == std::string::npos ? "" : rest.substr(colon + 1);
        for (auto &t : tabs) {
            if (t.addon != addon)
                continue;
            for (auto &o : t.options)
                if (o.id == option) {
                    bool ok = step(t, option, next);
                    swofty::log("[hub] %s: %s %s.%s%s", t.name.c_str(), next ? "next" : "prev", addon.c_str(),
                                option.c_str(), ok ? "" : " (no such option state)");
                    return 0;
                }
        }
    }
    return page_cmd ? page_cmd(vm, name, data) : 0;
}

bool own_vtable(void *page)
{
    void **src = *static_cast<void ***>(page);
    if (!page_vt) {
        page_vt_src = src;
        auto **copy = new void *[VT_COPY + 1];
        std::memcpy(copy, src - 1, (VT_COPY + 1) * sizeof(void *));
        page_cmd = reinterpret_cast<Command_t *>(copy[1 + VT_COMMAND]);
        copy[1 + VT_COMMAND] = reinterpret_cast<void *>(&page_command);
        page_vt = copy + 1;
    }
    if (src != page_vt_src)
        return false;
    *static_cast<void ***>(page) = page_vt;
    return true;
}

void *alloc_through(uintptr_t holder)
{
    void *allocator = at<void *>(holder);
    if (!allocator || !swofty::mem::object_like(allocator))
        return nullptr;
    auto fn = at<Alloc_t *>(at<uintptr_t>(reinterpret_cast<uintptr_t>(allocator)) + 0x18);
    return swofty::mem::in_image(reinterpret_cast<uintptr_t>(fn)) ? allocator : nullptr;
}

void *allocate(void *allocator, size_t size, size_t align)
{
    auto fn = at<Alloc_t *>(at<uintptr_t>(reinterpret_cast<uintptr_t>(allocator)) + 0x18);
    return fn(allocator, size, align);
}

void add_tab(uintptr_t laptop, HubTab &t)
{
    void *allocator = alloc_through(laptop + LAPTOP_ALLOC);
    if (!allocator) {
        note("[hub] %s: the laptop has no allocator; tab not added", t.name.c_str());
        return;
    }
    void *tab = allocate(allocator, TAB_SIZE, TAB_ALIGN);
    if (!tab)
        return;
    void *a2 = at<void *>(laptop + LAPTOP_ARG2), *a3 = at<void *>(laptop + LAPTOP_ARG3);
    reinterpret_cast<TabCtor_t *>(a_tab_ctor)(tab, a2, a3, reinterpret_cast<void *>(laptop),
                                              "tabbing_secondary_controller", "tabbing_secondary_tab", t.lng.c_str(),
                                              ICON, 1);
    struct {
        void *allocator, *obj;
    } owned{allocator, tab};
    uint32_t key = t.key;
    reinterpret_cast<Push_t *>(a_push)(reinterpret_cast<void *>(laptop + LAPTOP_TABS), &key, &owned);
    uintptr_t end = at<uintptr_t>(laptop + LAPTOP_TABS_END);
    if (!end || at<void *>(end - 8) != tab) {
        note("[hub] %s: the tab is not the laptop's last tab after the push; no page added", t.name.c_str());
        return;
    }
    void *page_alloc = alloc_through(reinterpret_cast<uintptr_t>(tab) + TAB_ALLOC);
    void *page = page_alloc ? allocate(page_alloc, PAGE_SIZE, PAGE_ALIGN) : nullptr;
    if (!page) {
        note("[hub] %s: no page allocated", t.name.c_str());
        return;
    }
    reinterpret_cast<PageCtor_t *>(a_page_ctor)(page, a2, a3, reinterpret_cast<void *>(laptop), t.content.c_str(),
                                                t.lng.c_str(), nullptr, "", PAGE_TYPE, 0);
    bool mine = own_vtable(page);
    reinterpret_cast<AddPage_t *>(a_add_page)(tab, page);
    publish(t);
    swofty::log("[hub] %s: tab '%s' added after STANDINGS (key 0x%08x, page %s%s, data %s)", t.name.c_str(),
                t.title.c_str(), t.key, t.content.c_str(), mine ? "" : ", its commands stay the game's",
                t.data.c_str());
}

void tabs_end(safetyhook::Context &c)
{
    uintptr_t laptop = c.rsi;
    if (!laptop || !swofty::mem::object_like(laptop))
        return;
    char group[64] = {};
    if (runtime::career_content_table()->data_group(nullptr, group, sizeof group) != SWOFTY_ASSET_OK)
        return;
    for (auto &t : tabs)
        if (t.group == group)
            add_tab(laptop, t);
}

void rnd_gate(safetyhook::Context &c)
{
    if (!hide.load())
        return;
    c.rax = reinterpret_cast<uintptr_t>(&zero_byte);
    note("[hub] laptop: the R&D tab is left out (hide_rnd)");
}

std::string upper(std::string s)
{
    for (auto &ch : s)
        ch = char(std::toupper(uint8_t(ch)));
    return s;
}

void load()
{
    std::string text = runtime::read_file(runtime::under_swofty("settings.json"), size_t(16) << 20);
    json::Value doc;
    if (text.empty() || !json::parse(text, doc))
        return;
    for (auto &m : doc.field("addons", "mods").a) {
        const json::Value &h = m["hub_tab"];
        if (h.type != json::Value::Obj || (m["enabled"].type == json::Value::Bool && !m["enabled"].b))
            continue;
        HubTab t;
        t.addon = m["id"].str();
        t.name = h["name"].str();
        t.title = h["title"].str(m["name"].str(t.addon.c_str()).c_str());
        t.group = h["data_group"].str();
        if (t.name.empty() || t.group.empty())
            continue;
        t.lng = "lng_swofty_hub_" + t.name;
        t.content = "swofty_hub_" + t.name;
        t.data = "view_model_data.hud_swofty_hub_" + t.name;
        t.key = runtime::fnv32(t.content.c_str());
        for (auto &id : h["options"].a)
            for (auto &o : m["options"].a)
                if (o["id"].str() == id.str())
                    t.options.push_back({id.str(), o["label"].str(id.str().c_str()), o["description"].str()});
        tabs.push_back(std::move(t));
    }
}

void setup()
{
    load();
    if (tabs.empty())
        return;
    a_tab_ctor = runtime::find(S_TAB_CTOR, "hub tabs", "hub");
    a_push = runtime::find(S_TAB_PUSH, "hub tabs", "hub");
    a_page_ctor = runtime::find(S_PAGE_CTOR, "hub tabs", "hub");
    a_add_page = runtime::find(S_PAGE_ADD, "hub tabs", "hub");
    uintptr_t end = runtime::find(S_TABS_END, "hub tabs", "hub"), gate = runtime::find(S_RND_GATE, "hub tabs", "hub");
    if (!a_tab_ctor || !a_push || !a_page_ctor || !a_add_page || !end || !gate) {
        swofty::log("[hub] a laptop target did not resolve; no hub tab");
        return;
    }
    auto e = safetyhook::MidHook::create(reinterpret_cast<void *>(end), tabs_end);
    auto g = safetyhook::MidHook::create(reinterpret_cast<void *>(gate), rnd_gate);
    if (!e || !g) {
        swofty::log("[hub] hooking the laptop builder failed; no hub tab");
        return;
    }
    h_end = std::move(*e);
    h_rnd = std::move(*g);
    if (const SwoftyLang *l = runtime::lang_table())
        for (auto &t : tabs)
            l->override_set("*", t.lng.c_str(), upper(t.title).c_str());
    live = true;
    for (auto &t : tabs)
        swofty::log("[hub] %s: tab '%s' for careers of %s, %zu option row(s) from %s", t.name.c_str(), t.title.c_str(),
                    t.group.c_str(), t.options.size(), t.addon.c_str());
}

int api_available()
{
    return live.load() ? 1 : 0;
}

int api_hide_rnd(int on)
{
    if (hide.exchange(on != 0) != (on != 0))
        swofty::log("[hub] R&D tab %s from the next Team HQ entry", on ? "hidden" : "shown");
    return SWOFTY_ASSET_OK;
}

int api_set_option(const char *addon, const char *option, const char *value)
{
    if (!addon || !option || !value)
        return SWOFTY_ASSET_INVALID;
    size_t current = 0;
    std::vector<std::string> commands, values;
    if (!settings_state(addon, option, current, nullptr, &commands, &values))
        return SWOFTY_ASSET_INVALID;
    for (size_t i = 0; i < values.size(); i++)
        if (values[i] == value) {
            if (i == current)
                return SWOFTY_ASSET_OK;
            uint32_t h = 5381;
            for (const char *s = commands[i].c_str(); *s; ++s)
                h = h * 33 + uint8_t(*s);
            return settings_command(h) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_INVALID;
        }
    return SWOFTY_ASSET_INVALID;
}

const SwoftyHub hub_table = {SWOFTY_HUB_VERSION, api_available, api_hide_rnd, api_set_option};

}

void hub_option_changed(const std::string &addon, const std::string &option)
{
    if (!live.load())
        return;
    for (auto &t : tabs) {
        if (t.addon != addon)
            continue;
        for (auto &o : t.options)
            if (o.id == option) {
                publish(t);
                break;
            }
    }
}

extern "C" __declspec(dllexport) const SwoftyHub *SwoftyCoreHub(void)
{
    return &hub_table;
}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("hub tabs: laptop builder hooks", setup);
}
