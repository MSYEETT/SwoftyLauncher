// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - dry tyres in Friday qualifying (its own DLL, so a binding it can't resolve drops only this hook).
//
// Friday qualifying runs as a sprint-shootout session (ids 10..14). In tyre allocation mode 0 the
// "set usable now" check (0x142e88730) has no case for those sessions, so only untagged sets
// (inters and wets) can be fitted, and the session start (Sess_TyreAllocation_OnSessionStart) hands
// sets back. After that session start, on a 1990 weekend, every unused and undamaged dry set of
// every car loses its handed-back flag and its session tag (tag 0 is usable in any session), so
// Friday and Saturday qualifying share the unused dry sets. Setting fq_tyre_fix turns it off.

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "fq_tyres.hpp"
#include "weekend.hpp"

SWOFTY_SETTING(bool, fq_tyre_fix, true);

namespace {

using f1990::at;

void *call_getter(void *obj, size_t slot)
{
    auto **vt = *static_cast<void ***>(obj);
    return vt && vt[slot] ? reinterpret_cast<void *(*)(void *)>(vt[slot])(obj) : nullptr;
}

}

SWOFTY_WRAP(Sess_TyreAllocation_OnSessionStart, void, void *component)
{
    orig(component);
    if (!component || !fq_tyre_fix)
        return;
    void *provider = at<void *>(component, 0x270);
    void *session = provider ? call_getter(provider, 0xa8 / 8) : nullptr;
    if (!session || !f1990::tyres::is_shootout_session(at<uint32_t>(session, 0x08)))
        return;
    void *weekend = call_getter(provider, 0x98 / 8);
    if (!weekend || !f1990::is_1990_weekend(weekend))
        return;
    auto *b = at<uint8_t *>(component, 0x2b0), *e = at<uint8_t *>(component, 0x2b8);
    int cars = 0, sets = 0;
    for (; b && b < e; b += 0x10) {
        auto *alloc = at<uint8_t *>(b, 0x08);
        if (!alloc)
            continue;
        cars++;
        sets += f1990::tyres::free_unused_dry_sets(alloc);
    }
    swofty::log("[f1-1990] FQ tyres: session %u: %d unused dry set(s) made usable on %d car allocation(s)",
                at<uint32_t>(session, 0x08), sets, cars);
}

SWOFTY_ON_LOAD
{
    swofty::log("[f1-1990] FQ tyres loaded: Friday qualifying dry sets %s", fq_tyre_fix ? "on" : "off (fq_tyre_fix)");
}

SWOFTY_ADDON()
