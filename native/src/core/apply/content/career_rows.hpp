#pragma once

#include "lower.hpp"
#include "teams.hpp"
#include "wear.hpp"

#include <cstdio>
#include <map>
#include <set>
#include <string>
#include <vector>

// Lowers career.rnd.tree (nodes of one team's R&D tree: upgrades.<group>.trees.<token>.<department>.upgrade_N) and
// career.market (one driver market group: driver_market.driver_market.DataGroup[<group>]) to row operations. The game
// reads both when a career is created and loaded, so they apply at boot with the other row registries.

namespace swofty_core::apply::content {

inline constexpr const char *RND_TOKENS[] = {"mercedes", "ferrari",  "redbull",     "williams",    "tororosso",
                                             "renault",  "haas",     "mclaren",     "sauber",      "force_india",
                                             "gm_cadillac", "user_defined"};
inline constexpr const char *RND_DEPARTMENTS[] = {"aerodynamics", "chassis", "durability", "engine", "fake"};
inline constexpr const char *RND_NODE_FIELDS[] = {
    "_metadata_", "upgrade_name", "upgrade_type", "upgrade_size", "upgrade_department", "cost", "cost_to_develop",
    "weighting", "unlock_at_facility_level", "supplier_delivery_date_index", "links",
    "fitting_times.num_days_to_research", "fitting_times.num_days_to_develop", "grid_coordinates.x",
    "grid_coordinates.y"};
inline constexpr const char *MARKET = "driver_market.driver_market";
inline constexpr const char *MONTHS[] = {"January", "February", "March",     "April",   "May",      "June",
                                         "July",    "August",   "September", "October", "November", "December"};

inline std::string number_text(const Value &v)
{
    if (v.is_int())
        return std::to_string(v.as_int());
    char buf[32];
    std::snprintf(buf, sizeof buf, "%g", v.as_double());
    return buf;
}

inline std::string capitalised(std::string_view s)
{
    std::string out(s);
    if (!out.empty() && out[0] >= 'a' && out[0] <= 'z')
        out[0] = char(out[0] - 'a' + 'A');
    return out;
}

inline std::optional<int64_t> count_of(const Read &read, const std::string &table, const std::string &row)
{
    auto t = read(table + "._" + row + "_instancecount_");
    if (!t)
        return std::nullopt;
    char *end = nullptr;
    long long n = std::strtoll(t->c_str(), &end, 10);
    return end && *end == '\0' && n >= 0 ? std::optional<int64_t>(n) : std::nullopt;
}

// The row of a table whose field holds want, by index: the table's real path.
inline std::string row_where(const Read &read, const std::string &table, const std::string &row, const char *field,
                             const std::string &want)
{
    auto n = count_of(read, table, row);
    for (int64_t i = 0; n && i < *n; i++) {
        std::string at = table + "." + row + "_" + std::to_string(i);
        if (read(at + "." + field) == want)
            return at;
    }
    return {};
}

inline std::string join(const Value &list)
{
    std::string out;
    if (auto a = list.if_array())
        for (auto &v : *a)
            if (v.is_string() && !v.as_string().empty())
                out += (out.empty() ? "" : ",") + std::string(v.as_string());
    return out;
}

inline std::string rnd_container(const std::string &group, const std::string &token, const std::string &department)
{
    return "upgrades." + group + ".trees." + token + "." + department;
}

// The upgrade types a group's stock trees use (the game reads any other name as type 0).
inline std::set<std::string> rnd_types(const Read &read, const std::string &group)
{
    std::set<std::string> out;
    for (const char *token : RND_TOKENS)
        for (std::size_t d = 0; d < 4; d++) {
            std::string c = rnd_container(group, token, RND_DEPARTMENTS[d]);
            auto n = count_of(read, c, "upgrade");
            for (int64_t i = 0; n && i < *n; i++)
                if (auto t = read(c + ".upgrade_" + std::to_string(i) + ".upgrade_type"))
                    out.insert(*t);
        }
    return out;
}

inline void lower_rnd_tree(const std::string &id, const Value &rec, const Read &read, Plan &plan)
{
    plan.owner = addon_of(id);
    std::string group = rec["data_group"].is_string() ? std::string(rec["data_group"].as_string()) : "2026";
    std::string token(rec["token"].as_string());
    std::string head = "career.rnd.tree " + id + ": ";
    if (token.empty() || !count_of(read, rnd_container(group, token, "fake"), "upgrade")) {
        plan.lines.push_back(head + "no tree " + token + " in upgrades group " + group + "; nothing applied");
        plan.refused++;
        return;
    }
    std::map<std::string, std::string> where;
    for (const char *d : RND_DEPARTMENTS) {
        std::string c = rnd_container(group, token, d);
        auto n = count_of(read, c, "upgrade");
        for (int64_t i = 0; n && i < *n; i++)
            if (auto l = read(c + ".upgrade_" + std::to_string(i) + "._metadata_"))
                where.emplace(*l, c);
    }
    std::set<std::string> types = rnd_types(read, group);
    int added = 0, edited = 0, refused = 0;
    const auto *nodes = rec["nodes"].if_array();
    for (std::size_t k = 0; nodes && k < nodes->size(); k++) {
        const Value &n = (*nodes)[k];
        std::string label(n["label"].as_string());
        std::string dept = n["department"].is_string() ? std::string(n["department"].as_string()) : "aerodynamics";
        std::string c = rnd_container(group, token, dept);
        auto refuse = [&](const std::string &why) {
            plan.lines.push_back(head + "node " + label + " refused: " + why);
            refused++;
        };
        if (std::string t = n["type"].is_string() ? std::string(n["type"].as_string()) : ""; !t.empty() && !types.count(t)) {
            refuse("type \"" + t + "\" is no type the " + group + " trees use");
            continue;
        }
        auto at = where.find(label);
        if (at != where.end() && at->second != c) {
            refuse("it is a node of " + at->second.substr(at->second.rfind('.') + 1) + ", not " + dept);
            continue;
        }
        std::string from = n["from"].is_string() ? std::string(n["from"].as_string()) : "";
        bool fresh = at == where.end();
        if (fresh) {
            if (from.empty())
                from = read(c + ".upgrade_0._metadata_").value_or("");
            auto src = where.find(from);
            if (from.empty() || src == where.end() || src->second != c) {
                refuse("no node " + (from.empty() ? std::string("to copy") : from) + " in " + dept + " to copy");
                continue;
            }
        }
        bool after_ok = true;
        if (auto a = n["after"].if_array())
            for (auto &v : *a)
                if (!where.count(std::string(v.as_string())) && v.as_string() != label)
                    after_ok = false;
        if (!after_ok) {
            refuse("an \"after\" node is not in the " + token + " tree");
            continue;
        }
        Value set = Value::object();
        if (n["name"].is_string() && !n["name"].as_string().empty())
            set["upgrade_name"] = Value(std::string(n["name"].as_string()));
        if (n["type"].is_string() && !n["type"].as_string().empty())
            set["upgrade_type"] = Value(std::string(n["type"].as_string()));
        if (n["size"].is_string())
            set["upgrade_size"] = Value(capitalised(n["size"].as_string()));
        static constexpr std::pair<const char *, const char *> NUMBERS[] = {
            {"cost", "cost"},
            {"cost_to_develop", "cost_to_develop"},
            {"weighting", "weighting"},
            {"unlock_at_facility_level", "unlock_at_facility_level"},
            {"research_days", "fitting_times.num_days_to_research"},
            {"develop_days", "fitting_times.num_days_to_develop"},
            {"x", "grid_coordinates.x"},
            {"y", "grid_coordinates.y"}};
        for (auto &[field, key] : NUMBERS)
            if (n[field].is_number())
                set[key] = Value(number_text(n[field]));
        std::string links = join(n["links"]);
        std::string row = c + ".upgrade[_metadata_=" + label + "]";
        if (fresh) {
            add_fields(plan, (c + ".upgrade").c_str(), RND_NODE_FIELDS, std::size(RND_NODE_FIELDS));
            set["_metadata_"] = Value(label);
            set["links"] = Value(links);
            set["upgrade_department"] = Value(capitalised(dept));
            push_row(plan, c, c + ".upgrade[_metadata_=" + from + "]", std::move(set));
            where.emplace(label, c);
            added++;
        } else {
            if (!links.empty())
                set["links"] = Value(links);
            for (auto &[f, v] : *set.if_object())
                push_op(plan, set_op(row, std::string(f), std::string(v.as_string())));
            edited++;
        }
        if (auto a = n["after"].if_array())
            for (auto &v : *a) {
                std::string parent(v.as_string());
                push_op(plan, append_op(where[parent] + ".upgrade[_metadata_=" + parent + "]", "links", label));
            }
    }
    plan.lines.push_back(head + std::to_string(added) + " node(s) added, " + std::to_string(edited) + " edited, " +
                         std::to_string(refused) + " refused in the " + token + " tree of upgrades group " + group);
    plan.refused += refused;
    if (!added && !edited)
        plan.refused++;
}

struct Date {
    std::string year, month, day;
};

inline std::optional<Date> date_of(const Value &v)
{
    if (!v.is_string() || v.as_string().size() != 10)
        return std::nullopt;
    std::string s(v.as_string());
    int m = std::atoi(s.substr(5, 2).c_str()), d = std::atoi(s.substr(8, 2).c_str());
    if (m < 1 || m > 12 || d < 1 || d > 31)
        return std::nullopt;
    return Date{s.substr(0, 4), MONTHS[m - 1], std::to_string(d)};
}

inline void set_date(Plan &plan, const std::string &row, const char *which, const Date &d)
{
    std::string sel = row + ".date_time[_metadata_=" + which + "]";
    push_op(plan, set_op(sel, "year", d.year));
    push_op(plan, set_op(sel, "month", d.month));
    push_op(plan, set_op(sel, "day", d.day));
}

struct MarketEntry {
    std::string driver;
    Value rec;
};

// Rewrites every schedule of a market table with the entries: rows in place, more rows copied from the schedule's
// first, extra stock rows cut by the instance count. Returns the schedules rewritten.
inline int rewrite_schedules(const Read &read, Plan &plan, const std::string &table, const char *schedule_row,
                             const char *entry_row, const std::vector<MarketEntry> &entries,
                             const std::function<void(const std::string &row, const MarketEntry &e)> &fill)
{
    auto schedules = count_of(read, table, schedule_row);
    int done = 0;
    for (int64_t s = 0; schedules && s < *schedules; s++) {
        std::string sched = table + "." + schedule_row + "_" + std::to_string(s);
        auto have = count_of(read, sched, entry_row);
        if (!have || !*have)
            continue;
        int64_t n = int64_t(entries.size());
        for (int64_t i = 0; i < n; i++) {
            std::string row = sched + "." + entry_row + "_" + std::to_string(i);
            if (i >= *have)
                push_row(plan, sched, sched + "." + entry_row + "_0",
                         Value::object({{"_metadata_", "sw_" + entries[std::size_t(i)].driver}}));
            push_op(plan, set_op(row, "driver", entries[std::size_t(i)].driver));
            fill(row, entries[std::size_t(i)]);
        }
        if (n < *have)
            push_op(plan, set_op(sched, std::string("_") + entry_row + "_instancecount_", std::to_string(n)));
        done++;
    }
    return done;
}

inline void lower_market(const std::string &id, const Value &rec, const Lookup &lookup, const Read &read,
                         const TeamRefs &refs, Plan &plan)
{
    plan.owner = addon_of(id);
    std::string group = rec["data_group"].is_string() ? std::string(rec["data_group"].as_string()) : "2026";
    std::string head = "career.market " + id + ": ";
    std::string base = row_where(read, MARKET, "DataGroup", "_metadata_", group);
    if (base.empty()) {
        plan.lines.push_back(head + "no driver market group " + group + "; nothing applied");
        plan.refused++;
        return;
    }
    std::vector<std::string> parts;
    int refused = 0;
    auto driver_of = [&](const Value &ref, std::string &label) {
        if (!ref.is_string())
            return false;
        label = label_of(lookup, "driver.identity", ref.as_string());
        if (read(row_at(DRIVERS, "driver", label) + "._metadata_"))
            return true;
        plan.lines.push_back(head + "driver " + std::string(ref.as_string()) + " has no drivers row; left out");
        refused++;
        return false;
    };
    if (auto list = rec["retirements"].if_array()) {
        std::vector<MarketEntry> entries;
        for (auto &r : *list) {
            std::string label;
            if (!driver_of(r["driver"], label))
                continue;
            if (!date_of(r["retire"]) || !date_of(r["announce"])) {
                plan.lines.push_back(head + "retirement of " + label + " has a bad date; left out");
                refused++;
                continue;
            }
            entries.push_back({label, r});
        }
        int n = rewrite_schedules(read, plan, base + ".driver_retirements", "DriverRetirementSchedule",
                                  "DriverRetirement", entries, [&plan](const std::string &row, const MarketEntry &e) {
                                      set_date(plan, row, "Retirement", *date_of(e.rec["retire"]));
                                      set_date(plan, row, "Announcement", *date_of(e.rec["announce"]));
                                  });
        parts.push_back(std::to_string(entries.size()) + " retirement(s) in " + std::to_string(n) + " schedule(s)");
    }
    if (auto list = rec["firings"].if_array()) {
        std::vector<MarketEntry> entries;
        for (auto &r : *list) {
            std::string label;
            if (driver_of(r["driver"], label))
                entries.push_back({label, r});
        }
        int n = rewrite_schedules(read, plan, base + ".driver_firings", "DriverFiredSchedule", "DriverFired", entries,
                                  [&plan](const std::string &row, const MarketEntry &e) {
                                      push_op(plan, set_op(row, "season_index", number_text(e.rec["season_index"])));
                                  });
        parts.push_back(std::to_string(entries.size()) + " firing(s) in " + std::to_string(n) + " schedule(s)");
    }
    if (auto list = rec["team_ratings"].if_array()) {
        std::string table = base + ".team_driver_rating_range_requirements";
        int set = 0, added = 0;
        for (auto &r : *list) {
            std::string team = team_label(r["team"], refs);
            if (team.empty() || !read("entrants.teams.team[_metadata_=" + team + "]._metadata_")) {
                plan.lines.push_back(head + "team " + std::string(r["team"].as_string()) + " has no entrants.teams row; left out");
                refused++;
                continue;
            }
            std::string rating = number_text(r["rating"]);
            std::string row = row_where(read, table, "TeamDriverRatingRangeRequirement", "team", team);
            if (!row.empty()) {
                push_op(plan, set_op(row, "rating_range", rating));
                set++;
                continue;
            }
            push_row(plan, table, table + ".TeamDriverRatingRangeRequirement_0",
                     Value::object({{"_metadata_", team}, {"team", team}, {"rating_range", rating}}));
            added++;
        }
        parts.push_back(std::to_string(set) + " team rating(s) set, " + std::to_string(added) + " added");
    }
    std::string line;
    for (auto &p : parts)
        line += (line.empty() ? "" : ", ") + p;
    plan.lines.push_back(head + (line.empty() ? std::string("nothing to change") : line) + " in market group " + group +
                         (refused ? ", " + std::to_string(refused) + " left out" : std::string()));
    plan.refused += std::size_t(refused);
    if (parts.empty())
        plan.refused++;
}

}
