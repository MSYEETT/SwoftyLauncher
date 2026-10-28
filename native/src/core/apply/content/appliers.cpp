#include "../applier.hpp"
#include "gvdb.hpp"
#include "lower.hpp"

#include "../../runtime.hpp"

#include <swofty/mixin.hpp>

#include <cstdio>

#include <map>
#include <memory>

// The core's appliers for the content registries. In 1.6.0 the rows of a package, its vehicles and their
// packages come from the addon's build operations (db/records.json, which every launcher builds), so at
// boot these appliers put in the records the game database edit path can take as they are: each car's
// audio.audio_car_data records, with the bundle of the engine sound its effective record names. Era boxes
// go into the core's era picker. Row copies (new vehicles, packages and entrant lists from layers and
// variants) wait for the row engine of the live apply paths; the plan logs each one it leaves.

namespace swofty_core::apply::content {

namespace {

std::unordered_map<uint64_t, gvdb::Record> &stock()
{
    static std::unordered_map<uint64_t, gvdb::Record> db;
    static bool loaded = false;
    if (!loaded) {
        loaded = true;
        std::wstring path = runtime::game_root() + L"\\game_package\\gamedatabases\\gamedatabases.2025.bin";
        std::string bin;
        if (FILE *f = _wfopen(path.c_str(), L"rb")) {
            char buf[1 << 16];
            for (std::size_t n; (n = fread(buf, 1, sizeof buf, f)) > 0;)
                bin.append(buf, n);
            fclose(f);
        }
        if (!gvdb::read(reinterpret_cast<const unsigned char *>(bin.data()), bin.size(), db)) {
            swofty::log("[apply] content: the game database file is unreadable; audio copies skipped");
            db.clear();
        }
    }
    return db;
}

Source game_source()
{
    return [](const std::string &key) -> std::optional<Stock> {
        auto &db = stock();
        auto it = db.find(gvdb::fnv64(key));
        if (it == db.end())
            return std::nullopt;
        return Stock{it->second.tag, it->second.text};
    };
}

Lookup store_lookup(Store &st, std::map<std::string, Value> &keep)
{
    return [&st, &keep](const std::string &reg, const std::string &id) -> const Value * {
        std::string k = reg + "|" + id;
        auto it = keep.find(k);
        if (it != keep.end())
            return &it->second;
        auto rec = st.get(reg, id);
        if (rec.status != registry::Status::ok)
            return nullptr;
        return &(keep[k] = rec.value);
    };
}

Plan lower(Store &st, const char *registry)
{
    std::map<std::string, Value> keep;
    Lookup lookup = store_lookup(st, keep);
    Source source = game_source();
    Plan plan;
    std::string reg = registry;
    for (auto &id : st.list(reg, "")) {
        const Value *rec = lookup(reg, id);
        if (!rec)
            continue;
        if (reg == "engine_sound")
            lower_engine_sound(id, *rec, plan);
        else if (reg == "vehicle.package")
            lower_vehicle_package(id, *rec, plan);
        else if (reg == "vehicle")
            lower_vehicle(id, *rec, lookup, source, plan);
        else if (reg == "content.package")
            lower_package(id, *rec, lookup, plan);
        else if (reg == "era.box")
            lower_box(id, *rec, lookup, plan);
    }
    return plan;
}

std::string unhex(std::string_view h)
{
    std::string out;
    for (std::size_t i = 0; i + 1 < h.size(); i += 2)
        out += char(std::stoi(std::string(h.substr(i, 2)), nullptr, 16));
    return out;
}

void submit(Plan &plan, Report &report)
{
    std::size_t rows = 0;
    for (std::size_t i = 0; i < plan.ops.size(); i++) {
        const Value &op = plan.ops[i];
        if (op["op"].as_string() != "put-record") {
            rows++;
            continue;
        }
        json::Value e;
        std::string text = "{\"key\":" + swofty::json::dump(op["key"]) + ",\"type\":" + std::to_string(op["type"].as_int()) +
                           ",\"value\":" + swofty::json::dump(Value(unhex(op["value"].as_string()))) + "}";
        if (!json::parse(text, e) || runtime::db_add(e) != SWOFTY_ASSET_OK) {
            report.refused++;
            report.lines.push_back("db " + std::string(op["key"].as_string()) + " refused (late or invalid)");
            continue;
        }
        report.applied++;
    }
    if (rows)
        report.lines.push_back(std::to_string(rows) +
                               " row operation(s) left to the addon's build operations (the core's row engine is not in "
                               "this build)");
    for (auto &l : plan.lines)
        report.lines.push_back(l);
}

class Rows : public Applier {
public:
    explicit Rows(const char *registry, Applies applies) : registry_(registry), applies_(applies) {}
    const char *registry() const override { return registry_; }
    Applies applies() const override { return applies_; }
    std::size_t database_keys(Store &st) override { return lower(st, registry_).keys; }
    void boot(Store &st, Report &report) override
    {
        Plan plan = lower(st, registry_);
        submit(plan, report);
    }

private:
    const char *registry_;
    Applies applies_;
};

class Packages : public Rows {
public:
    Packages() : Rows("content.package", Applies::content) {}
    void activate(Store &, const Context &, Report &report) override
    {
        report.lines.push_back("content.package: no box rules to apply in this build");
    }
    void deactivate(Store &, Report &) override {}
};

class Boxes : public Applier {
public:
    const char *registry() const override { return "era.box"; }
    Applies applies() const override { return Applies::read; }
    void install_read(Store &st, Report &report) override
    {
        Plan plan = lower(st, "era.box");
        const SwoftyEra *era = runtime::era_table();
        for (auto &b : plan.boxes) {
            SwoftyEraBox d{sizeof d, b.entrant_list.c_str(), b.title.c_str(), b.image.c_str(), b.order};
            int rc = era ? era->add_box(&d) : SWOFTY_ASSET_UNAVAILABLE;
            if (rc == SWOFTY_ASSET_OK)
                report.applied++;
            else
                report.refused++;
            report.lines.push_back("era.box " + b.id + ": " + b.entrant_list + (rc == SWOFTY_ASSET_OK ? " registered" : " not registered (a box for it exists or the picker is off)"));
        }
        for (auto &l : plan.lines)
            report.lines.push_back(l);
    }
};

}

void register_appliers()
{
    make<Rows>("engine_sound", Applies::boot);
    make<Rows>("vehicle.package", Applies::boot);
    make<Rows>("vehicle", Applies::boot);
    make<Packages>();
    make<Boxes>();
}

}
