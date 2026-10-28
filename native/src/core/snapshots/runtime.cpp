#include "../runtime.hpp"

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>
#include <swofty/systems/career.hpp>

#include <cstddef>
#include <cstdio>
#include <cstring>
#include <memory>
#include <mutex>

#include "../apply/moments.hpp"
#include "../hooks/hooks.hpp"
#include "../registry/table.hpp"
#include "../save_splice_logic.hpp"
#include "snapshots.hpp"

// Career snapshots in game: the book (snapshots.cpp) on the core's registry store, fed by the career
// moments on the hooks table and by the save splice's root loads and profile saves.
//
// Moments: career.loaded and career.unloaded (fired by the core's career tracker), career.created
// and career.season.start (declared by the core with firing helpers in hooks/core_events.cpp; the
// core does not yet see those moments, so nothing fires them until an apply path or the career mode's
// owner does: hooks::events::career_season_start(year, season_index) at the season's start, and
// hooks::events::career_created() once the new career's id is in the facts). At a season start the
// apply scheduler activates the season (apply::moments::career_season) and then the freeze runs, so
// both see the same season; this runs first among each moment's after handlers, so addons that read
// the season there read the snapshot. The career's stat-id map comes from apply::moments.
//
// Writing: only through the save splice, and only when it is armed and the game's bag type id is
// the one the entry's layout assumes (save_splice::entry_blocked). SWOFTY_REGISTRY_SAVE=0 (or off,
// dry) keeps the snapshots in memory for the session and logs what each save would have written.

namespace runtime {
namespace {

namespace reg = swofty_core::registry;
namespace snap = swofty_core::snapshots;
namespace sp = runtime::save_splice;
namespace hk = swofty_core::hooks;

std::once_flag started;
std::unique_ptr<snap::Book> the_book;
bool writes = false;
std::string blocked;
size_t dry_logged = 0;

snap::Book &book() { return *the_book; }

reg::Context facts_context()
{
    reg::Context ctx;
    if (auto f = hk::core().current(); f && f->career)
        ctx.data_group = f->career->data_group;
    char group[128] = {};
    if (auto t = runtime::career_content_table(); ctx.data_group.empty() && t && t->data_group &&
                                                  t->data_group(nullptr, group, sizeof group) == SWOFTY_ASSET_OK)
        ctx.data_group = group;
    return ctx;
}

std::string map_json(const std::string &career)
{
    swofty::json::Value out = swofty::json::Value::object();
    for (auto &[registry, records] : swofty_core::apply::moments::map_for_career(career))
        for (auto &[record, id] : records)
            out[registry][record] = swofty::json::Value(int64_t(id));
    return swofty::json::dump(out);
}

std::vector<std::string> adopt_json(const std::string &career, const std::string &text)
{
    swofty::json::Error e;
    swofty::json::Value v = swofty::json::parse(text, &e);
    swofty_core::apply::ids::CareerMap map;
    if (const auto *regs = v.if_object())
        for (auto &[registry, records] : *regs)
            if (const auto *o = records.if_object())
                for (auto &[record, id] : *o)
                    if (id.is_int())
                        map[std::string(registry)][std::string(record)] = id.as_int();
    std::vector<std::string> lines;
    for (auto &c : swofty_core::apply::moments::adopt(career, map))
        lines.push_back(c.line);
    return lines;
}

std::string facts_career()
{
    auto f = hk::core().current();
    return f && f->career ? f->career->id : std::string();
}

snap::format::Freeze freeze_of(const std::string &data_group)
{
    reg::Store &st = reg::store();
    for (auto &id : st.list("career.mode", "")) {
        reg::Record r = st.get("career.mode", id);
        if (r.status == reg::Status::ok && !data_group.empty() && r.value["data_group"].as_string() == data_group)
            return r.value["freeze"].as_string() == "at_creation" ? snap::format::Freeze::at_creation
                                                                   : snap::format::Freeze::per_season;
    }
    return snap::format::Freeze::per_season;
}

int on_loaded(void *, SwoftyHookCall *)
{
    book().career_loaded(facts_career(), facts_context());
    return 0;
}

int on_unloaded(void *, SwoftyHookCall *)
{
    book().career_unloaded();
    return 0;
}

int on_created(void *, SwoftyHookCall *)
{
    reg::Context ctx = facts_context();
    book().career_created(facts_career(), freeze_of(ctx.data_group), ctx);
    return 0;
}

int on_season_start(void *, SwoftyHookCall *call)
{
    int year = 0;
    if (call && call->payload) {
        auto *p = static_cast<const swofty::career::events::SeasonStart *>(call->payload);
        if (p->size >= offsetof(swofty::career::events::SeasonStart, season_index) + sizeof(int))
            year = p->year;
    }
    if (!year)
        if (auto f = hk::core().current(); f && f->career)
            year = f->career->year;
    if (!year) {
        swofty::log("[registry] career.season.start without a year; nothing frozen");
        return 0;
    }
    swofty_core::apply::moments::career_season(year);
    book().season_start(year, facts_context());
    return 0;
}

void entry_loaded(const std::vector<uint8_t> *body) { book().loaded(body); }

bool entry_saving(std::vector<uint8_t> &body)
{
    if (!book().saving(body))
        return false;
    if (writes)
        return true;
    book().written(false);
    if (body.size() != dry_logged) {
        dry_logged = body.size();
        swofty::log("[registry] swofty_registry: would write %zu bytes with this profile save, but %s; the career "
                    "snapshots stay in memory for this session",
                    body.size(), blocked.c_str());
    }
    return false;
}

void entry_written(bool ok)
{
    book().written(ok);
    if (ok)
        swofty::log("[registry] swofty_registry: written with this profile save");
}

void attach(const char *point, SwoftyHookFn fn)
{
    uint32_t token = 0;
    hk::core().attach("swofty.core", point, SWOFTY_PHASE_AFTER, 1000000, "", fn, nullptr, &token);
}

}

void snapshots_start(bool splice_live)
{
    std::call_once(started, [&] {
        the_book = std::make_unique<snap::Book>(reg::store(), [](const std::string &s) { swofty::log("%s", s.c_str()); });
        the_book->set_ids({map_json, adopt_json});
        reg::store().set_snapshot_source(the_book->source());
        attach("career.loaded", on_loaded);
        attach("career.unloaded", on_unloaded);
        attach("career.created", on_created);
        attach("career.season.start", on_season_start);
        char v[16] = {};
        DWORD n = GetEnvironmentVariableA("SWOFTY_REGISTRY_SAVE", v, sizeof v);
        bool off = n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off") || !strcmp(v, "dry"));
        blocked = off ? std::string("SWOFTY_REGISTRY_SAVE=") + v : splice_live ? sp::entry_blocked() : "the save splice is off";
        writes = blocked.empty();
        if (splice_live)
            sp::set_entry_hooks({entry_loaded, entry_saving, entry_written});
        swofty::log("[registry] career snapshots on: %s", writes ? "saved in swofty_registry through the save splice"
                                                                 : ("not saved (" + blocked + ")").c_str());
    });
}

}
