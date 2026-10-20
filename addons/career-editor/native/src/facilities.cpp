// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor: My Team facilities (protocol/facilities.md).
//
// Layouts are KB structs (re/db/structs, swofty::game): MyTeamFacilitiesComponent (its
// team map: slots, one control byte per slot, count), FacilityTeamEntry, FacilityTeamData
// (facilities and the packed team key), MyTeamFacility (type, perk tree, workforce cap,
// highest phase seen), FacilityPerkTree (node list) and FacilityNode
// (vt_Career_FacilityNode: state, stat id, phase, cost, days, purchased, ...). Facility and
// node lists hold 16-byte shared/weak pointers {object, control block}.
//
// Completion is the game's own: the daily pass counts days_remaining down on a purchased
// node and at 0 calls Career_Complete_PerkTreeNode (state 1 -> 2, the claim-rewards
// slot that applies the reward set and posts MessageFacilityUpgradeComplete, children
// unlocked). facilities.complete sets days_remaining to 0 and makes that call;
// facilities.set_level also sets purchased first, the way the game's unlock-at-start
// pass does (Career_Complete_FacilityStartNode). Those field stores are the ops' only
// direct writes ("unsafe" in the protocol); a refused completion restores them.

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include <swofty/game.hpp>

#include "ce.hpp"
#include "components.hpp"

using namespace swofty::game;

