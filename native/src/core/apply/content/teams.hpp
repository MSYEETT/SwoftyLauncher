#pragma once

#include "liveries.hpp"
#include "lower.hpp"

#include <cstring>
#include <functional>
#include <optional>
#include <set>

// Lowers team.identity (a new entrants.teams row and the side rows found by its label), team.pit_times and
// team.logo to the operations of the row engine, and dresses a loaded team record as another identity. A new team
// copies its donor's row (every key, story_info included) and takes its names, colours (signed ARGB), livery
// name, prestige, nationality, audio id and stat id from the record. The side rows (pit times, initial cash,
// facilities, driver market, recognition, renown and form, primary sponsors, showroom history) are copies of the
// donor's rows relabelled to the new team, in every data group that holds one. The wearables stream writes the
// team customisation rows (suit, gloves), the vehicle's animation team data follows the car.

namespace swofty_core::apply::content {

inline constexpr const char *TEAMS = "entrants.teams";
inline constexpr const char *TEAM_ROW = "entrants.teams.team";
inline constexpr const char *PIT_TIMES = "pit_lane.pit_times";
inline constexpr const char *PIT_ROW = "team_pit_times";
inline constexpr const char *TEAM_IDS = "team.identity";

struct SideTable {
    const char *group, *group_row, *sub, *row;
    bool pit;
};

inline constexpr SideTable SIDE_TABLES[] = {
    {PIT_TIMES, nullptr, nullptr, PIT_ROW, true},
    {"bank.initial_team_cash", nullptr, nullptr, "InitialTeamCash", false},
    {"showroom.teams_history", nullptr, nullptr, "team_history", false},
    {"facilities.facilities", "Facilities", "team_developments", "TeamDevelopment", false},
    {"driver_market.driver_market", "DataGroup", "team_driver_rating_range_requirements",
     "TeamDriverRatingRangeRequirement", false},
    {"recognition", "RecognitionDataGroup", "team_modifiers", "TeamModifier", false},
    {"career.my_team_renown_and_form", "MyTeamRenownAndFormDataGroup", "initial_renown_and_form",
     "InitialTeamRenownAndForm", false},
    {"career.my_team_sponsors", "MyTeamSponsorsDataGroup", "team_primary_sponsors", "team_primary_sponsor", false},
};

struct TeamRefs {
    std::map<std::string, std::string> constructor_team;
    std::map<std::string, std::string> pit_record;
    std::vector<std::string> lines;
    int constructors = 0, constructors_refused = 0;
};

inline std::string constructor_token(const Value &ref)
{
    if (!ref.is_string())
        return {};
    std::string s(ref.as_string());
    if (s.rfind("constructor/", 0) == 0)
        return s.substr(12);
    return key_of(s);
}

inline std::string team_label(const Value &ref, const TeamRefs &refs)
{
    if (!ref.is_string())
        return {};
    std::string s(ref.as_string());
    if (s.rfind("constructor/", 0) == 0) {
        auto it = refs.constructor_team.find(s.substr(12));
        return it == refs.constructor_team.end() ? std::string() : key_of(it->second);
    }
    return key_of(s);
}

inline std::optional<int32_t> argb(const Value &v)
{
    if (!v.is_string())
        return std::nullopt;
    std::string s(v.as_string());
    if (s.size() != 7 && s.size() != 9)
        return std::nullopt;
    if (s[0] != '#')
        return std::nullopt;
    uint32_t n = 0;
    for (std::size_t i = 1; i < s.size(); i++) {
        char c = s[i];
        int d = c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : c >= 'A' && c <= 'F' ? c - 'A' + 10 : -1;
        if (d < 0)
            return std::nullopt;
        n = n * 16 + uint32_t(d);
    }
    if (s.size() == 7)
        n |= 0xff000000u;
    return int32_t(n);
}

inline TeamRefs team_refs(const std::vector<std::pair<std::string, const Value *>> &identities,
                          const std::vector<std::pair<std::string, const Value *>> &constructors,
                          const std::vector<std::pair<std::string, const Value *>> &pit_times)
{
    TeamRefs r;
    for (auto &[id, rec] : identities)
        if (std::string t = constructor_token((*rec)["constructor"]); !t.empty())
            r.constructor_team[t] = id;
    for (auto &[id, rec] : constructors) {
        std::string token = key_of(id);
        auto it = r.constructor_team.find(token);
        if (it != r.constructor_team.end()) {
            r.lines.push_back("team.constructor constructor/" + token + ": provided by " + it->second);
            r.constructors++;
        } else if ((*rec)["team"].is_string()) {
            std::string team(static_cast<const Value &>(*rec)["team"].as_string());
            r.constructor_team[token] = team;
            r.lines.push_back("team.constructor constructor/" + token + ": provided by " + team);
            r.constructors++;
        } else {
            r.lines.push_back("team.constructor " + id + ": no enabled team is this constructor");
            r.constructors_refused++;
        }
    }
    for (auto &[id, rec] : pit_times) {
        std::string label = team_label((*rec)["team"], r);
        if (!label.empty())
            r.pit_record[label] = id;
    }
    return r;
}

inline std::string team_at(const std::string &label)
{
    return std::string(TEAM_ROW) + "[_metadata_=" + label + "]";
}

inline int64_t rows_of(const Read &read, const std::string &path)
{
    auto n = read(path);
    return n ? std::strtoll(n->c_str(), nullptr, 10) : 0;
}

struct SideRow {
    std::string table, path;
    bool found = false;
};

inline std::vector<SideRow> side_rows(const SideTable &s, const Read &read, const std::string &team)
{
    std::vector<SideRow> out;
    int64_t groups = s.group_row ? rows_of(read, std::string(s.group) + "._" + s.group_row + "_instancecount_") : 1;
    for (int64_t g = 0; g < groups; g++) {
        std::string table = s.group_row ? std::string(s.group) + "." + s.group_row + "_" + std::to_string(g) + "." + s.sub
                                        : std::string(s.group);
        int64_t rows = rows_of(read, table + "._" + s.row + "_instancecount_");
        SideRow hit{table, {}, false};
        for (int64_t i = 0; i < rows; i++) {
            std::string path = table + "." + s.row + "_" + std::to_string(i);
            if (read(path + ".team") == team) {
                hit.path = path;
                hit.found = true;
                break;
            }
        }
        out.push_back(std::move(hit));
    }
    return out;
}

inline std::size_t copy_side(Plan &plan, const SideTable &s, const Read &read, const std::string &from,
                             const std::string &to)
{
    std::size_t copied = 0;
    auto exists = side_rows(s, read, to);
    std::vector<SideRow> sources = side_rows(s, read, from);
    for (std::size_t g = 0; g < sources.size(); g++) {
        if (!sources[g].found || (g < exists.size() && exists[g].found))
            continue;
        push_row(plan, sources[g].table, sources[g].path,
                 Value::object({{"_metadata_", to}, {"team", to}}));
        copied++;
    }
    return copied;
}

inline constexpr const char *LOGO_CONTEXTS[] = {"driver_select", "team_select", "hud", "broadcast", "hub"};

struct LogoArray {
    const char *array;
    bool tower;
};

inline std::vector<LogoArray> logo_arrays(const std::string &context)
{
    if (context == "driver_select" || context == "hub")
        return {{"teams.driver_select_team_logos", false}};
    if (context == "team_select")
        return {{"team_logos.driver_career_2024_team_select", false}, {"teams.mp_team_select_logos", false}};
    if (context == "hud")
        return {{"osd.timing_tower.team_badges_hud", true}};
    return {{"osd.timing_tower.team_badges_broadcast", true},
            {"teams.badges_broadcast", false},
            {"broadcast_ui_generic.grid_team_logos", false}};
}

inline bool has_file(const Value &logo, const char *context)
{
    return logo["images"][context].is_string();
}

inline std::string stock_key(const Value &logo, const char *context)
{
    const Value &v = logo["images"][context];
    return v.is_object() && v["stock"].is_string() ? std::string(v["stock"].as_string()) : std::string();
}

inline void lower_logo_images(const std::string &team, const std::string &logo_id, const Value &logo,
                              const std::string &tower_key, Plan &plan)
{
    std::string addon = addon_of(logo_id);
    std::set<std::string> taken;
    for (const char *context : LOGO_CONTEXTS) {
        const Value &v = logo["images"][context];
        if (!v.is_string() && !(v.is_object() && v["stock"].is_string()))
            continue;
        for (auto &a : logo_arrays(context)) {
            if (a.tower && !v.is_string())
                continue;
            std::string key = a.tower ? tower_key : team;
            if (!taken.insert(std::string(a.array) + "|" + key).second)
                continue;
            LogoImage img;
            img.array = a.array;
            img.key = key;
            img.context = context;
            img.addon = addon;
            img.id = logo_id;
            img.team = team;
            if (v.is_string())
                img.file = std::string(v.as_string());
            else
                img.stock = std::string(v["stock"].as_string());
            plan.logos.push_back(std::move(img));
        }
    }
}

inline void lower_team(const std::string &id, const Value &rec, const Lookup &lookup, const Read &read,
                       const TeamRefs &refs, Plan &plan, const StatIds &stat = {})
{
    plan.owner = addon_of(id);
    std::string label = key_of(id);
    if (!rec["donor"].is_string()) {
        plan.lines.push_back("team.identity " + id + ": no donor; nothing applied");
        plan.refused++;
        return;
    }
    std::string donor = team_label(rec["donor"], refs);
    if (donor.empty() || !read(team_at(donor) + "._metadata_")) {
        plan.lines.push_back("team.identity " + id + ": the game database has no team row " +
                             (donor.empty() ? std::string(rec["donor"].as_string()) : donor) + " to copy; nothing applied");
        plan.refused++;
        return;
    }
    if (read(team_at(label) + "._metadata_")) {
        plan.lines.push_back("team.identity " + id + ": a team row named " + label + " exists; nothing applied");
        plan.refused++;
        return;
    }
    Value set = Value::object({{"_metadata_", label}});
    std::vector<std::string> skipped;
    auto put = [&](const char *field, std::string value) {
        if (value.empty())
            return;
        if (read(team_at(donor) + "." + field))
            set[field] = Value(std::move(value));
        else
            skipped.push_back(field);
    };
    const Value &names = rec["names"];
    put("display_name", title_text(names["title"]));
    put("display_name_short", title_text(names["short_name"]));
    put("display_name_super_short", title_text(names["super_short"]));
    put("display_name_code", names["code"].is_string() ? std::string(names["code"].as_string()) : std::string());
    put("livery_name", rec["livery_name"].is_string() ? std::string(rec["livery_name"].as_string()) : std::string());
    put("nationality", rec["nationality"].is_string() ? std::string(rec["nationality"].as_string()) : std::string());
    put("audio_id", rec["audio_id"].is_string() ? std::string(rec["audio_id"].as_string()) : std::string());
    put("prestige", rec["prestige"].is_int() ? std::to_string(rec["prestige"].as_int()) : std::string());
    const Value &c = rec["colours"];
    auto put_colour = [&](const char *field, const Value &v) {
        if (auto n = argb(v))
            put(field, std::to_string(*n));
    };
    put_colour("ui_team_colour", c["menu"]);
    put_colour("primary_colour", c["primary"]);
    put_colour("secondary_colour", c["secondary"]);
    put_colour("tertiary_colour", c["tertiary"]);
    const Value &w = rec["wear"].is_object() ? rec["wear"] : c;
    put_colour("team_wear_primary_colour", w["primary"]);
    put_colour("team_wear_secondary_colour", w["secondary"]);
    put_colour("team_wear_tertiary_colour", w["tertiary"]);
    auto sid = stat_of(stat, TEAM_IDS, id, pin_of(rec));
    if (sid)
        set["stat_id"] = Value(std::to_string(*sid));
    const Value *logo = rec["logo"].is_string() ? lookup("team.logo", std::string(rec["logo"].as_string())) : nullptr;
    bool tower_image = logo && (has_file(*logo, "hud") || has_file(*logo, "broadcast"));
    std::string tower_stock;
    if (logo && !tower_image)
        tower_stock = !stock_key(*logo, "hud").empty() ? stock_key(*logo, "hud") : stock_key(*logo, "broadcast");
    std::string tower_key = tower_image ? label : tower_stock;
    put("timing_tower_texture_path", tower_key);
    push_row(plan, TEAMS, team_at(donor), std::move(set));
    std::size_t sides = 0;
    bool own_pit = refs.pit_record.count(label) != 0;
    for (auto &s : SIDE_TABLES) {
        if (s.pit && own_pit)
            continue;
        sides += copy_side(plan, s, read, donor, label);
    }
    if (rec["logo"].is_string() && !logo)
        plan.lines.push_back("team.identity " + id + ": its logo " + std::string(rec["logo"].as_string()) +
                             " is not a team.logo record; no logo");
    if (logo)
        lower_logo_images(label, std::string(rec["logo"].as_string()), *logo, tower_key, plan);
    for (auto &f : skipped)
        plan.lines.push_back("team.identity " + id + ": " + donor + " has no " + f + "; left out");
    plan.lines.push_back("team.identity " + id + ": row " + label + " from " + donor +
                         (sid ? ", stat id " + std::to_string(*sid) : std::string()) + ", " + std::to_string(sides) +
                         " side row(s)" + (tower_image ? std::string(", own timing tower badge key")
                          : tower_stock.empty() ? std::string() : ", timing tower badge " + tower_stock));
}

inline void lower_pit_times(const std::string &id, const Value &rec, const Lookup &lookup, const Read &read,
                            const TeamRefs &refs, Plan &plan)
{
    plan.owner = addon_of(id);
    std::string team = team_label(rec["team"], refs);
    if (team.empty()) {
        plan.lines.push_back("team.pit_times " + id + ": no team; nothing applied");
        plan.refused++;
        return;
    }
    std::string from;
    if (rec["from"].is_string()) {
        const Value *src = lookup("team.pit_times", std::string(rec["from"].as_string()));
        from = src && (*src)["team"].is_string() ? team_label((*src)["team"], refs) : key_of(rec["from"].as_string());
    } else if (const Value *ident = rec["team"].is_string() ? lookup(TEAM_IDS, std::string(rec["team"].as_string())) : nullptr) {
        if ((*ident)["donor"].is_string())
            from = team_label((*ident)["donor"], refs);
    }
    static constexpr const char *FIELDS[] = {"min_pit_time", "max_pit_time", "min_pit_time_fully_upgraded",
                                              "max_pit_time_fully_upgraded"};
    SideTable table = SIDE_TABLES[0];
    auto have = side_rows(table, read, team);
    Value set = Value::object({{"_metadata_", team}, {"team", team}});
    std::size_t edits = 0;
    for (const char *f : FIELDS)
        if (rec[f].is_number()) {
            set[f] = Value(text(rec[f]));
            edits++;
        }
    if (have[0].found) {
        for (const char *f : FIELDS)
            if (rec[f].is_number())
                push_op(plan, set_op(have[0].path, f, text(rec[f])));
        plan.lines.push_back("team.pit_times " + id + ": " + std::to_string(edits) + " value(s) set on the row of " + team);
        return;
    }
    if (from.empty() || !side_rows(table, read, from)[0].found) {
        plan.lines.push_back("team.pit_times " + id + ": no pit times row of " + (from.empty() ? std::string("(no source)") : from) +
                             " to copy; nothing applied");
        plan.refused++;
        return;
    }
    auto src = side_rows(table, read, from)[0];
    push_row(plan, src.table, src.path, std::move(set));
    plan.lines.push_back("team.pit_times " + id + ": row for " + team + " from " + from +
                         (edits ? ", " + std::to_string(edits) + " value(s) set" : std::string()));
}

struct Dress {
    std::string title, short_name, super_short, code;
    std::optional<int32_t> primary, secondary, tertiary, wear_primary, wear_secondary, wear_tertiary, menu;
    std::string tower_key;
};

inline Dress dress_of(const Value &rec)
{
    Dress d;
    d.title = title_text(rec["names"]["title"]);
    d.short_name = title_text(rec["names"]["short_name"]);
    d.super_short = title_text(rec["names"]["super_short"]);
    if (rec["names"]["code"].is_string())
        d.code = std::string(rec["names"]["code"].as_string());
    const Value &c = rec["colours"];
    d.menu = argb(c["menu"]);
    d.primary = argb(c["primary"]);
    d.secondary = argb(c["secondary"]);
    d.tertiary = argb(c["tertiary"]);
    const Value &w = rec["wear"].is_object() ? rec["wear"] : c;
    d.wear_primary = argb(w["primary"]);
    d.wear_secondary = argb(w["secondary"]);
    d.wear_tertiary = argb(w["tertiary"]);
    return d;
}

namespace team_record {
inline constexpr std::size_t DISPLAY_NAME = 0x0b0, CODE = 0x148, SHORT = 0x1e0, SUPER_SHORT = 0x278,
                             PRIMARY = 0x564, SECONDARY = 0x568, TERTIARY = 0x56c, WEAR_PRIMARY = 0x570,
                             WEAR_SECONDARY = 0x574, WEAR_TERTIARY = 0x578, MENU = 0x57c, HOLDER_CAPACITY = 0x80;
}

inline void write_holder(uint8_t *record, std::size_t at, const std::string &text)
{
    if (text.empty())
        return;
    char *ptr;
    std::memcpy(&ptr, record + at, sizeof ptr);
    if (!ptr)
        return;
    std::size_t n = std::min(text.size(), team_record::HOLDER_CAPACITY - 1);
    std::memcpy(ptr, text.data(), n);
    ptr[n] = 0;
}

inline void write_colour(uint8_t *record, std::size_t at, const std::optional<int32_t> &v)
{
    if (v)
        std::memcpy(record + at, &*v, sizeof(int32_t));
}

inline void write_dress(uint8_t *record, const Dress &d)
{
    using namespace team_record;
    write_holder(record, DISPLAY_NAME, d.title);
    write_holder(record, SHORT, d.short_name);
    write_holder(record, SUPER_SHORT, d.super_short);
    write_holder(record, CODE, d.code);
    write_colour(record, PRIMARY, d.primary);
    write_colour(record, SECONDARY, d.secondary);
    write_colour(record, TERTIARY, d.tertiary);
    write_colour(record, WEAR_PRIMARY, d.wear_primary);
    write_colour(record, WEAR_SECONDARY, d.wear_secondary);
    write_colour(record, WEAR_TERTIARY, d.wear_tertiary);
    write_colour(record, MENU, d.menu);
}

struct Pending {
    std::string group, row, team;
    int slot = 0;
};

struct Live {
    std::function<const Value *(const std::string &id)> identity;
    std::function<int64_t(const std::string &id)> stat_of;
    std::function<uint8_t *(uint16_t stat)> record;
    std::function<void(const std::string &line)> log;
    std::vector<Pending> pending;
    std::map<std::string, std::string> logo_override;
    std::vector<LogoImage> manifest;
};

inline Live &live()
{
    static Live l;
    return l;
}

inline bool dress_slot(const char *group, int slot, const char *row, const char *team)
{
    Live &l = live();
    auto say = [&l](const std::string &line) {
        if (l.log)
            l.log(line);
    };
    std::string where = std::string("dress ") + (group ? group : "") + " slot " + std::to_string(slot) + ": " +
                        (row ? row : "") + " as " + (team ? team : "");
    if (!row || !team || !l.identity || !l.stat_of) {
        say(where + ": the team applier is not set up");
        return false;
    }
    const Value *rec = l.identity(team);
    if (!rec) {
        say(where + ": " + team + " is not a team.identity record");
        return false;
    }
    int64_t stat = l.stat_of(row);
    if (stat < 0 || stat > 0xffff) {
        say(where + ": the slot row has no stat id from the install's store");
        return false;
    }
    uint8_t *record = l.record ? l.record(uint16_t(stat)) : nullptr;
    if (!record) {
        l.pending.push_back({group ? group : "", row, team, slot});
        say(where + ": queued; the loaded team record (stat id " + std::to_string(stat) + ") is not reachable yet");
        return false;
    }
    write_dress(record, dress_of(*rec));
    say(where + ": names and colours written to the loaded record of stat id " + std::to_string(stat));
    return true;
}

}
