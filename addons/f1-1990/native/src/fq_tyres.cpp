// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - dry tyres in Friday qualifying (its own DLL, so a binding it can't resolve drops only this hook).
//
// Friday qualifying runs as a sprint-shootout session (ids 10..14). In tyre allocation mode 0 the
// "set usable now" check (0x142e88730) has no case for those sessions, so only untagged sets
// (inters and wets) can be fitted, and the session start (Sess_TyreAllocation_OnSessionStart) hands
// sets back. After that session start, on a 1990 weekend, every unused and undamaged dry set of
// every car loses its handed-back flag and its session tag (tag 0 is usable in any session), so
// Friday and Saturday qualifying share the unused dry sets. The 1990 weekend structure exists only in
// the f1_1990 setup mode, so 2025/2026 sessions are untouched. Setting fq_tyre_fix turns it off.
//
// Tyre fallback (setting tyre_fallback): some cars of a 26-car 1990 session found no compound to fit
// ("Unable to get Closest Available Compound for vehicle %u") and ran without tyres. At every 1990
// session start, after the game's own session start, a car allocation with no sets at all gets a
// copy of the sets of the fullest allocation, as new sets. When Sess_Tyres_ClosestAvailableCompound
// still finds nothing for a car of a 1990 session, the same refill runs, or, for a car whose fresh
// sets are all tagged for other sessions, those sets lose the tag; then the lookup runs again. Every
// step is logged with the allocation's set counts.

#include <atomic>
#include <bitset>
#include <cstdio>
#include <iterator>
#include <mutex>

#include <swofty/events.hpp>
#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include "fq_tyres.hpp"
#include "weekend.hpp"

SWOFTY_SETTING(bool, fq_tyre_fix, true);
SWOFTY_SETTING(bool, tyre_fallback, true);

namespace {

using f1990::at;
namespace t = f1990::tyres;

constexpr uint32_t kNoCompound = 0xffffffff;
constexpr size_t kEntrantSize = 0x88;
constexpr size_t kEntrantMax = 256;

std::mutex g_lock;
void *g_component;
bool g_1990;
uint32_t g_session;
std::bitset<256> g_reported;
int g_lines;

void *call_getter(void *obj, size_t slot)
{
    auto **vt = *static_cast<void ***>(obj);
    return vt && vt[slot] ? reinterpret_cast<void *(*)(void *)>(vt[slot])(obj) : nullptr;
}

void *heap_alloc(void *allocator, size_t bytes)
{
    auto **vt = *static_cast<void ***>(allocator);
    return reinterpret_cast<void *(*)(void *, size_t, size_t)>(vt[0x18 / 8])(allocator, bytes, 4);
}

void heap_release(void *allocator, void *p)
{
    auto **vt = *static_cast<void ***>(allocator);
    reinterpret_cast<void (*)(void *, void *)>(vt[0x38 / 8])(allocator, p);
}

const t::Heap kHeap{heap_alloc, heap_release};

unsigned key(uint32_t id)
{
    return ((id & 0xff) << 14 | id >> 16) & 0xffff;
}

bool sentinel(uint32_t id)
{
    return (id & 0xffff0000) == 0x3fff0000;
}

struct Cars {
    uint8_t *b = nullptr, *e = nullptr;

    explicit Cars(void *component)
    {
        if (!swofty::mem::readable(static_cast<uint8_t *>(component) + 0x2b0, 0x10))
            return;
        auto *pb = at<uint8_t *>(component, 0x2b0), *pe = at<uint8_t *>(component, 0x2b8);
        if (pb && pe > pb && size_t(pe - pb) % 0x10 == 0 && swofty::mem::readable(pb, size_t(pe - pb)))
            b = pb, e = pe;
    }

    template <class F> void each(F f) const
    {
        for (uint8_t *c = b; c < e; c += 0x10) {
            auto *alloc = at<uint8_t *>(c, 0x08);
            if (alloc && swofty::mem::readable(alloc, 0xf0) && !sentinel(at<uint32_t>(alloc, 0)))
                f(alloc);
        }
    }

    uint8_t *find(uint32_t id) const
    {
        uint8_t *found = nullptr;
        each([&](uint8_t *alloc) {
            uint32_t a = at<uint32_t>(alloc, 0);
            if (!found && uint8_t(a) == uint8_t(id) && uint16_t(a >> 16) == uint16_t(id >> 16))
                found = alloc;
        });
        return found;
    }