namespace {

constexpr size_t kMaxBuckets = 4096;
constexpr size_t kMaxFacilities = 16;
constexpr size_t kMaxNodes = 512;
constexpr int kMaxDepth = 32;

// std::shared_ptr / weak_ptr element {object, control block}: 16 bytes, use count at
// +0x08 of the control block (library layout, not a game class).
constexpr size_t kPtrPair = 16;
constexpr size_t kCtrlUses = 0x08;

constexpr uint8_t kLocked = 0;
constexpr uint8_t kUnlocked = 1;
constexpr uint8_t kComplete = 2;
constexpr int kMaxLevel = 4;
constexpr int kMaxPasses = 16;

FacilityNode *node_of(uintptr_t n) { return reinterpret_cast<FacilityNode *>(n); }

const MyTeamFacility *fac_of(uintptr_t f) { return reinterpret_cast<const MyTeamFacility *>(f); }

uintptr_t first(uintptr_t pair) { return *reinterpret_cast<const uintptr_t *>(pair); }

uintptr_t second(uintptr_t pair) { return *reinterpret_cast<const uintptr_t *>(pair + sizeof(uintptr_t)); }

const char *type_name(unsigned t)
{
    static const char *const k[] = {"engineering", "personnel", "corporate"};
    return t < 3 ? k[t] : "other";
}

struct Node {
    uintptr_t at = 0;
    uint32_t stat = 0;
};

struct Facility {
    uintptr_t at = 0;
    uint8_t type = 0;
    size_t index = 0;
    std::vector<Node> nodes;
};

struct TeamFac {
    uint32_t team = 0;
    bool player = false;
    std::vector<Facility> facs;
};

struct Walk {
    std::vector<TeamFac> teams;
    std::string why;
};

bool same_key(uint32_t a, uint32_t b) { return (a & 0xffff00ffu) == (b & 0xffff00ffu); }

std::string node_name(uintptr_t n)
{
    uintptr_t s = reinterpret_cast<uintptr_t>(node_of(n)->name_loc_tag);
    if (!s)
        return {};
    const char *p = reinterpret_cast<const char *>(s);
    if (ce::comp::readable(s, 64))
        return std::string(p, strnlen(p, 64));
    std::string out;
    for (size_t i = 0; i < 64; ++i) {
        char c = 0;
        if (!ce::comp::read(s + i, c) || !c)
            break;
        out.push_back(c);
    }
    return out;
}

bool live(uintptr_t ctrl)
{
    int32_t uses = 0;
    return ctrl && ce::comp::read(ctrl + kCtrlUses, uses) && uses > 0;
}

void collect(uintptr_t b, uintptr_t e, int depth, std::vector<Node> &out)
{
    if (depth > kMaxDepth || !b || e < b || (e - b) % kPtrPair || (e - b) / kPtrPair > kMaxNodes ||
        !ce::comp::readable(b, e - b))
        return;
    uintptr_t vt = vt_Career_FacilityNode.address();
    for (uintptr_t p = b; p < e && out.size() < kMaxNodes; p += kPtrPair) {
        uintptr_t obj = first(p);
        uintptr_t ctrl = second(p);
        if (!obj || !live(ctrl) || !ce::comp::readable(obj, sizeof(FacilityNode)))
            continue;
        const FacilityNode *node = node_of(obj);
        if (reinterpret_cast<uintptr_t>(node->vtable) != vt)
            continue;
        bool seen = false;
        for (auto &x : out)
            if (x.at == obj) {
                seen = true;
                break;
            }
        if (seen)
            continue;
        Node n;
        n.at = obj;
        n.stat = node->stat_id;
        out.push_back(n);
        collect(reinterpret_cast<uintptr_t>(node->children_begin), reinterpret_cast<uintptr_t>(node->children_end),
                depth + 1, out);
    }
}

void nodes_of(uintptr_t tree, std::vector<Node> &out)
{
    const auto *t = reinterpret_cast<const FacilityPerkTree *>(tree);
    collect(reinterpret_cast<uintptr_t>(t->nodes_begin), reinterpret_cast<uintptr_t>(t->nodes_end), 0, out);
}

bool walk(Walk &w)
{
    uintptr_t comp = ce::comp::get(ce::comp::Id::MyTeamFacilities);
    if (!comp) {
        w.why = "not a My Team career";
        return false;
    }
    if (!ce::comp::readable(comp, sizeof(MyTeamFacilitiesComponent))) {
        w.why = "component unreadable";
        return false;
    }
    const auto *mc = reinterpret_cast<const MyTeamFacilitiesComponent *>(comp);
    uintptr_t slots = reinterpret_cast<uintptr_t>(mc->team_slots);
    uintptr_t ctrl = reinterpret_cast<uintptr_t>(mc->team_ctrl);
    uint64_t count = mc->team_count;
    if (!slots || ctrl <= slots || (ctrl - slots) % sizeof(FacilityTeamEntry *)) {
        w.why = count ? "team map layout not as expected" : "no facility data";
        return false;
    }
    size_t nb = (ctrl - slots) / sizeof(FacilityTeamEntry *);
    if (nb > kMaxBuckets || !ce::comp::readable(slots, nb * sizeof(FacilityTeamEntry *)) || !ce::comp::readable(ctrl, nb)) {
        w.why = "team map layout not as expected";
        return false;
    }
    ce::comp::Entrant me{};
    bool have_player = ce::comp::player(me);
    size_t used = 0;
    for (size_t i = 0; i < nb; ++i) {
        if (!mc->team_ctrl[i])
            continue;
        ++used;
        const FacilityTeamEntry *entry = mc->team_slots[i];
        if (!entry || !ce::comp::readable(reinterpret_cast<uintptr_t>(entry), sizeof(FacilityTeamEntry)))
            continue;
        TeamFac t;
        const FacilityTeamData *data = entry->data;
        if (!data || !ce::comp::readable(reinterpret_cast<uintptr_t>(data), sizeof(FacilityTeamData)))
            continue;
        t.team = data->team_key;
        t.player = have_player && same_key(t.team, me.team);
        uintptr_t fb = reinterpret_cast<uintptr_t>(data->facilities_begin);
        uintptr_t fe = reinterpret_cast<uintptr_t>(data->facilities_end);
        if (!fb || fe < fb || (fe - fb) % kPtrPair || (fe - fb) / kPtrPair > kMaxFacilities ||
            !ce::comp::readable(fb, fe - fb))
            continue;
        for (uintptr_t p = fb; p < fe; p += kPtrPair) {
            uintptr_t f = first(p);
            if (!f || !ce::comp::readable(f, sizeof(MyTeamFacility)))
                continue;
            Facility fac;
            fac.at = f;
            fac.type = fac_of(f)->facility_type;
            fac.index = t.facs.size();
            uintptr_t tree = reinterpret_cast<uintptr_t>(fac_of(f)->tree);
            if (tree && ce::comp::readable(tree, sizeof(FacilityPerkTree)))
                nodes_of(tree, fac.nodes);
            t.facs.push_back(std::move(fac));
        }
        w.teams.push_back(std::move(t));
    }
    if (used != count) {
        w.teams.clear();
        w.why = "team map count mismatch";
        return false;
    }
    if (w.teams.size() == 1 && !have_player)
        w.teams[0].player = true;
    return true;
}

int node_code(uintptr_t n)
{
    uint8_t s = node_of(n)->state;
    if (s == kComplete)
        return 3;
    if (s == kLocked)
        return 0;
    return node_of(n)->purchased ? 2 : 1;
}

const char *node_status(uintptr_t n)
{
    static const char *const k[] = {"locked", "available", "building", "built"};
    return k[node_code(n)];
}

bool building(uintptr_t n) { return node_of(n)->state == kUnlocked && node_of(n)->purchased; }

int level_of(const Facility &f)
{
    int lv = 0;
    for (auto &n : f.nodes)
        if (node_of(n.at)->state == kComplete)
            lv = std::max(lv, node_of(n.at)->phase);
    return lv;
}

void opt_int(ce::Writer &w, const char *k, int64_t v)
{
    if (!v)
        return;
    w.key(k);
    w.integer(v);
}

void write_node(ce::Writer &w, uintptr_t n)
{
    int code = node_code(n);
    w.begin_object();
    w.key("i");
    w.integer(node_of(n)->stat_id);
    w.key("n");
    w.str(node_name(n));
    w.key("s");
    w.integer(code);
    opt_int(w, "p", node_of(n)->phase);
    opt_int(w, "lv", node_of(n)->ui_level);
    opt_int(w, "mx", node_of(n)->ui_max_level);
    opt_int(w, "c", node_of(n)->cost);
    opt_int(w, "dd", node_of(n)->days_to_complete);
    if (code == 2) {
        w.key("dl");
        w.integer(node_of(n)->days_remaining);
    }
    w.end_object();
}

void write_facility(ce::Writer &w, const Facility &f, bool with_nodes)
{
    unsigned built = 0, avail = 0;
    for (auto &n : f.nodes) {
        int code = node_code(n.at);
        built += code == 3;
        avail += code == 1;
    }
    w.begin_object();
    w.key("ty");
    w.integer(f.type);
    w.key("lv");
    w.integer(level_of(f));
    w.key("hp");
    w.integer(fac_of(f.at)->highest_phase_seen);
    if (fac_of(f.at)->workforce_cap_set) {
        w.key("wc");
        w.integer(fac_of(f.at)->workforce_cap);
    }
    w.key("nt");
    w.integer(static_cast<int64_t>(f.nodes.size()));
    w.key("nb");
    w.integer(built);
    w.key("na");
    w.integer(avail);
    w.key("bld");
    w.begin_array();
    for (auto &n : f.nodes)
        if (building(n.at)) {
            w.begin_object();
            w.key("i");
            w.integer(n.stat);
            w.key("dl");
            w.integer(node_of(n.at)->days_remaining);
            w.key("dd");
            w.integer(node_of(n.at)->days_to_complete);
            w.end_object();
        }
    w.end_array();
    if (with_nodes) {
        w.key("nodes");
        w.begin_array();
        for (auto &n : f.nodes)
            write_node(w, n.at);
        w.end_array();
    }
    w.end_object();
}

TeamFac *pick_team(Walk &wk, const ce::Json &args)
{
    const ce::Json &v = args["team"];
    if (v.type == ce::Json::Null || (v.type == ce::Json::Str && v.s == "player")) {
        for (auto &t : wk.teams)
            if (t.player)
                return &t;
        return nullptr;
    }
    long long k = v.num(-1);
    if (k < 0 || k > 0xffffffffll)
        return nullptr;
    for (auto &t : wk.teams)
        if (t.team == static_cast<uint32_t>(k))
            return &t;
    return nullptr;
}

int parse_facility(const ce::Json &v)
{
    if (v.type == ce::Json::Num) {
        long long n = v.num(-1);
        return n >= 0 && n <= 2 ? static_cast<int>(n) : -2;
    }
    if (v.type == ce::Json::Str) {
        for (int t = 0; t < 3; ++t)
            if (v.s == type_name(static_cast<unsigned>(t)))
                return t;
        return -2;
    }
    return v.type == ce::Json::Null ? -1 : -2;
}

ce::OpResult fail(const char *code, const std::string &after = {})
{
    ce::OpResult r;
    r.error = code;
    r.after = after;
    return r;
}

std::string team_json(const TeamFac &t, int type)
{
    ce::Writer w;
    w.begin_object();
    w.key("team");
    w.integer(t.team);
    w.key("f");
    w.begin_array();
    for (auto &f : t.facs)
        if (type < 0 || f.type == type)
            write_facility(w, f, false);
    w.end_array();
    w.end_object();
    return w.text();
}

bool complete_node(uintptr_t n, bool purchase, bool seen)
{
    int32_t days = node_of(n)->days_remaining;
    uint8_t bought = node_of(n)->purchased;
    if (purchase && !bought)
        node_of(n)->purchased = 1;
    node_of(n)->days_remaining = 0;
    bool done = Career_Complete_PerkTreeNode(reinterpret_cast<void *>(n)) != 0 &&
                node_of(n)->state == kComplete;
    if (!done) {
        node_of(n)->days_remaining = days;
        node_of(n)->purchased = bought;
        return false;
    }
    if (seen)
        node_of(n)->seen_completed = 1;
    return true;
}

void log_node(const char *op, const TeamFac &t, const Facility &f, uintptr_t n, const char *from, int32_t days,
              bool ok)
{
    ce::log("%s team=0x%08x %s node %u \"%s\" phase %d: %s (%d days left) -> %s%s", op, t.team,
            type_name(f.type), node_of(n)->stat_id, node_name(n).c_str(), node_of(n)->phase, from,
            days, node_status(n), ok ? "" : " (game refused, fields restored)");
}

ce::OpResult op_complete(const ce::Json &args)
{
    Walk wk;
    if (!walk(wk))
        return fail(wk.why == "not a My Team career" ? "mode_unsupported" : "not_ready");
    TeamFac *t = pick_team(wk, args);
    if (!t)
        return fail("no_team");
    int type = parse_facility(args["facility"]);
    if (type == -2)
        return fail("bad_args");
    long long want = -1;
    if (args["node"].type == ce::Json::Num) {
        want = args["node"].num(-1);
        if (want < 0 || want > 0xffffffffll)
            return fail("bad_args");
    }
    std::string before = team_json(*t, type);
    size_t tried = 0, done = 0;
    for (auto &f : t->facs) {
        if (type >= 0 && f.type != type)
            continue;
        for (auto &n : f.nodes) {
            if (want >= 0 && n.stat != static_cast<uint32_t>(want))
                continue;
            bool available = node_code(n.at) == 1;
            if (!building(n.at) && !(want >= 0 && available)) {
                if (want >= 0)
                    return fail("wrong_state", before);
                continue;
            }
            ++tried;
            int32_t days = node_of(n.at)->days_remaining;
            bool ok = complete_node(n.at, available, available);
            done += ok;
            log_node("facilities.complete", *t, f, n.at, available ? "available" : "building", days, ok);
        }
    }
    if (!tried)
        return fail(want >= 0 ? "no_node" : "nothing_building", before);
    ce::OpResult r;
    r.before = before;
    r.after = team_json(*t, type);
    r.ok = done > 0;
    if (!r.ok)
        r.error = "game_refused";
    ce::log("facilities.complete team=0x%08x: %zu of %zu builds completed", t->team, done, tried);
    return r;
}

ce::OpResult op_set_level(const ce::Json &args)
{
    Walk wk;
    if (!walk(wk))
        return fail(wk.why == "not a My Team career" ? "mode_unsupported" : "not_ready");
    TeamFac *t = pick_team(wk, args);
    if (!t)
        return fail("no_team");
    int type = parse_facility(args["facility"]);
    long long level = args["level"].num(-1);
    if (type < 0 || args["level"].type != ce::Json::Num || level < 1 || level > kMaxLevel)
        return fail("bad_args");
    Facility *fac = nullptr;
    for (auto &f : t->facs)
        if (f.type == type)
            fac = &f;
    if (!fac)
        return fail("no_facility");
    std::string before = team_json(*t, type);
    size_t done = 0, refused = 0;
    for (int pass = 0; pass < kMaxPasses; ++pass) {
        bool moved = false;
        for (auto &n : fac->nodes) {
            if (node_of(n.at)->state != kUnlocked || node_of(n.at)->phase > level)
                continue;
            bool was_building = node_of(n.at)->purchased != 0;
            int32_t days = node_of(n.at)->days_remaining;
            bool ok = complete_node(n.at, true, !was_building);
            log_node("facilities.set_level", *t, *fac, n.at, was_building ? "building" : "available", days, ok);
            if (ok) {
                ++done;
                moved = true;
            } else {
                ++refused;
            }
        }
        if (!moved)
            break;
    }
    ce::OpResult r;
    r.before = before;
    r.after = team_json(*t, type);
    r.ok = done > 0 || (refused == 0 && level_of(*fac) >= level);
    if (!r.ok)
        r.error = refused ? "game_refused" : "nothing_to_complete";
    ce::log("facilities.set_level team=0x%08x %s level %lld: %zu nodes completed, %zu refused, level now "
            "%d",
            t->team, type_name(static_cast<unsigned>(type)), level, done, refused, level_of(*fac));
    return r;
}

void section_facilities(ce::Writer &w)
{
    w.begin_object();
    Walk wk;
    bool in = ce::in_career();
    bool ok = in && walk(wk);
    w.key("ready");
    w.boolean(ok);
    w.key("available");
    w.boolean(in && ce::comp::get(ce::comp::Id::MyTeamFacilities) != 0);
    if (!ok) {
        w.key("reason");
        w.str(in ? wk.why : "no career loaded");
        w.end_object();
        return;
    }
    const TeamFac *mine = nullptr;
    for (auto &t : wk.teams)
        if (t.player)
            mine = &t;
    w.key("team");
    if (mine)
        w.integer(mine->team);
    else
        w.null();
    w.key("f");
    w.begin_array();
    if (mine)
        for (auto &f : mine->facs)
            write_facility(w, f, true);
    w.end_array();
    w.key("teams");
    w.begin_array();
    for (auto &t : wk.teams) {
        w.begin_object();
        w.key("team");
        w.integer(t.team);
        w.key("player");
        w.boolean(t.player);
        w.key("lv");
        w.begin_array();
        for (auto &f : t.facs) {
            w.begin_array();
            w.integer(f.type);
            w.integer(level_of(f));
            w.end_array();
        }
        w.end_array();
        w.end_object();
    }
    w.end_array();
    w.end_object();
}

} // namespace

CE_SECTION("facilities", section_facilities);
CE_OP("facilities.complete", op_complete);
CE_OP("facilities.set_level", op_set_level);
