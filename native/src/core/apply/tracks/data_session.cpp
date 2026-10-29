#include "box.hpp"
#include "fuel_plan.hpp"

#include "../applier.hpp"
#include "../../runtime.hpp"

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstring>
#include <map>
#include <mutex>

// track.data (Applies::session) and the active box's per_package fuel, through the fuel writer's pattern
// (career_ops/track_fuel.cpp): at each session load the core reads the track data sets (TrackDataSystem from the
// championship entity's +0xf58) and the session's query (entity +0x2b0 setup mode stat id byte +0x80, entity
// +0xe18 the selected entrant list's u16 season at +0xb0; Trk_ChampionshipEntity_TrackDataQuery_Body, V1631), then
// writes the box's burn per lap on the session's track in the sets that query reads. Without a box entry, a
// track.data record <track>/<data set> whose fuel differs from the set's is written into that named set. Box writes
// are put back on the next box press, on leaving the box and on career unload; track.data writes when the session
// ends. Each write keeps the bytes it found and puts back only what still holds the bytes the core wrote. Only
// fuel is written: the other track.data fields take effect at the next start (D4).

namespace swofty_core::apply::tracks {

namespace {

namespace pf = ::career_ops::fuel;

constexpr uintptr_t ENTITY_MODE = 0x2b0, MODE_STAT = 0x80, ENTITY_LIST = 0xe18, LIST_SEASON = 0xb0;

struct GameMem {
    bool read(uintptr_t at, void *out, size_t n)
    {
        if (!at || !swofty::mem::readable(at, n))
            return false;
        std::memcpy(out, reinterpret_cast<const void *>(at), n);
        return true;
    }
    bool c_string(uintptr_t at, std::string &out) { return swofty::mem::c_string(at, out, 64); }
};

bool writable(uintptr_t at, size_t n)
{
    if (!swofty::mem::readable(at, n))
        return false;
    uintptr_t p = at, end = at + n;
    while (p < end) {
        MEMORY_BASIC_INFORMATION mbi{};
        if (!VirtualQuery(reinterpret_cast<const void *>(p), &mbi, sizeof mbi) || mbi.State != MEM_COMMIT)
            return false;
        DWORD prot = mbi.Protect & 0xff;
        if (mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS) || !(prot == PAGE_READWRITE || prot == PAGE_EXECUTE_READWRITE))
            return false;
        uintptr_t next = reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
        if (next <= p)
            return false;
        p = next;
    }
    return true;
}

bool put(uintptr_t at, const void *src, size_t n)
{
    if (!writable(at, n))
        return false;
    std::memcpy(reinterpret_cast<void *>(at), src, n);
    return true;
}

bool write_fuel(uintptr_t obj, const pf::Fuel &f)
{
    return put(obj + pf::kBurn, &f.burn, sizeof f.burn) && put(obj + pf::kRows, f.rows, f.count * pf::kRowSize) &&
           put(obj + pf::kQualOneShot, &f.one_shot, sizeof f.one_shot) &&
           put(obj + pf::kQualShort, &f.short_quali, sizeof f.short_quali) &&
           put(obj + pf::kQualFull, &f.full_quali, sizeof f.full_quali);
}

std::mutex mu;
std::vector<FuelWrite> box_writes, session_writes;
std::string waiting_logged;

uintptr_t entity()
{
    const SwoftyCareer *c = runtime::career_table();
    return c && c->entity ? reinterpret_cast<uintptr_t>(c->entity()) : 0;
}

bool read_sets(uintptr_t e, std::vector<pf::Set> &sets, std::string &why)
{
    GameMem m;
    uintptr_t sys = 0;
    if (!e) {
        why = "no championship entity";
        return false;
    }
    if (!pf::read_u64(m, e + pf::kEntityTrackData, sys) || !swofty::mem::object_like(sys)) {
        why = "the championship entity's +0xf58 is not a game object";
        return false;
    }
    return pf::read_sets(m, sys, sets, why);
}

bool read_query(uintptr_t e, Query &q, std::string &why)
{
    uintptr_t mode = 0, list = 0;
    uint8_t stat = 0;
    if (!swofty::mem::read(e + ENTITY_MODE, mode) || !mode || !swofty::mem::read(mode + MODE_STAT, stat)) {
        why = "the session's setup mode is unreadable";
        return false;
    }
    q.mode = stat;
    uint16_t season = 0;
    if (swofty::mem::read(e + ENTITY_LIST, list) && list && swofty::mem::read(list + LIST_SEASON, season)) {
        q.season = season & 0xff;
        q.season_used = (season >> 8) != 0;
    }
    if (q.mode >= 32 || (q.season_used && q.season >= 32)) {
        why = "the session's query is out of range";
        return false;
    }
    return true;
}

int apply_writes(std::vector<FuelWrite> &plan, std::vector<FuelWrite> &kept)
{
    int n = 0;
    for (auto &w : plan) {
        bool held = false;
        for (auto &k : kept)
            held = held || k.object == w.object;
        if (held)
            continue;
        if (!write_fuel(w.object, w.after)) {
            write_fuel(w.object, w.before);
            continue;
        }
        kept.push_back(w);
        n++;
    }
    return n;
}

int put_back(std::vector<FuelWrite> &kept, int *changed)
{
    GameMem m;
    int n = 0;
    for (auto it = kept.rbegin(); it != kept.rend(); ++it) {
        pf::Fuel now;
        pf::Saved s{it->object, it->track, it->before, it->after};
        switch (pf::undo_action(s, pf::read_fuel(m, it->object, now), now)) {
        case pf::Undo::restore:
            n += write_fuel(it->object, it->before);
            break;
        default:
            if (changed)
                ++*changed;
            break;
        }
    }
    kept.clear();
    return n;
}

std::string code_of(const std::string &track)
{
    if (const BaseTrack *t = base().by_id(track))
        return t->code;
    auto slash = track.rfind('/');
    return slash == std::string::npos ? track : track.substr(slash + 1);
}

class Data : public Applier {
public:
    const char *registry() const override { return "track.data"; }
    Applies applies() const override { return Applies::session; }