    uint8_t *donor(size_t *sets) const
    {
        uint8_t *best = nullptr;
        *sets = 0;
        each([&](uint8_t *alloc) {
            size_t n = t::census(alloc).total;
            if (n > *sets)
                best = alloc, *sets = n;
        });
        return best;
    }
};

struct Entrants {
    const uint8_t *b = nullptr, *e = nullptr;

    explicit Entrants(void *component)
    {
        if (!swofty::mem::readable(static_cast<uint8_t *>(component) + 0x278, 0x08))
            return;
        auto *career = at<uint8_t *>(component, 0x278);
        if (!career || !swofty::mem::readable(career + 0x2e0, 0x10))
            return;
        auto *pb = at<const uint8_t *>(career, 0x2e0), *pe = at<const uint8_t *>(career, 0x2e8);
        if (pb && pe > pb && size_t(pe - pb) % kEntrantSize == 0 && size_t(pe - pb) / kEntrantSize <= kEntrantMax &&
            swofty::mem::readable(pb, size_t(pe - pb)))
            b = pb, e = pe;
    }

    uint32_t id(const uint8_t *r) const { return at<uint8_t>(r, 0x50) | uint32_t(at<uint16_t>(r, 0x52)) << 16; }

    const uint8_t *vehicle(uint8_t v) const
    {
        for (const uint8_t *r = b; r < e; r += kEntrantSize)
            if (at<uint8_t>(r, 0x82) == v)
                return r;
        return nullptr;
    }
};

void describe(const t::Census &c, char *out, size_t n)
{
    std::snprintf(out, n, "%zu set(s), %zu fresh, %zu fresh tagged; per slot S %zu/%zu M %zu/%zu H %zu/%zu I %zu/%zu W %zu/%zu",
                  c.total, c.fresh, c.tagged, c.fresh_sets[0], c.sets[0], c.fresh_sets[1], c.sets[1], c.fresh_sets[2],
                  c.sets[2], c.fresh_sets[3], c.sets[3], c.fresh_sets[4], c.sets[4]);
}

int refill(const Cars &cars, uint8_t *alloc, const char *why)
{
    size_t donor_sets;
    uint8_t *donor = cars.donor(&donor_sets);
    if (!donor) {
        swofty::log("[f1-1990] tyres: allocation %04x (%s) has no sets and no allocation has any to copy",
                    key(at<uint32_t>(alloc, 0)), why);
        return 0;
    }
    int copied = t::copy_sets(alloc, donor, kHeap);
    swofty::log("[f1-1990] tyres: allocation %04x (preset %u, %s) had no sets: %d set(s) copied from %04x (%zu set(s))%s",
                key(at<uint32_t>(alloc, 0)), at<uint32_t>(alloc, 4), why, copied < 0 ? 0 : copied,
                key(at<uint32_t>(donor, 0)), donor_sets, copied < 0 ? "; an allocation failed" : "");
    return copied;
}

void audit(void *component, uint32_t session)
{
    Cars cars(component);
    Entrants entrants(component);
    size_t count = 0, lo = SIZE_MAX, hi = 0;
    uint8_t *empty[32];
    size_t empties = 0;
    cars.each([&](uint8_t *alloc) {
        size_t n = t::census(alloc).total;
        count++;
        lo = std::min(lo, n);
        hi = std::max(hi, n);
        if (!n && empties < std::size(empty))
            empty[empties++] = alloc;
    });
    size_t with_car = 0, missing = 0;
    for (const uint8_t *r = entrants.b; r < entrants.e; r += kEntrantSize) {
        if (at<uint8_t>(r, 0x82) == 0xff)
            continue;
        with_car++;
        if (!cars.find(entrants.id(r)) && missing++ < 8)
            swofty::log("[f1-1990] tyres: session %u: V%02u (driver %04x) has no weekend allocation", session,
                        at<uint8_t>(r, 0x82), key(entrants.id(r)));
    }
    swofty::log("[f1-1990] tyres: session %u: %zu car allocation(s) for %zu car(s), %zu set(s) per car at least, %zu at "
                "most; %zu empty, %zu car(s) without one",
                session, count, with_car, count ? lo : 0, hi, empties, missing);
    for (size_t i = 0; i < empties; i++)
        refill(cars, empty[i], "session start");
}

}

