#include "runtime.hpp"

#include <windows.h>

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "layout.hpp"
#include "save_keep_logic.hpp"

// Unknown save container keeper. The profile save is one tree of save-data nodes under the root
// save_game (SaveDataSystem+0x20, vt_Game_SaveDataRootContainer). Every game mode whose row sets
// register_save_container hangs its own container, named by save_data.name, under that root, and the
// mode's slot manager inside it holds every career slot of the mode. A career content addon with its own
// mode container (driver_career_progressive, driver_career_1990) is therefore a top-level child of the
// root. In a session without that addon nothing registers the child: Game_SaveData_DeserialiseContainer
// drops its bag at the profile load, and the next profile save writes the root without it, so every
// career of the addon is gone (re/db/subsystems/game.md#save-containers).
//
// The engine already knows how to keep such bags: a node whose retain_unknown bool is true copies every
// child bag that no registered child claims into its retained map at load, and its serialise writes each
// one back unchanged under its own name. The root is built with the bool false. The wrap of
// Game_SaveData_DeserialiseContainer turns it on for the root right before each root load, so an absent
// addon's container rides through the session untouched and is read normally once the addon is back.
// The game never opens a retained bag. SWOFTY_KEEP_UNKNOWN_SAVES=0 turns the keeper off.

namespace {

namespace mem = swofty::mem;
namespace g = swofty::game;
namespace keep = runtime::save_keep;
namespace layout = runtime::layout;

using Deserialise_t = int *(*)(void *self, int *result, void *bag, uint8_t reset_on_error, uint64_t groups);

constexpr const char *MIXIN = "swofty_core.dll (save keeper)";
constexpr const char *TARGET = "Game_SaveData_DeserialiseContainer";
constexpr const char *ROOT_VT = "vt_Game_SaveDataRootContainer";

Deserialise_t orig = nullptr;
uintptr_t root_vt = 0;
bool offered = false, settled = false, live = false;
std::atomic<uint32_t> loads{0};

template <class C = g::SaveDataContainer> bool arm(uintptr_t self, uint64_t &kept_before)
{
    if constexpr (layout::save_container<C>) {
        uintptr_t flag = self + offsetof(C, retain_unknown);
        uint64_t key = 0;
        if (!mem::read(self + offsetof(C, retain_unknown_key), key) || !mem::readable(flag, 1) ||
            !mem::read(self + offsetof(C, retained_count), kept_before))
            return false;
        *reinterpret_cast<volatile uint8_t *>(flag) = keep::true_byte(key);
        uint8_t now = 0;
        return mem::read(flag, now) && keep::flag(now, key);
    } else {
        (void)self, (void)kept_before;
        return false;
    }
}

template <class C = g::SaveDataContainer> bool kept(uintptr_t self, uint64_t &count)
{
    if constexpr (layout::save_container<C>) {
        return mem::read(self + offsetof(C, retained_count), count);
    } else {
        (void)self, (void)count;
        return false;
    }
}

bool is_root(void *self)
{
    uintptr_t vt = 0;
    return root_vt && self && mem::read(reinterpret_cast<uintptr_t>(self), vt) && vt == root_vt;
}

int *deserialise_entry(void *self, int *result, void *bag, uint8_t reset_on_error, uint64_t groups)
{
    if (!is_root(self))
        return orig(self, result, bag, reset_on_error, groups);
    uint32_t n = ++loads;
    uint64_t before = 0;
    bool armed = arm(reinterpret_cast<uintptr_t>(self), before);
    int *r = orig(self, result, bag, reset_on_error, groups);
    int code = r && mem::readable(r, 4) ? *r : -1;
    uint64_t count = 0;
    if (!armed)
        swofty::log("[saves] profile load %u: could not turn on keeping unknown save containers on the save root "
                    "%p; the game drops containers no loaded mode claims",
                    n, self);
    else if (kept(reinterpret_cast<uintptr_t>(self), count) && count)
        swofty::log("[saves] profile load %u (result %d): kept %llu save container(s) that no loaded game mode "
                    "claims, such as careers of an addon that is off; the next profile save writes them back unchanged",
                    n, code, (unsigned long long)count);
    else
        swofty::log("[saves] profile load %u (result %d): every save container is claimed by a loaded mode", n, code);
    return r;
}

}

namespace runtime {

std::vector<CoreMixins> save_keep_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_KEEP_UNKNOWN_SAVES", v, sizeof v);
    if (!keep::enabled(n && n < sizeof v ? v : nullptr)) {
        swofty::log("[saves] unknown save container keeper off (SWOFTY_KEEP_UNKNOWN_SAVES=%s)", v);
        return out;
    }
    if (!layout::SAVE_CONTAINER) {
        swofty::log("[saves] unknown save container keeper off: this core was built without the SaveDataContainer "
                    "layout");
        return out;
    }
    root_vt = swofty::api->resolve(ROOT_VT);
    if (!root_vt || !swofty::api->resolve(TARGET)) {
        swofty::log("[saves] unknown save container keeper off: needs %s and %s, which this build does not resolve",
                    TARGET, ROOT_VT);
        return out;
    }
    out.push_back({MIXIN, {{TARGET, SWOFTY_MIXIN_WRAP, 100, (void *)&deserialise_entry, (void **)&orig}}});
    return out;
}

void save_keep_commit()
{
    if (!offered || settled || !root_vt)
        return;
    settled = true;
    live = core_mixins_active(MIXIN);
    swofty::log("[saves] unknown save container keeper %s", live ? "armed: save containers of absent addons are "
                                                                   "kept through each profile load and save"
                                                                 : "did not install (see the [core] lines)");
}

}