    void session_load(Store &store, const Context &ctx, Report &report) override
    {
        if (!base().ready || ctx.track.empty())
            return;
        std::string code = code_of(ctx.track), why;
        std::vector<pf::Set> sets;
        uintptr_t e = entity();
        Active a = active();
        bool box_fuel = a.set && [&] {
            for (auto &[id, en] : a.entries.by_track)
                if (en.burn && base().by_id(id) && base().by_id(id)->code == code)
                    return true;
            return false;
        }();
        std::vector<std::pair<std::string, float>> data = data_fuel(store, ctx.track);
        if (!box_fuel && data.empty())
            return;
        if (!read_sets(e, sets, why)) {
            if (waiting_logged != why)
                swofty::log("[tracks] %s: track fuel not written: %s", ctx.track.c_str(), why.c_str());
            waiting_logged = why;
            return;
        }
        std::lock_guard g(mu);
        if (box_fuel) {
            Query q;
            if (!read_query(e, q, why)) {
                swofty::log("[tracks] box %s: fuel for %s not written: %s", a.package.c_str(), code.c_str(), why.c_str());
            } else {
                std::vector<FuelWrite> plan = plan_box_fuel(base(), a.entries, query_sets(sets, q), code);
                int n = apply_writes(plan, box_writes);
                report.applied += n;
                swofty::log("[tracks] box %s: %s fuel written in %d set(s) (query mode %u, season %u%s); put back when the "
                            "box is left",
                            a.package.c_str(), code.c_str(), n, q.mode, q.season, q.season_used ? "" : " unused");
            }
            return;
        }
        std::vector<FuelWrite> plan;
        for (auto &[set_name, burn] : data) {
            const pf::Table *t = pf::table_named(set_name);
            const pf::Set *s = t ? pf::pick(sets, *t) : nullptr;
            if (s && burn > 0.0f && burn < 20.0f)
                plan_burn(plan, *s, code, burn, ctx.track + "/" + set_name);
        }
        int n = apply_writes(plan, session_writes);
        report.applied += n;
        if (n)
            swofty::log("[tracks] %s: track.data fuel written in %d set(s) for this session", ctx.track.c_str(), n);
    }

    void session_end(Store &, Report &report) override
    {
        std::lock_guard g(mu);
        int changed = 0;
        report.put_back += put_back(session_writes, &changed);
        if (changed)
            report.lines.push_back(std::to_string(changed) + " track.data fuel write(s) changed since; left as they are");
    }

private:
    std::vector<std::pair<std::string, float>> data_fuel(Store &store, const std::string &track)
    {
        std::vector<std::pair<std::string, float>> out;
        for (auto &n : pf::kSets) {
            registry::Record r = store.get("track.data", track + "/" + n.name);
            if (r.status != registry::Status::ok)
                continue;
            const swofty::json::Value &f = r.value["fuel"];
            if (f.is_object() && f["burn_per_lap"].is_number() && f["burn_per_lap"].as_double() > 0)
                out.push_back({n.name, float(f["burn_per_lap"].as_double())});
        }
        return out;
    }
};

}

int put_back_box_fuel(const char *why)
{
    std::lock_guard g(mu);
    if (box_writes.empty())
        return 0;
    int changed = 0;
    size_t had = box_writes.size();
    int n = put_back(box_writes, &changed);
    swofty::log("[tracks] box fuel: %d of %zu write(s) put back (%s)%s", n, had, why ? why : "",
                changed ? (", " + std::to_string(changed) + " changed since and left").c_str() : "");
    return n;
}

void register_per_package();

void register_appliers()
{
    static std::once_flag once;
    std::call_once(once, [] {
        register_per_package();
        make<Data>();
    });
}

}