SWOFTY_WRAP(Sess_TyreAllocation_OnSessionStart, void, void *component)
{
    orig(component);
    if (!component)
        return;
    void *provider = at<void *>(component, 0x270);
    void *session = provider ? call_getter(provider, 0xa8 / 8) : nullptr;
    void *weekend = provider ? call_getter(provider, 0x98 / 8) : nullptr;
    bool is_1990 = session && weekend && f1990::is_1990_weekend(weekend);
    {
        std::lock_guard lock(g_lock);
        g_component = component;
        g_1990 = is_1990;
        g_session = session ? at<uint32_t>(session, 0x08) : 0;
        g_reported.reset();
        if (is_1990 && tyre_fallback)
            audit(component, g_session);
    }
    if (!is_1990 || !fq_tyre_fix || !f1990::tyres::is_shootout_session(at<uint32_t>(session, 0x08)))
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

SWOFTY_WRAP(Sess_Tyres_ClosestAvailableCompound, uint32_t, void *tyre_rules, uint8_t vehicle, const uint32_t *preference,
            float wear_scale)
{
    uint32_t slot = orig(tyre_rules, vehicle, preference, wear_scale);
    if (slot != kNoCompound || !tyre_fallback)
        return slot;
    std::lock_guard lock(g_lock);
    if (!g_component || !g_1990)
        return slot;
    Cars cars(g_component);
    Entrants entrants(g_component);
    const uint8_t *entrant = entrants.vehicle(vehicle);
    uint8_t *alloc = entrant ? cars.find(entrants.id(entrant)) : nullptr;
    bool report = !g_reported.test(vehicle) && g_lines < 64;
    g_reported.set(vehicle);
    g_lines += report;
    bool same_cars = swofty::mem::readable(static_cast<uint8_t *>(g_component) + 0x2d8, 0x08) &&
                     at<void *>(tyre_rules, 0xd8) == at<void *>(g_component, 0x2d8);
    if (!alloc) {
        if (report)
            swofty::log("[f1-1990] tyres: session %u: V%02u (driver %04x) found no compound and has no weekend "
                        "allocation (%s car list)",
                        g_session, vehicle, entrant ? key(entrants.id(entrant)) : 0xffff, same_cars ? "same" : "other");
        return slot;
    }
    t::Census before = t::census(alloc);
    char text[200];
    describe(before, text, sizeof text);
    const char *action = "nothing to change";
    bool changed = false;
    if (!before.total) {
        changed = refill(cars, alloc, "no compound found") > 0;
        action = changed ? "refilled" : "refill failed";
    } else if (before.fresh && before.tagged) {
        changed = t::untag_fresh_sets(alloc) > 0;
        action = changed ? "fresh sets untagged" : "nothing to change";
    }
    uint32_t again = changed ? orig(tyre_rules, vehicle, preference, wear_scale) : slot;
    if (report)
        swofty::log("[f1-1990] tyres: session %u: V%02u (driver %04x) found no compound: %s (%s car list); %s; "
                    "lookup again: %d",
                    g_session, vehicle, key(entrants.id(entrant)), text, same_cars ? "same" : "other", action,
                    again == kNoCompound ? -1 : int(again));
    return again;
}

namespace {

void on_unloaded(const SwoftyEvent *, void *)
{
    std::lock_guard lock(g_lock);
    g_component = nullptr;
    g_1990 = false;
}

}

SWOFTY_ON_LOAD
{
    swofty::log("[f1-1990] FQ tyres loaded: Friday qualifying dry sets %s, tyre fallback %s",
                fq_tyre_fix ? "on" : "off (fq_tyre_fix)", tyre_fallback ? "on" : "off (tyre_fallback)");
    uint32_t token = 0;
    if (swofty::events::subscribe(SWOFTY_EVENT_CAREER_UNLOADED, on_unloaded, nullptr, &token) != swofty::events::Result::ok)
        swofty::log("[f1-1990] tyres: no career.unloaded event; the fallback keeps the last weekend's allocations");
}

SWOFTY_ADDON()
