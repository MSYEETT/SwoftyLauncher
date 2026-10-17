#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstdint>
#include <cstring>
#include "safetyhook.hpp"
#include "swofty_api.h"

namespace {

constexpr const char *SECTION = ".xtls";
constexpr const char *SIG_HOME_ANCHOR = "45 8D 67 01 4C 89 F1 44 89 E2 E8 ?? ?? ?? ?? F6";
constexpr const char *SIG_BUILDER =
    "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC 88 02 00 00 C5 F8 29 B4 24 70 02 00 00 4C 89 8C 24";
constexpr const char *SIG_BASE_POPULATE =
    "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC 88 00 00 00 C5 F8 29 74 24 70 44 89 44";
constexpr const char *SIG_NESTR_FROM = "41 57 41 56 41 54 56 57 55 53 48 83 EC 50 44 89 C6 48 89 CF";
constexpr uint8_t HOOK_SITE_BYTES[] = {0xF6, 0x46, 0x05, 0x04, 0x74};

constexpr const char *ROUTE = "mod_menu";
constexpr const char *TITLE = "MOD SETTINGS";
constexpr const char *DESCRIPTION = "Configure the mods installed by Swofty Mod Launcher";
constexpr size_t VT_SLOTS = 32;

struct NeStr;
struct Action {
    int32_t type;
    int32_t pad;
    NeStr *str;
};

using AdderFn = void (*)(void *list, uint32_t index);
using PopulateFn = void (*)(uint8_t *item, void *node, uint32_t a7);
using NeStrFromFn = uint32_t (*)(NeStr **out, const char *s, uint32_t flags, uintptr_t len);
using VmSetFn = void *(*)(void *registry, void *parent, const char *name, void *value, int overwrite);

const SwoftyApi *api;
AdderFn career_adder;
PopulateFn base_populate;
NeStrFromFn nestr_from;
VmSetFn vm_set_string, vm_set_bool;
void **registry;
safetyhook::MidHook hook;
void *vt_storage[VT_SLOTS + 1];
void **vt_source;
NeStr *title_str, *description_str;
unsigned hook_calls, populate_calls, action_calls;

uintptr_t rel32(uintptr_t at)
{
    return at + 4 + *reinterpret_cast<const int32_t *>(at);
}

bool bytes_at(uintptr_t at, const uint8_t *want, size_t n, const char *what)
{
    if (memcmp(reinterpret_cast<const void *>(at), want, n) == 0)
        return true;
    api->log("[core] %s: unexpected bytes at 0x%llx", what, (unsigned long long)at);
    return false;
}

void mod_populate(uint8_t *item, void *node, uint32_t a7)
{
    unsigned call = ++populate_calls;
    base_populate(item, node, a7);
    auto vm_node = *reinterpret_cast<uint8_t **>(item + 0x20);
    if (!vm_node) {
        api->log("[core] populate #%u item=%p: no VM node after base populate, label left as is", call, item);
        return;
    }
    if (!title_str) {
        nestr_from(&title_str, TITLE, 0, 0);
        nestr_from(&description_str, DESCRIPTION, 0, 0);
    }
    void *reg = *registry, *parent = vm_node + 8;
    uint32_t no = 0;
    vm_set_string(reg, parent, "title", &title_str, 1);
    vm_set_bool(reg, parent, "title_localised", &no, 1);
    vm_set_string(reg, parent, "description", &description_str, 1);
    vm_set_bool(reg, parent, "description_localised", &no, 1);
    api->log("[core] populate #%u item=%p node=%p a7=%u vm=%p: title='%s' description set, localised=false", call,
             item, node, a7, vm_node, TITLE);
}

Action *mod_get_action(uint8_t *item, Action *out)
{
    out->type = 1;
    out->str = nullptr;
    nestr_from(&out->str, ROUTE, 0, 0);
    api->log("[core] getAction #%u item=%p -> {type 1, route '%s'}", ++action_calls, item, ROUTE);
    return out;
}

uint8_t *last_item(uint8_t *list, size_t *count)
{
    auto begin = *reinterpret_cast<uint8_t **>(list + 0x78);
    auto end = *reinterpret_cast<uint8_t **>(list + 0x80);
    if (!begin || end <= begin || (end - begin) % 16)
        return nullptr;
    *count = (end - begin) / 16;
    auto tile = *reinterpret_cast<uint8_t **>(end - 8);
    return tile ? *reinterpret_cast<uint8_t **>(tile + 0xd0) : nullptr;
}

void after_career(safetyhook::Context &c)
{
    unsigned call = ++hook_calls;
    auto list = reinterpret_cast<uint8_t *>(c.r14);
    api->log("[core] hook #%u: list(r14)=0x%llx r12=0x%llx r15=0x%llx rsi=0x%llx rip=0x%llx", call,
             (unsigned long long)c.r14, (unsigned long long)c.r12, (unsigned long long)c.r15,
             (unsigned long long)c.rsi, (unsigned long long)c.rip);
    size_t before = 0, after = 0;
    uint8_t *career = last_item(list, &before);
    if (!career) {
        api->log("[core] hook #%u: items vector does not end with a tile, nothing added", call);
        return;
    }
    void **career_vt = *reinterpret_cast<void ***>(career);
    uint32_t index = (static_cast<uint8_t>(c.r12) + 1) & 0xff;
    career_adder(list, index);
    uint8_t *item = last_item(list, &after);
    c.r12 += 1;
    c.r15 += 1;
    if (!item || after != before + 1 || *reinterpret_cast<void ***>(item) != career_vt) {
        api->log("[core] hook #%u: adder result unexpected (items %zu -> %zu, item %p, vt %p vs career %p); tile "
                 "left as a plain Career copy",
                 call, before, after, item, item ? *reinterpret_cast<void **>(item) : nullptr, career_vt);
        return;
    }
    if (vt_source != career_vt) {
        memcpy(vt_storage, career_vt - 1, sizeof vt_storage);
        vt_storage[1 + 4] = reinterpret_cast<void *>(mod_populate);
        vt_storage[1 + 5] = reinterpret_cast<void *>(mod_get_action);
        vt_source = career_vt;
        api->log("[core] hook #%u: built mod vtable %p from career vtable %p (slot 4 populate %p -> %p, slot 5 "
                 "getAction %p -> %p)",
                 call, vt_storage + 1, career_vt, career_vt[4], vt_storage[5], career_vt[5], vt_storage[6]);
    }
    *reinterpret_cast<void ***>(item) = vt_storage + 1;
    auto id = reinterpret_cast<NeStr **>(item + 0x10);
    NeStr *old_id = *id, *new_id = nullptr;
    nestr_from(&new_id, ROUTE, 0, 0);
    *id = new_id;
    api->log("[core] hook #%u: added button_%u (items %zu -> %zu) item=%p, id %p -> %p '%s', r12 -> 0x%llx, r15 -> "
             "0x%llx",
             call, index, before, after, item, old_id, new_id, ROUTE, (unsigned long long)c.r12,
             (unsigned long long)c.r15);
}

bool find(const char *what, const char *pattern, uintptr_t *out)
{
    int hits = api->scan(SECTION, pattern, out);
    api->log("[core] signature %s: %d hit(s)%s", what, hits, hits == 1 ? "" : " -> FAIL");
    return hits == 1;
}

} // namespace

extern "C" __declspec(dllexport) int SwoftyModInit(const SwoftyApi *a)
{
    api = a;
    if (!a || a->version != SWOFTY_API_VERSION)
        return 0;
    uintptr_t base = api->module_base();
    api->log("[core] swofty_core init, game base 0x%llx", (unsigned long long)base);

    uintptr_t anchor = 0, builder = 0, populate = 0, nestr = 0;
    bool ok = find("home_anchor", SIG_HOME_ANCHOR, &anchor);
    ok &= find("tile_list_builder", SIG_BUILDER, &builder);
    ok &= find("item_base_populate", SIG_BASE_POPULATE, &populate);
    ok &= find("nestr_from_cstr", SIG_NESTR_FROM, &nestr);
    if (!ok) {
        api->log("[core] a signature did not match exactly once; installing nothing");
        return 0;
    }

    static const uint8_t call_op[] = {0xE8}, mov_rcx_rip[] = {0x48, 0x8B, 0x0D};
    uintptr_t site = anchor + 0xF;
    if (anchor < builder || anchor - builder > 0x8000) {
        api->log("[core] home anchor 0x%llx is not inside the builder at 0x%llx; installing nothing",
                 (unsigned long long)anchor, (unsigned long long)builder);
        return 0;
    }
    if (!bytes_at(site, HOOK_SITE_BYTES, sizeof HOOK_SITE_BYTES, "hook site") ||
        !bytes_at(populate + 0x53, mov_rcx_rip, 3, "registry load") ||
        !bytes_at(populate + 0x69, call_op, 1, "VmSetString call") ||
        !bytes_at(populate + 0x28e, call_op, 1, "VmSetBool call")) {
        api->log("[core] derived-offset check failed; installing nothing");
        return 0;
    }

    career_adder = reinterpret_cast<AdderFn>(rel32(anchor + 0xB));
    base_populate = reinterpret_cast<PopulateFn>(populate);
    nestr_from = reinterpret_cast<NeStrFromFn>(nestr);
    registry = reinterpret_cast<void **>(rel32(populate + 0x56));
    vm_set_string = reinterpret_cast<VmSetFn>(rel32(populate + 0x6a));
    vm_set_bool = reinterpret_cast<VmSetFn>(rel32(populate + 0x28f));
    auto rva = [base](auto p) { return (unsigned long long)(reinterpret_cast<uintptr_t>(p) - base); };
    api->log("[core] builder base+0x%llx, anchor base+0x%llx, hook site base+0x%llx", rva(builder), rva(anchor),
             rva(site));
    api->log("[core] career_adder base+0x%llx, base_populate base+0x%llx, nestr_from base+0x%llx", rva(career_adder),
             rva(base_populate), rva(nestr_from));
    api->log("[core] registry global base+0x%llx (= %p now), vm_set_string base+0x%llx, vm_set_bool base+0x%llx",
             rva(registry), *registry, rva(vm_set_string), rva(vm_set_bool));

    auto created = safetyhook::MidHook::create(reinterpret_cast<void *>(site), after_career);
    if (!created) {
        auto &e = created.error();
        api->log("[core] MidHook at 0x%llx FAILED: error type %d (inline type %d, ip %p); nothing installed",
                 (unsigned long long)site, (int)e.type, (int)e.inline_hook_error.type,
                 (void *)e.inline_hook_error.ip);
        return 0;
    }
    hook = std::move(*created);
    api->log("[core] MidHook installed at 0x%llx (base+0x%llx); MOD SETTINGS tile goes under Career on the next "
             "home build",
             (unsigned long long)site, rva(site));
    return 1;
}
