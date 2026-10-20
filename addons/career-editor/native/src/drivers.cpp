// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor: drivers (ratings, line-up swaps, contract money, the drivers snapshot).
//
// Layouts are KB structs (re/db/structs, generated into swofty/game/career.hpp and entrants.hpp):
//   DriverStatisticsComponent  the career's statistics (My Team; the Driver Career subclass
//                              DriverCareerPlayerDriverStatisticsComponent shares it), found
//                              through ce::comp. Its getters take &stats_data (the embedded
//                              driverStatistics::Data).
//   DriverStatsEntry           one driverData element: DriverStatId + default and context sets
//   ContextStatSet, DriverStatistic, EvolutionCurve, CurvePoint, StatInfo
//   CareerTimeComponent        clock and season dates for the curve progress
//   EntrantRecord, EntrantWriteData, CareerEntity   the championship line-up and its writer
//   CareerDriverContractRenewalComponent, DriverMoveRecord   the game's move log
//   DriverContractsMyTeamComponent, ContractRecord           My Team contract lists
//
// A rating edit rewrites the y of every point of the curve the getter uses (count and x kept, so
// nothing is reallocated), then rebuilds lut[] and the end value with the game's own evaluator
// (Career_Eval_Curve), exactly as the game's curve loader does after parsing points. Dynamic
// statistics have no curve: their dynamic_level moves instead. Everything is read back through the
// game's getter for the same driver, and every other driver's value is compared before and after.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <unordered_map>
#include <vector>

#include <swofty/db.hpp>
#include <swofty/career.hpp>
#include <swofty/mem.hpp>
#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "ce.hpp"
#include "components.hpp"

using namespace swofty::game;

namespace {

using StatsComp = DriverStatisticsComponent;

constexpr uintptr_t kCompData = offsetof(StatsComp, stats_data);
constexpr uintptr_t kCompCtx = offsetof(StatsComp, stat_context);
constexpr uintptr_t kCompTime = offsetof(StatsComp, career_time);
constexpr uintptr_t kCompInfos = offsetof(StatsComp, stat_infos_begin);
constexpr uintptr_t kCompGate = offsetof(StatsComp, use_story_mode_data);
constexpr uintptr_t kCompApplied = offsetof(StatsComp, last_patch_applied_checksum);
constexpr uintptr_t kCompDeclined = offsetof(StatsComp, last_patch_declined_checksum);
constexpr uintptr_t kCompDrivers = offsetof(StatsComp, driver_data_begin);
constexpr uintptr_t kTimeDay = offsetof(CareerTimeComponent, current_minutes);
constexpr uintptr_t kTimeLength = offsetof(CareerTimeComponent, season_dates);

constexpr uintptr_t kDriverSize = sizeof(DriverStatsEntry);
constexpr uintptr_t kDriverSetStats = offsetof(DriverStatsEntry, stats_begin);
constexpr uintptr_t kDriverCtx = offsetof(DriverStatsEntry, context_sets_begin);
constexpr uintptr_t kCtxSize = sizeof(ContextStatSet);
constexpr uintptr_t kCtxMaskA = offsetof(ContextStatSet, mask_a);
constexpr uintptr_t kCtxMaskB = offsetof(ContextStatSet, mask_b);
constexpr uintptr_t kCtxStats = offsetof(ContextStatSet, stats_begin);

constexpr uintptr_t kStatSize = sizeof(DriverStatistic);
constexpr uintptr_t kStatInfo = offsetof(DriverStatistic, info_stat_id);
constexpr uintptr_t kStatCurve = offsetof(DriverStatistic, evolution_graph);
constexpr uintptr_t kStatPatched = offsetof(DriverStatistic, patched_evolution_graph);
constexpr uintptr_t kStatHasPatched = offsetof(DriverStatistic, has_patched_graph);
constexpr uintptr_t kStatBoost = offsetof(DriverStatistic, temporary_boost_value);
constexpr uintptr_t kStatDynamic = offsetof(DriverStatistic, is_dynamic_stat);
constexpr uintptr_t kStatLevel = offsetof(DriverStatistic, dynamic_level);

constexpr uintptr_t kInfoId = offsetof(StatInfo, stat_id);
constexpr uintptr_t kInfoWeight = offsetof(StatInfo, weight);
constexpr uintptr_t kInfoDefault = offsetof(StatInfo, default_value);

constexpr uintptr_t kCurveEnd = offsetof(EvolutionCurve, end_value);
constexpr uintptr_t kCurveMinY = offsetof(EvolutionCurve, min_y);
constexpr uintptr_t kCurveMaxY = offsetof(EvolutionCurve, max_y);
constexpr uintptr_t kCurveLut = offsetof(EvolutionCurve, lut);
constexpr uintptr_t kCurvePoints = offsetof(EvolutionCurve, points_begin);
constexpr size_t kLutSize = sizeof(EvolutionCurve::lut);
constexpr size_t kPointSize = sizeof(CurvePoint);
constexpr size_t kPointY = offsetof(CurvePoint, y);

// Vtable slot offsets of DriverStatisticsComponent and of its context provider (slot indexes, not
// fields): +0x268 min, +0x270 max, +0x278 fixed progress, +0x280 Career_Get_DriverStatValue.
constexpr uintptr_t kVtMin = 0x268;
constexpr uintptr_t kVtMax = 0x270;
constexpr uintptr_t kVtFixedProgress = 0x278;
constexpr uintptr_t kVtGetStat = 0x280;
constexpr uintptr_t kCtxVtFill = 0xc8;

// Sanity bounds for vectors read from game memory (not game fields).
constexpr size_t kMaxDrivers = 1024;
constexpr size_t kMaxStats = 64;
constexpr size_t kMaxSets = 32;
constexpr size_t kMaxPoints = 256;
constexpr size_t kMaxInfos = 64;
constexpr size_t kUndoCap = 64;

using GetStatFn = float (*)(void *self, void *data, const uint32_t *driver, const uint16_t *info, const uint8_t *ctx,
                            float x, bool patched, bool clamp, bool round);
using BoundFn = float (*)(void *self);
using FixedFn = bool (*)(void *self, const uint32_t *driver);
using CtxFn = void (*)(void *self, uint8_t *out);

template <class T> bool rd(uintptr_t a, T &out)
{
    return swofty::mem::read(a, out);
}

template <class T> T rdv(uintptr_t a, T def = T{})
{
    T v{};
    return swofty::mem::read(a, v) ? v : def;
}

bool writable(uintptr_t p, size_t n)
{
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(reinterpret_cast<const void *>(p), &mbi, sizeof mbi) != sizeof mbi)
        return false;
    if (mbi.State != MEM_COMMIT || (mbi.Protect & PAGE_GUARD))
        return false;
    DWORD w = PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY;
    if (!(mbi.Protect & w))
        return false;
    return p + n <= reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
}

template <class T> bool wr(uintptr_t a, const T &v)
{
    if (!writable(a, sizeof v))
        return false;
    std::memcpy(reinterpret_cast<void *>(a), &v, sizeof v);
    return true;
}

uint32_t save_id(uint32_t key)
{
    return ((key & 0xff) << 14) | (key >> 16);
}

uint32_t key_from_save(uint32_t id)
{
    return ((id >> 14) & 0x3) | ((id & 0x3fff) << 16);
}

// ---------------------------------------------------------------------------
// The component and its vectors.

struct Comp {
    uintptr_t at = 0;
    uintptr_t data = 0;
    uintptr_t vt = 0;
    bool player_class = false;
};

bool vec(uintptr_t at, size_t stride, size_t cap, uintptr_t &b, size_t &n)
{
    uintptr_t e = 0;
    if (!rd(at, b) || !rd(at + 8, e))
        return false;
    if (!b || e < b || (e - b) % stride || (e - b) / stride > cap)
        return false;
    n = (e - b) / stride;
    return n == 0 || swofty::mem::readable(b, e - b);
}

bool find_comp(Comp &c, const char **why)
{
    c = {};
    uintptr_t p = ce::comp::get(ce::comp::Id::DriverCareerPlayerDriverStatistics);
    c.player_class = p != 0;
    if (!p)
        p = ce::comp::get(ce::comp::Id::DriverStatistics);
    if (!p) {
        *why = "no DriverStatisticsComponent on the championship entity";
        return false;
    }
    uintptr_t vt = 0;
    if (!rd(p, vt) || !swofty::mem::readable(vt, kVtGetStat + sizeof(void *))) {
        *why = "component vtable unreadable";
        return false;
    }
    uintptr_t b;
    size_t n;
    if (!vec(p + kCompDrivers, kDriverSize, kMaxDrivers, b, n)) {
        *why = "driver table unreadable";
        return false;
    }
    c.at = p;
    c.data = p + kCompData;
    c.vt = vt;
    return true;
}

template <class F> F slot(const Comp &c, uintptr_t off)
{
    uintptr_t fn = rdv<uintptr_t>(c.vt + off);
    return swofty::mem::in_image(fn) ? reinterpret_cast<F>(fn) : nullptr;
}

uintptr_t find_driver(const Comp &c, uint32_t key)
{
    uintptr_t b;
    size_t n;
    if (!vec(c.at + kCompDrivers, kDriverSize, kMaxDrivers, b, n))
        return 0;
    for (size_t i = 0; i < n; ++i) {
        uintptr_t e = b + i * kDriverSize;
        if (rdv<uint8_t>(e + offsetof(DriverStatsEntry, kind)) == (key & 0xff) &&
            rdv<uint16_t>(e + offsetof(DriverStatsEntry, stat_id)) == (key >> 16))
            return e;
    }
    return 0;
}

bool context(const Comp &c, uint8_t ctx[16])
{
    std::memset(ctx, 0, 16);
    uintptr_t obj = rdv<uintptr_t>(c.at + kCompCtx);
    uintptr_t vt = obj ? rdv<uintptr_t>(obj) : 0;
    uintptr_t fn = vt ? rdv<uintptr_t>(vt + kCtxVtFill) : 0;
    if (!swofty::mem::in_image(fn))
        return false;
    reinterpret_cast<CtxFn>(fn)(reinterpret_cast<void *>(obj), ctx);
    return true;
}

bool patched_gate(const Comp &c)
{
    return rdv<uint8_t>(c.at + kCompGate) == 0 && rdv<uint32_t>(c.at + kCompApplied) >= 2;
}

float progress(const Comp &c, uint32_t key)
{
    auto fixed = slot<FixedFn>(c, kVtFixedProgress);
    if (!fixed || fixed(reinterpret_cast<void *>(c.at), &key))
        return 0.0f;
    uintptr_t time = rdv<uintptr_t>(c.at + kCompTime);
    if (!time || !swofty::mem::readable(time + kTimeLength, kTimeDay - kTimeLength + sizeof(uint32_t)))
        return 0.0f;
    uint32_t clock = rdv<uint32_t>(time + kTimeDay);
    uint32_t len = Career_Get_CareerLengthDays_Thunk(reinterpret_cast<void *>(time + kTimeLength));
    if (!len)
        return 0.0f;
    return std::min(1.0f, static_cast<float>(clock) / static_cast<float>(len));
}

float stat_value(const Comp &c, uint32_t key, uint16_t info, bool clamp, bool round)
{
    auto get = slot<GetStatFn>(c, kVtGetStat);
    if (!get)
        return NAN;
    uint8_t ctx[16];
    context(c, ctx);
    return get(reinterpret_cast<void *>(c.at), reinterpret_cast<void *>(c.data), &key, &info, ctx,
               progress(c, key), patched_gate(c), clamp, round);
}

float overall(const Comp &c, uint32_t key)
{
    return Career_Get_DriverOverallRating(reinterpret_cast<void *>(c.at), reinterpret_cast<void *>(c.data), &key,
                                          true);
}

// Every statistic set of a driver: the default one first, then the context-aware ones.
std::vector<uintptr_t> sets_of(uintptr_t driver)
{
    std::vector<uintptr_t> out{driver + kDriverSetStats};
    uintptr_t b;
    size_t n;
    if (vec(driver + kDriverCtx, kCtxSize, kMaxSets, b, n))
        for (size_t i = 0; i < n; ++i)
            out.push_back(b + i * kCtxSize + kCtxStats);
    return out;
}

// The set the game's getter reads for this context (same match as the getter).
uintptr_t active_set(uintptr_t driver, const uint8_t ctx[16])
{
    uintptr_t b;
    size_t n;
    if (vec(driver + kDriverCtx, kCtxSize, kMaxSets, b, n)) {
        for (size_t i = 0; i < n; ++i) {
            uintptr_t e = b + i * kCtxSize;
            uint32_t ma = rdv<uint32_t>(e + kCtxMaskA), mb = rdv<uint32_t>(e + kCtxMaskB);
            bool a = !ctx[1] || (ctx[0] < 32 && (ma & (1u << ctx[0])));
            bool bb = !ctx[3] || (ctx[2] < 32 && (mb & (1u << ctx[2])));
            if (a && bb)
                return e + kCtxStats;
        }
    }
    return driver + kDriverSetStats;
}

uintptr_t find_stat(uintptr_t set, uint16_t info)
{
    uintptr_t b;
    size_t n;
    if (!vec(set, kStatSize, kMaxStats, b, n))
        return 0;
    for (size_t i = 0; i < n; ++i)
        if (rdv<uint16_t>(b + i * kStatSize + kStatInfo) == info)
            return b + i * kStatSize;
    return 0;
}

uintptr_t used_curve(const Comp &c, uintptr_t stat)
{
    if (patched_gate(c) && rdv<uint8_t>(stat + kStatHasPatched))
        return stat + kStatPatched;
    return stat + kStatCurve;
}

struct CurveView {
    uintptr_t pts = 0;
    size_t n = 0;
    float miny = 0, maxy = 100;
};

bool curve_view(uintptr_t curve, CurveView &v)
{
    if (!vec(curve + kCurvePoints, kPointSize, kMaxPoints, v.pts, v.n) || v.n < 4)
        return false;
    v.miny = rdv<float>(curve + kCurveMinY);
    v.maxy = rdv<float>(curve + kCurveMaxY);
    return std::isfinite(v.miny) && std::isfinite(v.maxy) && v.maxy > v.miny;
}

// Career_Eval_Curve returns 0 outside 0..1 instead of clamping, so x is clamped here.
float eval_curve(uintptr_t curve, float x)
{
    if (!(x >= 0.0f))
        x = 0.0f;
    if (x > 1.0f)
        x = 1.0f;
    return Career_Eval_Curve(reinterpret_cast<void *>(curve), x);
}

float curve_at(uintptr_t curve, const CurveView &v, float x)
{
    return eval_curve(curve, x) * (v.maxy - v.miny) + v.miny;
}

bool curve_rebuild(uintptr_t curve, const CurveView &v)
{
    if (!writable(curve + kCurveEnd, kCurveLut + kLutSize - kCurveEnd))
        return false;
    uint8_t lut[kLutSize];
    for (size_t i = 0; i < kLutSize; ++i) {
        float y = eval_curve(curve, static_cast<float>(i) * 0.015625f);
        int q = static_cast<int>(0.5f + y * 255.0f);
        lut[i] = static_cast<uint8_t>(q < 1 ? 0 : q > 254 ? 255 : q);
    }
    std::memcpy(reinterpret_cast<void *>(curve + kCurveLut), lut, sizeof lut);
    float end = rdv<float>(v.pts + (v.n - 2) * kPointSize + kPointY) * (v.maxy - v.miny) + v.miny;
    return wr(curve + kCurveEnd, end);
}

bool curve_read_y(const CurveView &v, std::vector<float> &ys)
{
    ys.resize(v.n);
    for (size_t i = 0; i < v.n; ++i)
        if (!rd(v.pts + i * kPointSize + kPointY, ys[i]))
            return false;
    return true;
}

bool curve_write_y(uintptr_t curve, const CurveView &v, const std::vector<float> &ys)
{
    if (ys.size() != v.n || !writable(v.pts, v.n * kPointSize))
        return false;
    for (size_t i = 0; i < v.n; ++i)
        std::memcpy(reinterpret_cast<void *>(v.pts + i * kPointSize + kPointY), &ys[i], sizeof(float));
    return curve_rebuild(curve, v);
}

// ---------------------------------------------------------------------------
// Names from the gamedb (entrants.drivers, entrants.teams, driver_statistics_info).
//
// Strings come from the db library's string_at (it decodes the game's encoded string records);
// integers from int_at. Every string is checked again here: 1..63 bytes of printable UTF-8, or no name.

constexpr size_t kNameMax = 64; // longest name the snapshot accepts (sanity bound, not a game field)

struct Names {
    bool loaded = false;
    std::unordered_map<uint32_t, std::string> driver, initials, team, stat;
    std::unordered_map<uint32_t, int> number;
};

Names g_names;

bool db_text(const std::string &path, std::string &out)
{
    out.clear();
    const char *v = nullptr;
    if (swofty::db::string_at(path, &v) != swofty::db::Result::ok || !v)
        return false;
    size_t n = strnlen(v, kNameMax);
    if (!n || n >= kNameMax || !swofty::mem::printable_utf8(v, n))
        return false;
    out.assign(v, n);
    return true;
}

bool db_int(const std::string &path, int64_t &out)
{
    return swofty::db::int_at(path, &out) == swofty::db::Result::ok;
}

template <class F> void each_row(const std::string &table, const std::string &stem, F f)
{
    int64_t count = 0;
    if (!db_int(table + "._" + stem + "_instancecount_", count) || count <= 0 || count > 4096)
        return;
    for (int64_t i = 0; i < count; ++i)
        f(table + "." + stem + "_" + std::to_string(i));
}

int g_name_tries = 0;

const Names &names()
{
    if (g_names.loaded || !swofty::db::available())
        return g_names;
    if (g_name_tries++ % 30 != 0)
        return g_names;
    each_row("entrants.drivers", "driver", [](const std::string &row) {
        int64_t id;
        if (!db_int(row + ".stat_id", id) || id < 0 || id > 0xffff)
            return;
        uint32_t k = static_cast<uint32_t>(id);
        std::string v;
        if (db_text(row + "._metadata_", v))
            g_names.driver[k] = v;
        if (db_text(row + ".initials", v))
            g_names.initials[k] = v;
        int64_t num;
        if (db_int(row + ".car_number", num) && num >= 0 && num < 1000)
            g_names.number[k] = static_cast<int>(num);
    });
    each_row("entrants.teams", "team", [](const std::string &row) {
        int64_t id;
        std::string v;
        if (db_int(row + ".stat_id", id) && id >= 0 && id <= 0xffff && db_text(row + "._metadata_", v))
            g_names.team[static_cast<uint32_t>(id)] = v;
    });
    each_row("driver_statistics.driver_statistics.driver_statistics_info", "StatisticInfo",
             [](const std::string &row) {
                 int64_t id;
                 std::string v;
                 if (db_int(row + ".stat_id", id) && id >= 0 && id <= 0xffff && db_text(row + "._metadata_", v))
                     g_names.stat[static_cast<uint32_t>(id)] = v;
             });
    g_names.loaded = !g_names.driver.empty();
    if (g_names.loaded || g_name_tries <= 90)
        ce::log("drivers: gamedb names loaded: %zu drivers, %zu teams, %zu statistics",
            g_names.driver.size(), g_names.team.size(), g_names.stat.size());
    return g_names;
}

std::string stat_name(uint16_t info)
{
    auto &m = names().stat;
    auto it = m.find(info);
    if (it != m.end() && !it->second.empty())
        return it->second;
    return "stat_" + std::to_string(info);
}

struct InfoRow {
    uint16_t id;
    float weight;
    float def;
};

std::vector<InfoRow> infos(const Comp &c)
{
    std::vector<InfoRow> out;
    uintptr_t b;
    size_t n;
    if (!vec(c.at + kCompInfos, 16, kMaxInfos, b, n))
        return out;
    for (size_t i = 0; i < n; ++i) {
        uintptr_t p = rdv<uintptr_t>(b + i * 16);
        if (!p || !swofty::mem::readable(p, kInfoDefault + sizeof(float)))
            continue;
        out.push_back({rdv<uint16_t>(p + kInfoId), rdv<float>(p + kInfoWeight), rdv<float>(p + kInfoDefault)});
    }
    return out;
}

std::vector<uint16_t> stat_ids(uintptr_t driver)
{
    std::vector<uint16_t> out;
    uintptr_t b;
    size_t n;
    if (vec(driver + kDriverSetStats, kStatSize, kMaxStats, b, n))
        for (size_t i = 0; i < n; ++i)
            out.push_back(rdv<uint16_t>(b + i * kStatSize + kStatInfo));
    return out;
}

bool resolve_stat(uintptr_t driver, const std::string &k, uint16_t &out)
{
    char *end = nullptr;
    long v = std::strtol(k.c_str(), &end, 0);
    bool numeric = end && *end == 0 && !k.empty();
    for (uint16_t id : stat_ids(driver)) {
        if ((numeric && id == v) || stat_name(id) == k) {
            out = id;
            return true;
        }
    }
    return false;
}

// ---------------------------------------------------------------------------
// Undo memory: the exact y values (and dynamic level) before the last edits.

struct Saved {
    uint32_t key;
    uint16_t info;
    size_t set;
    bool patched;
    std::vector<float> ys;
    float level;
};

std::vector<Saved> g_undo;

void remember(Saved s)
{
    if (g_undo.size() >= kUndoCap)
        g_undo.erase(g_undo.begin());
    g_undo.push_back(std::move(s));
}

// ---------------------------------------------------------------------------
// JSON helpers.

void stat_json(ce::Writer &w, const Comp &c, uint32_t key, uintptr_t driver, uint16_t info, const uint8_t ctx[16],
               float x, bool detail)
{
    uintptr_t st = find_stat(active_set(driver, ctx), info);
    w.begin_object();
    w.key("value");
    float v = stat_value(c, key, info, true, true);
    if (std::isfinite(v))
        w.integer(static_cast<int64_t>(v));
    else
        w.null();
    bool dyn = st && rdv<uint8_t>(st + kStatDynamic) != 0;
    uintptr_t cv = st ? used_curve(c, st) : 0;
    w.key("base");
    CurveView view;
    if (st && !dyn && curve_view(cv, view))
        w.num(std::round(curve_at(cv, view, x) * 10.0) / 10.0);
    else
        w.null();
    w.key("curve");
    w.str(!st ? "none" : dyn ? "dynamic" : cv == st + kStatPatched ? "patched" : "base");
    if (detail && st) {
        w.key("dynamicLevel");
        w.num(rdv<float>(st + kStatLevel));
        w.key("boost");
        w.num(rdv<float>(st + kStatBoost));
    }
    w.end_object();
}

void ratings_json(ce::Writer &w, const Comp &c, uint32_t key, uintptr_t driver, const std::vector<uint16_t> &which,
                  bool full)
{
    uint8_t ctx[16];
    context(c, ctx);
    float x = progress(c, key);
    w.key("ratings");
    w.begin_object();
    for (uint16_t info : which) {
        w.key(stat_name(info).c_str());
        stat_json(w, c, key, driver, info, ctx, x, full);
    }
    w.end_object();
}

// ---------------------------------------------------------------------------
// Contracts (My Team, read only). DriverContractsMyTeamComponent keeps four lists of 0x70-byte
// contract/move records, each list {allocator, begin, end, cap} as the record serialiser
// (Career_Serialise_DriverContractList) reads them:
//   +0x790 pendingDriverContracts   +0x7b0 decidedDriverContracts
//   +0x7d0 pendingDriverMoves       +0x7f0 lastSeasonsSignings
// Record: +0x00 performance clause name (loc string), +0x28 u8 clause stat (5 = none),
//   +0x2c i32 clause target, +0x34 i32 salary, +0x38 signingBonus, +0x3c teamPerfMoney,
//   +0x40 teamPerfRaceValue, +0x44 f32 successChance, +0x48 negotiationBonus, +0x4c u8 isCounteroffer,
//   +0x50 driver key, +0x54 old team key, +0x58 new team key, +0x5c u8 driverMoveReason,
//   +0x60 u32 weekendIndex, +0x64 u16 careerDayIndex, +0x68 u16 dayContractSent,
//   +0x6a u16 dayContractDecided, +0x6c u8 accepted, +0x6d u8 acknowledged.

constexpr uintptr_t kRecSize = sizeof(ContractRecord);
constexpr size_t kMaxRecords = 512;
constexpr uint8_t kNoClause = 5; // ContractRecord::clause_stat value for "no performance clause"

// A DriverStatId / team key as stored (u32): kind in the low byte, stat id in the high word.
uint32_t key_at(uintptr_t a)
{
    return rdv<uint32_t>(a) & 0xffff00ffu;
}

void record_json(ce::Writer &w, uintptr_t p)
{
    ContractRecord r{};
    rd(p, r);
    w.begin_object();
    w.key("driverStatId");
    w.integer(save_id(r.driver));
    w.key("oldTeamStatId");
    w.integer(r.old_team >> 16);
    w.key("newTeamStatId");
    w.integer(r.new_team >> 16);
    w.key("salary");
    w.integer(r.salary);
    w.key("signingBonus");
    w.integer(r.signing_bonus);
    w.key("teamPerfMoney");
    w.integer(r.team_perf_money);
    w.key("teamPerfRaceValue");
    w.integer(r.team_perf_race_value);
    w.key("successChance");
    w.num(r.success_chance);
    w.key("negotiationBonus");
    w.integer(r.negotiation_bonus);
    w.key("isCounteroffer");
    w.boolean(r.is_counteroffer != 0);
    w.key("reason");
    w.integer(r.driver_move_reason);
    w.key("weekendIndex");
    w.integer(r.weekend_index);
    w.key("careerDayIndex");
    w.integer(r.career_day_index);
    w.key("dayContractSent");
    w.integer(r.day_contract_sent);
    w.key("dayContractDecided");
    w.integer(r.day_contract_decided);
    w.key("accepted");
    w.boolean(r.accepted != 0);
    w.key("acknowledged");
    w.boolean(r.acknowledged != 0);
    w.key("clause");
    if (r.clause_stat != kNoClause) {
        w.begin_object();
        w.key("stat");
        w.integer(r.clause_stat);
        w.key("target");
        w.integer(r.clause_target);
        w.end_object();
    } else {
        w.null();
    }
    w.end_object();
}

void list_json(ce::Writer &w, const char *name, uintptr_t begin_field)
{
    w.key(name);
    w.begin_array();
    uintptr_t b;
    size_t n;
    if (vec(begin_field, kRecSize, kMaxRecords, b, n))
        for (size_t i = 0; i < n; ++i)
            record_json(w, b + i * kRecSize);
    w.end_array();
}

void contracts_json(ce::Writer &w)
{
    using C = DriverContractsMyTeamComponent;
    w.key("contracts");
    uintptr_t c = ce::comp::get(ce::comp::Id::DriverContractsMyTeam);
    if (!c || !swofty::mem::readable(c + offsetof(C, pending_contracts_begin),
                                  offsetof(C, last_season_signings_end) + 8 - offsetof(C, pending_contracts_begin))) {
        w.null();
        return;
    }
    w.begin_object();
    list_json(w, "pending", c + offsetof(C, pending_contracts_begin));
    list_json(w, "decided", c + offsetof(C, decided_contracts_begin));
    list_json(w, "pendingMoves", c + offsetof(C, pending_moves_begin));
    list_json(w, "lastSeasonSignings", c + offsetof(C, last_season_signings_begin));
    w.end_object();
}

// ---------------------------------------------------------------------------
// Line-up: the championship entity's entrant rows (entity+0x2e0..+0x2e8, 0x88 bytes each).
//   +0x00 vtable, +0x08..+0x4b identity (+0x48 bit 0x10000000 = human), +0x50 driver key,
//   +0x54 driver data, +0x58 team key, +0x5c..+0x6b vehicle keys (+0x5e == 0x3fff: no car),
//   +0x78 seat value, +0x80 u8 position, +0x81 has-position, +0x83 driver flag.
// Career_Set_EntrantData_Thunk rewrites one row (found by its identity) and posts the entity's
// entrant-change messages; Career_Record_DriverMove appends driverMoveData to the renewal
// component (+0x3a8 vector, 0x18 stride) and posts the driver-move message.

constexpr uint32_t kNoKey = 0x3fff0000u;   // stat id 0x3fff in the high word: no driver/team/vehicle
constexpr uint32_t kHumanBit = 0x10000000u; // EntrantRecord::ordinal_flags bit of the human entrant
constexpr size_t kMaxEntrants = 256;
constexpr uintptr_t kEntrantSize = sizeof(EntrantRecord);
constexpr uintptr_t kMoveSize = sizeof(DriverMoveRecord);
constexpr uint8_t kReasonSigned = 0;  // DriverMoveRecord::reason the game's signing writes for the incoming driver
constexpr uint8_t kReasonBenched = 3; // ... and for a driver benched in his own team
static_assert(sizeof(EntrantRecord) == 0x88);

// The driver key as the game stores and compares it: the whole dword from id_lo (id_lo, pad, id_hi).
uint32_t entrant_driver(const EntrantRecord &r)
{
    uint32_t v;
    std::memcpy(&v, &r.id_lo, sizeof v);
    return v;
}

bool has_car(const EntrantRecord &r)
{
    return (r.vehicle_ref & 0xffff0000u) != kNoKey;
}

bool has_car(uintptr_t row)
{
    EntrantRecord r{};
    return rd(row, r) && has_car(r);
}

void seat_json(ce::Writer &w, const EntrantRecord &r)
{
    w.begin_object();
    w.key("hasCar");
    w.boolean(has_car(r));
    w.key("position");
    if (r.team_position >> 8)
        w.integer(r.team_position & 0xff);
    else
        w.null();
    w.end_object();
}

void seat_json(ce::Writer &w, uintptr_t row)
{
    EntrantRecord r{};
    if (rd(row, r))
        seat_json(w, r);
    else
        w.null();
}

void moves_json(ce::Writer &w)
{
    w.key("recordedMoves");
    uintptr_t r = ce::comp::get(ce::comp::Id::CareerDriverContractRenewal);
    if (!r) {
        w.null();
        return;
    }
    w.begin_array();
    uintptr_t b;
    size_t n;
    if (r && vec(r + offsetof(CareerDriverContractRenewalComponent, driver_moves_begin), kMoveSize, 4096, b, n)) {
        for (size_t i = n > 20 ? n - 20 : 0; i < n; ++i) {
            DriverMoveRecord m{};
            if (!rd(b + i * kMoveSize, m))
                continue;
            w.begin_object();
            w.key("driverStatId");
            w.integer(save_id(m.driver));
            w.key("oldTeamStatId");
            w.integer(m.old_team >> 16);
            w.key("newTeamStatId");
            w.integer(m.new_team >> 16);
            w.key("reason");
            w.integer(m.reason);
            w.key("weekendIndex");
            w.integer(m.weekend_index);
            w.end_object();
        }
    }
    w.end_array();
}

// ---------------------------------------------------------------------------
// Snapshot section "drivers".

void section(ce::Writer &w)
{
    Comp c;
    const char *why = "no career loaded";
    bool ok = ce::in_career() && find_comp(c, &why);
    w.begin_object();
    w.key("available");
    w.boolean(ok);
    w.key("reason");
    w.str(ok ? "" : why);
    if (!ok) {
        w.end_object();
        return;
    }
    w.key("component");
    w.str(c.player_class ? "DriverCareerPlayerDriverStatisticsComponent" : "DriverStatisticsComponent");
    w.key("patch");
    w.begin_object();
    w.key("appliedChecksum");
    w.integer(rdv<uint32_t>(c.at + kCompApplied));
    w.key("declinedChecksum");
    w.integer(rdv<uint32_t>(c.at + kCompDeclined));
    w.key("usesPatchedCurves");
    w.boolean(patched_gate(c));
    w.end_object();

    w.key("stats");
    w.begin_array();
    for (auto &i : infos(c)) {
        w.begin_object();
        w.key("infoStatId");
        w.integer(i.id);
        w.key("name");
        w.str(stat_name(i.id));
        w.key("weight");
        w.num(i.weight);
        w.key("default");
        w.num(i.def);
        w.end_object();
    }
    w.end_array();

    std::unordered_map<uint32_t, ce::comp::Entrant> seat;
    for (auto &e : ce::comp::entrants())
        seat[e.driver] = e;
    const Names &nm = names();

    auto name_or_null = [&w](const std::unordered_map<uint32_t, std::string> &m, uint32_t key) {
        auto it = (key & 0xff) == 0 ? m.find(key >> 16) : m.end();
        if (it != m.end() && !it->second.empty() && it->second.size() < kNameMax)
            w.str(it->second);
        else
            w.null();
    };
    std::vector<InfoRow> stat_rows = infos(c);
    std::vector<uintptr_t> others;

    w.key("drivers");
    w.begin_array();
    uintptr_t b;
    size_t n;
    if (vec(c.at + kCompDrivers, kDriverSize, kMaxDrivers, b, n)) {
        for (size_t i = 0; i < n; ++i) {
            uintptr_t d = b + i * kDriverSize;
            uint32_t key = key_at(d + offsetof(DriverStatsEntry, kind));
            if ((key & 0xffff0000u) == kNoKey)
                continue;
            auto s = seat.find(key);
            if (s == seat.end()) {
                others.push_back(d);
                continue;
            }
            w.begin_object();
            w.key("driverStatId");
            w.integer(save_id(key));
            w.key("name");
            name_or_null(nm.driver, key);
            w.key("initials");
            name_or_null(nm.initials, key);
            auto num = (key & 0xff) == 0 ? nm.number.find(key >> 16) : nm.number.end();
            w.key("number");
            if (num != nm.number.end())
                w.integer(num->second);
            else
                w.null();
            uint32_t team = s->second.team;
            w.key("team");
            if ((team & 0xffff0000u) != kNoKey) {
                w.begin_object();
                w.key("teamStatId");
                w.integer(team >> 16);
                w.key("name");
                name_or_null(nm.team, team & 0xffff0000u);
                w.end_object();
            } else {
                w.null();
            }
            w.key("seat");
            seat_json(w, s->second.at);
            w.key("onGrid");
            w.boolean(has_car(s->second.at));
            w.key("player");
            w.boolean(s->second.human);
            w.key("overall");
            float o = overall(c, key);
            if (std::isfinite(o))
                w.integer(static_cast<int64_t>(o));
            else
                w.null();
            ratings_json(w, c, key, d, stat_ids(d), false);
            w.end_object();
        }
    }
    w.end_array();

    w.key("otherDrivers");
    w.begin_array();
    for (uintptr_t d : others) {
        uint32_t key = key_at(d + offsetof(DriverStatsEntry, kind));
        w.begin_object();
        w.key("driverStatId");
        w.integer(save_id(key));
        w.key("name");
        name_or_null(nm.driver, key);
        w.key("overall");
        float o = overall(c, key);
        if (std::isfinite(o))
            w.integer(static_cast<int64_t>(o));
        else
            w.null();
        w.key("values");
        w.begin_array();
        for (auto &info : stat_rows) {
            float v = find_stat(d + kDriverSetStats, info.id) ? stat_value(c, key, info.id, true, true) : NAN;
            if (std::isfinite(v))
                w.integer(static_cast<int64_t>(v));
            else
                w.null();
        }
        w.end_array();
        w.end_object();
    }
    w.end_array();
    w.key("progress");
    uint32_t first = n ? key_at(b + offsetof(DriverStatsEntry, kind)) : 0;
    w.num(n ? std::round(progress(c, first) * 10000.0) / 10000.0 : 0.0);
    contracts_json(w);
    moves_json(w);
    w.key("pendingSwaps");
    w.begin_array();
    w.end_array();
    w.end_object();
}

// ---------------------------------------------------------------------------
// Op drivers.ratings.set

struct Edit {
    uint16_t info;
    double target;
    bool has_expect;
    double expect;
};

ce::OpResult fail(const char *err, const std::string &detail = {})
{
    ce::OpResult r;
    r.ok = false;
    r.error = detail.empty() ? err : std::string(err) + ": " + detail;
    ce::log("drivers.ratings.set FAILED %s", r.error.c_str());
    return r;
}

std::string result_json(const Comp &c, uint32_t key, uintptr_t driver, const std::vector<uint16_t> &which)
{
    ce::Writer w;
    w.begin_object();
    w.key("driverStatId");
    w.integer(save_id(key));
    ratings_json(w, c, key, driver, which, true);
    w.key("overall");
    float o = overall(c, key);
    if (std::isfinite(o))
        w.integer(static_cast<int64_t>(o));
    else
        w.null();
    w.end_object();
    return w.text();
}

struct Neighbour {
    uint32_t key;
    uint16_t info;
    float value;
};

std::vector<Neighbour> neighbours(const Comp &c, uint32_t except, const std::vector<uint16_t> &which)
{
    std::vector<Neighbour> out;
    uintptr_t b;
    size_t n;
    if (!vec(c.at + kCompDrivers, kDriverSize, kMaxDrivers, b, n))
        return out;
    for (size_t i = 0; i < n; ++i) {
        uintptr_t d = b + i * kDriverSize;
        uint32_t key = key_at(d + offsetof(DriverStatsEntry, kind));
        if (key == except)
            continue;
        for (uint16_t info : which)
            out.push_back({key, info, stat_value(c, key, info, false, false)});
    }
    return out;
}

struct Undo {
    uintptr_t curve;
    uintptr_t stat;
    std::vector<float> ys;
    float level;
    bool curve_changed;
};

void roll_back(const std::vector<Undo> &log)
{
    for (auto it = log.rbegin(); it != log.rend(); ++it) {
        if (it->curve_changed) {
            CurveView v;
            if (curve_view(it->curve, v))
                curve_write_y(it->curve, v, it->ys);
        } else {
            wr(it->stat + kStatLevel, it->level);
        }
    }
}

bool apply_one(const Comp &c, uint32_t key, uintptr_t driver, const Edit &e, const std::string &mode,
               std::vector<Undo> &log, std::string &how)
{
    uint8_t ctx[16];
    context(c, ctx);
    float x = progress(c, key);
    uintptr_t active = active_set(driver, ctx);
    uintptr_t ast = find_stat(active, e.info);
    if (!ast)
        return false;
    float raw = stat_value(c, key, e.info, false, false);
    if (!std::isfinite(raw))
        return false;
    bool dyn = rdv<uint8_t>(ast + kStatDynamic) != 0;
    std::vector<uintptr_t> sets = sets_of(driver);

    if (dyn) {
        float delta = static_cast<float>(e.target) - raw;
        int n = 0;
        for (uintptr_t set : sets) {
            uintptr_t st = find_stat(set, e.info);
            if (!st || !rdv<uint8_t>(st + kStatDynamic))
                continue;
            float old = rdv<float>(st + kStatLevel);
            if (!wr(st + kStatLevel, old + delta))
                return false;
            log.push_back({0, st, {}, old, false});
            ++n;
        }
        how = "dynamicLevel " + std::to_string(delta) + " in " + std::to_string(n) + " set(s)";
        return n > 0;
    }

    uintptr_t acurve = used_curve(c, ast);
    CurveView av;
    if (!curve_view(acurve, av))
        return false;
    float cur_base = curve_at(acurve, av, x);
    float others = raw - cur_base;
    float target_base = static_cast<float>(e.target) - others;
    float target_norm = std::clamp((target_base - av.miny) / (av.maxy - av.miny), 0.0f, 1.0f);
    float shift_norm = target_norm - eval_curve(acurve, x);
    bool patched = acurve == ast + kStatPatched;

    int n = 0;
    for (size_t si = 0; si < sets.size(); ++si) {
        uintptr_t st = find_stat(sets[si], e.info);
        if (!st || rdv<uint8_t>(st + kStatDynamic))
            continue;
        uintptr_t cv = patched && rdv<uint8_t>(st + kStatHasPatched) ? st + kStatPatched : st + kStatCurve;
        CurveView v;
        std::vector<float> old;
        if (!curve_view(cv, v) || !curve_read_y(v, old))
            continue;
        std::vector<float> ys = old;
        bool restored = false;
        if (mode == "restore") {
            for (auto it = g_undo.rbegin(); it != g_undo.rend(); ++it) {
                if (it->key == key && it->info == e.info && it->set == si && it->patched == patched &&
                    it->ys.size() == ys.size()) {
                    ys = it->ys;
                    g_undo.erase(std::next(it).base());
                    restored = true;
                    break;
                }
            }
        }
        if (!restored) {
            for (float &y : ys)
                y = mode == "shift" ? std::clamp(y + shift_norm, 0.0f, 1.0f) : target_norm;
        }
        if (!curve_write_y(cv, v, ys)) {
            curve_write_y(cv, v, old);
            return false;
        }
        log.push_back({cv, st, old, 0.0f, true});
        remember({key, e.info, si, patched, old, 0.0f});
        ++n;
    }
    how = std::string(patched ? "patched" : "base") + " curve " + (mode.empty() ? "flat" : mode) + " in " +
          std::to_string(n) + " set(s)";
    return n > 0;
}

ce::OpResult op_ratings_set(const ce::Json &args)
{
    Comp c;
    const char *why = "";
    if (!find_comp(c, &why))
        return fail("no_component", why);
    const ce::Json &id = args["driverStatId"];
    if (id.type != ce::Json::Num || id.n < 0 || id.n > 0xffff || id.n != std::floor(id.n))
        return fail("bad_args", "driverStatId must be an integer 0..65535");
    uint32_t key = key_from_save(static_cast<uint32_t>(id.n));
    uintptr_t driver = find_driver(c, key);
    if (!driver)
        return fail("unknown_driver", std::to_string(static_cast<long long>(id.n)));

    std::string mode = args["mode"].str("flat");
    if (mode != "flat" && mode != "shift" && mode != "restore")
        return fail("bad_args", "mode must be flat, shift or restore");

    const ce::Json &rat = args["ratings"];
    if (rat.type != ce::Json::Obj || rat.o.empty())
        return fail("bad_args", "ratings must be a non-empty object");
    auto bound_min = slot<BoundFn>(c, kVtMin);
    auto bound_max = slot<BoundFn>(c, kVtMax);
    float lo = bound_min ? bound_min(reinterpret_cast<void *>(c.at)) : 0.0f;
    float hi = bound_max ? bound_max(reinterpret_cast<void *>(c.at)) : 100.0f;
    if (!std::isfinite(lo) || !std::isfinite(hi) || hi <= lo)
        lo = 0.0f, hi = 100.0f;

    std::vector<Edit> edits;
    std::vector<uint16_t> which;
    for (auto &[k, v] : rat.o) {
        uint16_t info;
        if (!resolve_stat(driver, k, info))
            return fail("unknown_stat", k);
        if (v.type != ce::Json::Num || !std::isfinite(v.n) || v.n < 0 || v.n > 100)
            return fail("bad_value", k + " must be a number 0..100");
        Edit e{info, std::clamp(v.n, static_cast<double>(lo), static_cast<double>(hi)), false, 0};
        const ce::Json &ex = args["expect"][k.c_str()];
        if (ex.type == ce::Json::Num) {
            e.has_expect = true;
            e.expect = ex.n;
        }
        edits.push_back(e);
        which.push_back(info);
    }
    for (auto &e : edits) {
        if (!e.has_expect)
            continue;
        float live = stat_value(c, key, e.info, true, true);
        if (!std::isfinite(live) || std::fabs(live - e.expect) > 0.5)
            return fail("stale", stat_name(e.info) + " is " + std::to_string(live) + ", expected " +
                                     std::to_string(e.expect));
    }

    ce::OpResult r;
    r.before = result_json(c, key, driver, which);
    std::vector<Neighbour> nb = neighbours(c, key, which);
    std::vector<float> before_vals;
    for (auto &e : edits)
        before_vals.push_back(stat_value(c, key, e.info, true, true));
    float before_overall = overall(c, key);

    std::vector<Undo> log;
    std::string how_all;
    for (auto &e : edits) {
        std::string how;
        if (!apply_one(c, key, driver, e, mode, log, how)) {
            roll_back(log);
            return fail("write_failed", stat_name(e.info) + " (curve empty or memory not writable)");
        }
        how_all += (how_all.empty() ? "" : "; ") + stat_name(e.info) + ": " + how;
    }

    for (auto &n : nb) {
        float now = stat_value(c, n.key, n.info, false, false);
        if (!(std::fabs(now - n.value) < 1e-3f || (std::isnan(now) && std::isnan(n.value)))) {
            roll_back(log);
            return fail("write_failed", "driver " + std::to_string(save_id(n.key)) + " " + stat_name(n.info) +
                                            " moved; edit rolled back");
        }
    }
    std::string detail;
    for (size_t i = 0; i < edits.size(); ++i) {
        float after = stat_value(c, key, edits[i].info, true, true);
        if (mode != "restore" && (!std::isfinite(after) || std::fabs(after - std::round(edits[i].target)) > 1.0f)) {
            roll_back(log);
            return fail("write_failed", stat_name(edits[i].info) + " reads " + std::to_string(after) +
                                            " after the write, wanted " + std::to_string(edits[i].target));
        }
        char buf[128];
        std::snprintf(buf, sizeof buf, "%s%s %.0f -> %.0f", detail.empty() ? "" : ", ", stat_name(edits[i].info).c_str(),
                      before_vals[i], after);
        detail += buf;
    }
    r.after = result_json(c, key, driver, which);
    r.ok = true;
    ce::log("drivers.ratings.set driver %u (key 0x%x, %s) %s; overall %.0f -> %.0f (%s, mode %s, "
            "%zu neighbour reads unchanged)",
            save_id(key), key, c.player_class ? "DriverCareerPlayerDriverStatistics" : "DriverStatistics",
            detail.c_str(), before_overall, overall(c, key), how_all.c_str(), mode.c_str(), nb.size());
    return r;
}

ce::OpResult op_fail(const char *op, const char *err, const std::string &detail)
{
    ce::OpResult r;
    r.ok = false;
    r.error = detail.empty() ? err : std::string(err) + ": " + detail;
    ce::log("%s FAILED %s", op, r.error.c_str());
    return r;
}

// ---------------------------------------------------------------------------
// Op drivers.swap

struct Row {
    uintptr_t at = 0;
    EntrantRecord rec{};
};

// The bytes of an EntrantRecord before its driver id (vtable, name block, ordinal_flags) are the
// identity Career_Set_EntrantData_Thunk matches rows by.
constexpr size_t kIdentitySize = offsetof(EntrantRecord, id_lo);

bool entrant_rows(uintptr_t entity, std::vector<Row> &out)
{
    uintptr_t b;
    size_t n;
    out.clear();
    if (!entity || !vec(entity + offsetof(CareerEntity, entrants_begin), kEntrantSize, kMaxEntrants, b, n) || !n)
        return false;
    out.resize(n);
    for (size_t i = 0; i < n; ++i) {
        out[i].at = b + i * kEntrantSize;
        if (!rd(out[i].at, out[i].rec))
            return false;
    }
    return true;
}

const Row *row_of(const std::vector<Row> &rows, uint32_t driver)
{
    for (auto &r : rows)
        if (entrant_driver(r.rec) == driver)
            return &r;
    return nullptr;
}

// The data block Career_Set_EntrantData_Thunk takes, laid out the way Career_Sign_DriverToTeam
// builds it: driver fields from the driver's row, team and seat fields from the seat's row.
EntrantWriteData entrant_data(const Row &driver_row, const Row &seat_row)
{
    EntrantWriteData d{};
    d.driver = entrant_driver(driver_row.rec);
    d.driver_customisation = driver_row.rec.driver_customisation;
    d.team_stat_id = seat_row.rec.team_stat_id;
    d.vehicle_ref = seat_row.rec.vehicle_ref;
    d.driver_reg_value = seat_row.rec.driver_reg_value;
    d.vehicle_customisation = seat_row.rec.vehicle_customisation;
    d.badge_stat_id = seat_row.rec.badge_stat_id;
    d.seat_value = seat_row.rec.const_ffff0012;
    return d;
}

bool same_identity(const EntrantRecord &x, const EntrantRecord &y)
{
    return std::memcmp(reinterpret_cast<const uint8_t *>(&x) + sizeof(void *),
                       reinterpret_cast<const uint8_t *>(&y) + sizeof(void *), kIdentitySize - sizeof(void *)) == 0;
}

void seat_text(char *buf, size_t n, const Row &r)
{
    std::snprintf(buf, n, "team %u car 0x%x pos %d", r.rec.team_stat_id >> 16, r.rec.vehicle_ref,
                  (r.rec.team_position >> 8) ? (r.rec.team_position & 0xff) : -1);
}

std::string swap_json(const std::vector<Row> &rows, uint32_t a, uint32_t b)
{
    ce::Writer w;
    w.begin_object();
    for (uint32_t k : {a, b}) {
        const Row *r = row_of(rows, k);
        w.key(k == a ? "a" : "b");
        w.begin_object();
        w.key("driverStatId");
        w.integer(save_id(k));
        if (r) {
            w.key("teamStatId");
            if ((r->rec.team_stat_id & 0xffff0000u) != kNoKey)
                w.integer(r->rec.team_stat_id >> 16);
            else
                w.null();
            w.key("seat");
            seat_json(w, r->rec);
        }
        w.end_object();
    }
    w.key("appliedNow");
    w.boolean(true);
    w.end_object();
    return w.text();
}

bool same_row(const Row &x)
{
    EntrantRecord live{};
    return rd(x.at, live) && std::memcmp(&live, &x.rec, kEntrantSize) == 0;
}

// Career_Set_EntrantData_Thunk posts its begin message before it searches, and only posts the end
// message (clearing CareerEntity::entrant_update_busy) when a row matched. A miss would leave the
// game mid entrant update, so the row must match first: the game's compare (0x1431146d0) requires
// the same human bit and, for AI rows, the same ordinal_flags. Exactly one row of the live table may
// match, and it must be the row being written.
bool unique_identity(uintptr_t entity, const Row &id_row)
{
    std::vector<Row> rows;
    if (!entrant_rows(entity, rows))
        return false;
    uint32_t id = id_row.rec.ordinal_flags;
    if (id & kHumanBit)
        return false;
    size_t hits = 0;
    uintptr_t at = 0;
    for (auto &r : rows) {
        uint32_t v = r.rec.ordinal_flags;
        if (!(v & kHumanBit) && v == id) {
            ++hits;
            at = r.at;
        }
    }
    return hits == 1 && at == id_row.at;
}

bool entity_busy(uintptr_t entity)
{
    return rdv<uint8_t>(entity + offsetof(CareerEntity, entrant_update_busy)) != 0;
}

// How the game announces a move. My Team: Career_Record_DriverMove on the renewal component (it
// appends driverMoveData and posts MessageDriverMoveConfirmed). Driver Career has no renewal
// component and no move log: its signing (0x140d766b0, DriverCareerContractsManagerComponent) posts
// MessageDriverMoveConfirmed itself, built from its weekend source and career clock, right before
// each Career_Set_EntrantData_Thunk. The Upgrades component listens to it to reassign per-driver
// upgrade state.
struct MoveSink {
    uintptr_t renewal = 0; // CareerDriverContractRenewalComponent (My Team)
    uintptr_t manager = 0; // DriverCareerContractsManagerComponent (Driver Career)
};

constexpr uintptr_t kVtWeekendIndex = 0x108; // vtable slot of the weekend source returning the weekend index
constexpr uint32_t kMinutesPerDay = 1440;

using WeekendFn = uint32_t (*)(void *self);

bool announce(const MoveSink &sink, uint32_t driver, uint32_t old_team, uint32_t new_team, uint8_t reason)
{
    if (sink.renewal) {
        Career_Record_DriverMove(reinterpret_cast<void *>(sink.renewal), driver, old_team, new_team, reason);
        return true;
    }
    using M = DriverCareerContractsManagerComponent;
    uintptr_t src = rdv<uintptr_t>(sink.manager + offsetof(M, weekend_source));
    uintptr_t time = rdv<uintptr_t>(sink.manager + offsetof(M, career_time));
    uintptr_t vt = src ? rdv<uintptr_t>(src) : 0;
    uintptr_t fn = vt ? rdv<uintptr_t>(vt + kVtWeekendIndex) : 0;
    if (!swofty::mem::in_image(fn) || !time || !vt_Career_MessageDriverMoveConfirmed.address())
        return false;
    DriverMoveConfirmedMessage m{};
    m.vtable = vt_Career_MessageDriverMoveConfirmed.get();
    m.move.driver = driver;
    m.move.old_team = old_team;
    m.move.new_team = new_team;
    m.move.reason = reason;
    m.move.weekend_index = reinterpret_cast<WeekendFn>(fn)(reinterpret_cast<void *>(src));
    m.move.day_index =
        static_cast<uint16_t>(rdv<uint32_t>(time + offsetof(CareerTimeComponent, current_minutes)) / kMinutesPerDay);
    return swofty::career::post(&m) == swofty::career::Result::ok;
}

bool apply_row(uintptr_t entity, const MoveSink &sink, const Row &id_row, uint32_t old_team, uint32_t new_team,
               uint8_t reason, const EntrantWriteData &data, uint16_t position)
{
    if (entity_busy(entity) || !unique_identity(entity, id_row))
        return false;
    uint8_t id[kIdentitySize];
    std::memcpy(id, &id_row.rec, kIdentitySize);
    if (!announce(sink, data.driver, old_team, new_team, reason))
        return false;
    Career_Set_EntrantData_Thunk(reinterpret_cast<void *>(entity), id, reinterpret_cast<const uint32_t *>(&data),
                                 position, id_row.rec.car_number);
    if (entity_busy(entity))
        ce::log("drivers.swap WARNING entity entrant-update flag still set after the write");
    return true;
}

bool teamless(const EntrantRecord &r)
{
    return (r.team_stat_id & 0xffff0000u) == kNoKey;
}

// Driver Career: release the outgoing driver's contract the way its signing does (state 3, the
// day, and the value the entity's component registry returns from vtable slot +0x108).
constexpr uintptr_t kVtRegistryValue = 0x108; // component registry vtable slot (not a field)
constexpr uint8_t kContractReleased = 3;     // Career_Set_DriverContractState state the replace path writes
constexpr uint8_t kReasonReleased = 1;       // MessageDriverMoveConfirmed reason for a driver taken off the grid
constexpr uint32_t kNoSeatValue = 0xffff0012u; // EntrantRecord::const_ffff0012 of a row without a seat
constexpr size_t kContractSlot = 16;          // DriverCareerContractOwner slots {?, DriverContract *}

using RegistryValueFn = uint32_t (*)(void *registry);
using WeekendFn2 = uintptr_t (*)(void *self);

bool release_contract(uintptr_t entity, const MoveSink &sink, uint32_t driver, bool &found)
{
    found = false;
    using M = DriverCareerContractsManagerComponent;
    uintptr_t owner = rdv<uintptr_t>(sink.manager + offsetof(M, contract_owner));
    uintptr_t time = rdv<uintptr_t>(sink.manager + offsetof(M, career_time));
    uintptr_t b;
    size_t n;
    if (!owner || !time || !vec(owner + offsetof(DriverCareerContractOwner, contracts_begin), kContractSlot, 4096, b, n))
        return false;
    uintptr_t contract = 0;
    for (size_t i = 0; i < n && !contract; ++i) {
        uintptr_t c = rdv<uintptr_t>(b + i * kContractSlot + sizeof(void *));
        if (c && rdv<uint32_t>(c + offsetof(DriverContract, driver)) == driver)
            contract = c;
    }
    if (!contract)
        return true;
    uintptr_t registry = entity + offsetof(CareerEntity, component_registry);
    uintptr_t vt = rdv<uintptr_t>(registry);
    uintptr_t fn = vt ? rdv<uintptr_t>(vt + kVtRegistryValue) : 0;
    if (!swofty::mem::in_image(fn))
        return false;
    uint32_t value = reinterpret_cast<RegistryValueFn>(fn)(reinterpret_cast<void *>(registry));
    uint16_t day = static_cast<uint16_t>(rdv<uint32_t>(time + offsetof(CareerTimeComponent, current_minutes)) /
                                         kMinutesPerDay);
    Career_Set_DriverContractState(reinterpret_cast<void *>(contract), day, kContractReleased, value);
    found = true;
    return true;
}

// The end of every Driver Career signing batch: the changed drivers between the current line-up and
// the weekend's line-up (Career_Build_LineupChanges), posted as MessageDriverMovesFinalised_upgrades,
// _recognition, then MessageDriverMovesFinalised. The _upgrades handler creates the team modifiers a
// newly seated driver needs (the null-modifier crash of a bare swap) and moves the per-driver state.
constexpr uintptr_t kVtCurrentWeekend = 0xa0; // weekend source vtable slot returning the current weekend
constexpr uintptr_t kWeekendEntrants = 0x18;  // TODO(struct): season lane Weekend: its start-of-weekend entrant vector object

bool finalise_moves(uintptr_t entity, const MoveSink &sink, uint64_t &changes)
{
    changes = 0;
    using M = DriverCareerContractsManagerComponent;
    uintptr_t src = rdv<uintptr_t>(sink.manager + offsetof(M, weekend_source));
    uintptr_t vt = src ? rdv<uintptr_t>(src) : 0;
    uintptr_t fn = vt ? rdv<uintptr_t>(vt + kVtCurrentWeekend) : 0;
    if (!swofty::mem::in_image(fn) || !Career_Build_LineupChanges.address())
        return false;
    uintptr_t weekend = reinterpret_cast<WeekendFn2>(fn)(reinterpret_cast<void *>(src));
    if (!weekend || !swofty::mem::readable(weekend + kWeekendEntrants, 3 * sizeof(void *)))
        return false;
    // The entrant vector object {allocator, begin, end} starts one pointer before entrants_begin.
    uintptr_t current = entity + offsetof(CareerEntity, entrants_begin) - sizeof(void *);
    DriverMovesFinalisedMessage m{};
    Career_Build_LineupChanges(&m.changes, reinterpret_cast<void *>(current),
                               reinterpret_cast<void *>(weekend + kWeekendEntrants));
    if (m.count > 40)
        return false;
    changes = m.count;
    void **vts[] = {vt_Career_MessageDriverMovesFinalisedUpgrades.get(),
                    vt_Career_MessageDriverMovesFinalisedRecognition.get(),
                    vt_Career_MessageDriverMovesFinalised.get()};
    for (void **v : vts) {
        DriverMovesFinalisedMessage copy = m;
        copy.vtable = v;
        copy.reserved = 0;
        if (swofty::career::post(&copy) != swofty::career::Result::ok)
            return false;
    }
    return true;
}

EntrantWriteData released_data(const Row &r)
{
    EntrantWriteData d{};
    d.driver = entrant_driver(r.rec);
    d.driver_customisation = r.rec.driver_customisation;
    d.team_stat_id = kNoKey;
    d.vehicle_ref = kNoKey;
    d.driver_reg_value = kNoKey;
    d.vehicle_customisation = kNoKey;
    d.badge_stat_id = kNoKey;
    d.seat_value = kNoSeatValue;
    return d;
}

bool row_matches(const EntrantRecord &now, const EntrantRecord &was, const EntrantWriteData &d)
{
    return now.team_stat_id == d.team_stat_id && now.vehicle_ref == d.vehicle_ref &&
           now.driver_reg_value == d.driver_reg_value && now.vehicle_customisation == d.vehicle_customisation &&
           now.badge_stat_id == d.badge_stat_id && entrant_driver(now) == d.driver && same_identity(now, was);
}

ce::OpResult op_swap(const ce::Json &args)
{
    const char *op = "drivers.swap";
    const ce::Json &ja = args["a"], &jb = args["b"];
    auto valid = [](const ce::Json &v) {
        return v.type == ce::Json::Num && v.n >= 0 && v.n <= 0xffff && v.n == std::floor(v.n);
    };
    if (!valid(ja) || !valid(jb))
        return op_fail(op, "bad_args", "a and b must be driverStatId integers 0..65535");
    uint32_t a = key_from_save(static_cast<uint32_t>(ja.n)), b = key_from_save(static_cast<uint32_t>(jb.n));
    if (a == b)
        return op_fail(op, "bad_args", "a and b are the same driver");

    uintptr_t entity = ce::comp::entity();
    MoveSink sink;
    sink.renewal = ce::comp::get(ce::comp::Id::CareerDriverContractRenewal);
    if (!sink.renewal)
        sink.manager = ce::comp::get(ce::comp::Id::DriverCareerContractsManager);
    if (!entity)
        return op_fail(op, "no_component", "no championship entity");
    if (!sink.renewal && !sink.manager)
        return op_fail(op, "no_component",
                       "neither CareerDriverContractRenewalComponent (My Team) nor DriverCareerContractsManagerComponent "
                       "(Driver Career) is on the championship entity");
    if (entity_busy(entity))
        return op_fail(op, "busy", "the game is in the middle of an entrant update");
    if (sink.renewal &&
        rdv<uint8_t>(sink.renewal + offsetof(CareerDriverContractRenewalComponent, disable_driver_moves)) != 0)
        return op_fail(op, "refused", "the game has driver moves disabled for this career (disableDriverMoves)");

    std::vector<Row> rows;
    if (!entrant_rows(entity, rows))
        return op_fail(op, "no_component", "entrant table unreadable");
    const Row *pa = row_of(rows, a), *pb = row_of(rows, b);
    if (!pa || !pb)
        return op_fail(op, "no_entrant",
                       std::string("driver ") + std::to_string(save_id(!pa ? a : b)) +
                           " is not in this championship's line-up (no entrant row, listed under otherDrivers), so "
                           "it has no seat to swap; pick a driver from the drivers list");
    Row ra = *pa, rb = *pb;
    if ((ra.rec.ordinal_flags | rb.rec.ordinal_flags) & kHumanBit)
        return op_fail(op, "player_locked", "the player's own seat cannot be swapped");
    uint32_t ta = ra.rec.team_stat_id, tb = rb.rec.team_stat_id;
    bool car_a = has_car(ra.rec), car_b = has_car(rb.rec);
    bool free_a = teamless(ra.rec), free_b = teamless(rb.rec);
    if (!car_a && !car_b)
        return op_fail(op, "no_seat", "neither driver has a race seat");
    if (ta == tb && car_a == car_b)
        return op_fail(op, "same_team", "both drivers race for the same team; nothing to swap");
    bool replace = false; // Driver Career: a teamless driver takes a race seat, the seat's driver is released
    if (sink.renewal) {
        if (free_a || free_b)
            return op_fail(op, "no_team",
                           std::string("driver ") + std::to_string(save_id(free_a ? a : b)) +
                               " has no team seat in this championship; in My Team swap two drivers who both "
                               "have seats");
    } else {
        if ((free_a && car_a) || (free_b && car_b) || (!free_a && !car_a) || (!free_b && !car_b))
            return op_fail(op, "reserve_dc",
                           std::string("driver ") + std::to_string(save_id((!car_a && !free_a) || (free_a && car_a) ? a : b)) +
                               " is a reserve (a team but no car); in Driver Career only race drivers and "
                               "free agents can be swapped, because R&D there is per race driver");
        replace = free_a || free_b;
    }
    const ce::Json &ex = args["expect"];
    if (ex.type == ce::Json::Obj) {
        const ce::Json &ea = ex["aTeamStatId"], &eb = ex["bTeamStatId"];
        if ((ea.type == ce::Json::Num && static_cast<uint32_t>(ea.n) != ta >> 16) ||
            (eb.type == ce::Json::Num && static_cast<uint32_t>(eb.n) != tb >> 16))
            return op_fail(op, "stale", "a driver's team changed since the launcher read it");
    }

    // In a Driver Career replace the incoming (teamless) driver goes first, then the seat's driver
    // is released, as the game's signing orders it.
    if (replace && free_b) {
        std::swap(ra, rb);
        std::swap(a, b);
        std::swap(ta, tb);
        std::swap(car_a, car_b);
    }
    ce::OpResult r;
    r.before = swap_json(rows, a, b);
    char sa[96], sb[96];
    seat_text(sa, sizeof sa, ra);
    seat_text(sb, sizeof sb, rb);

    EntrantWriteData da = entrant_data(ra, rb);
    EntrantWriteData db = replace ? released_data(rb) : entrant_data(rb, ra);
    uint16_t pos_a = ra.rec.team_position, pos_b = rb.rec.team_position;
    uint8_t reason_a = car_b ? kReasonSigned : kReasonBenched;
    uint8_t reason_b = replace ? kReasonReleased : car_a ? kReasonSigned : kReasonBenched;
    uint32_t new_b = replace ? kNoKey : ta;
    if (!unique_identity(entity, ra) || !unique_identity(entity, rb))
        return op_fail(op, "refused", "an entrant row's identity is not unique in the line-up; nothing written");
    bool wrote_a = apply_row(entity, sink, ra, ta, tb, reason_a, da, pos_b);
    bool contract_released = false;
    if (wrote_a && replace && !release_contract(entity, sink, entrant_driver(rb.rec), contract_released))
        ce::log("drivers.swap WARNING the outgoing driver's contract could not be released");
    bool wrote_b = wrote_a && apply_row(entity, sink, rb, tb, new_b, reason_b, db, replace ? 0 : pos_a);

    std::vector<Row> now;
    bool ok = wrote_a && wrote_b && entrant_rows(entity, now) && now.size() == rows.size();
    const Row *na = ok ? row_of(now, a) : nullptr, *nb = ok ? row_of(now, b) : nullptr;
    ok = ok && na && nb && na->at == ra.at && nb->at == rb.at;
    if (ok) {
        ok = row_matches(na->rec, ra.rec, da) && row_matches(nb->rec, rb.rec, db);
        for (size_t i = 0; ok && i < rows.size(); ++i)
            if (rows[i].at != ra.at && rows[i].at != rb.at && !same_row(rows[i]))
                ok = false;
    }
    uint64_t finalised = 0;
    bool finalise_ok = true;
    if (!ok) {
        EntrantWriteData ua = entrant_data(ra, ra), ub = entrant_data(rb, rb);
        bool back_a = !wrote_a || apply_row(entity, sink, ra, tb, ta, car_a ? kReasonSigned : kReasonBenched, ua, pos_a);
        bool back_b = !wrote_b || apply_row(entity, sink, rb, new_b, tb, car_b ? kReasonSigned : kReasonBenched, ub, pos_b);
        if (!sink.renewal)
            finalise_moves(entity, sink, finalised);
        return op_fail(op, "write_failed",
                       std::string("the entrant rows did not read back as swapped; ") +
                           (back_a && back_b ? "restored both rows" : "COULD NOT restore every row, reload the save") +
                           (contract_released ? " (the released contract stays released)" : ""));
    }
    if (!sink.renewal)
        finalise_ok = finalise_moves(entity, sink, finalised);
    if (!finalise_ok)
        ce::log("drivers.swap WARNING the MessageDriverMovesFinalised messages could not be posted");
    r.after = swap_json(now, a, b);
    r.ok = true;
    char na_t[96], nb_t[96];
    seat_text(na_t, sizeof na_t, *na);
    seat_text(nb_t, sizeof nb_t, *nb);
    ce::log("drivers.swap driver %u [%s] <-> driver %u [%s] -> driver %u [%s], driver %u [%s] "
            "(%s; %zu other entrants unchanged, applied now)",
            save_id(a), sa, save_id(b), sb, save_id(a), na_t, save_id(b), nb_t,
            sink.renewal ? "2 moves recorded"
            : replace    ? (contract_released ? "Driver Career replace: incoming announced, outgoing contract released "
                                                "and announced to no team, moves finalised"
                                              : "Driver Career replace: incoming announced, outgoing announced to no "
                                                "team (no contract found), moves finalised")
                         : "Driver Career: 2 moves announced, moves finalised",
            rows.size() - 2);
    if (!sink.renewal)
        ce::log("drivers.swap finalised %llu line-up change(s) (MessageDriverMovesFinalised_upgrades, _recognition, "
                "MessageDriverMovesFinalised)%s",
                static_cast<unsigned long long>(finalised), finalise_ok ? "" : " FAILED");
    return r;
}

// ---------------------------------------------------------------------------
// Op drivers.contract.set (unsafe: raw field write the record serialiser reads straight back)

struct MoneyField {
    const char *name;
    uintptr_t off;
};

constexpr MoneyField kMoney[] = {
    {"salary", offsetof(ContractRecord, salary)},
    {"signingBonus", offsetof(ContractRecord, signing_bonus)},
    {"teamPerfMoney", offsetof(ContractRecord, team_perf_money)},
    {"teamPerfRaceValue", offsetof(ContractRecord, team_perf_race_value)}};

ce::OpResult op_contract_set(const ce::Json &args)
{
    const char *op = "drivers.contract.set";
    uintptr_t c = ce::comp::get(ce::comp::Id::DriverContractsMyTeam);
    if (!c)
        return op_fail(op, "wrong_mode", "contracts are My Team only (no DriverContractsMyTeamComponent)");
    const ce::Json &id = args["driverStatId"];
    if (id.type != ce::Json::Num || id.n < 0 || id.n > 0xffff || id.n != std::floor(id.n))
        return op_fail(op, "bad_args", "driverStatId must be an integer 0..65535");
    std::string list = args["list"].str("decided");
    using C = DriverContractsMyTeamComponent;
    uintptr_t off = list == "decided" ? offsetof(C, decided_contracts_begin)
                    : list == "pending" ? offsetof(C, pending_contracts_begin)
                                        : 0;
    if (!off)
        return op_fail(op, "bad_args", "list must be decided or pending");
    uint32_t key = key_from_save(static_cast<uint32_t>(id.n));
    const ce::Json &team = args["newTeamStatId"];
    uintptr_t b;
    size_t n;
    if (!vec(c + off, kRecSize, kMaxRecords, b, n))
        return op_fail(op, "no_contract", "the " + list + " list is empty");
    uintptr_t rec = 0;
    int matches = 0;
    for (size_t i = 0; i < n; ++i) {
        uintptr_t p = b + i * kRecSize;
        if (key_at(p + offsetof(ContractRecord, driver)) != key)
            continue;
        if (team.type == ce::Json::Num &&
            (key_at(p + offsetof(ContractRecord, new_team)) >> 16) != static_cast<uint32_t>(team.n))
            continue;
        rec = p;
        ++matches;
    }
    if (!matches)
        return op_fail(op, "no_contract", "no " + list + " contract for driver " + std::to_string(static_cast<long long>(id.n)));
    if (matches > 1)
        return op_fail(op, "ambiguous", "several " + list + " contracts for this driver; pass newTeamStatId");

    int32_t want[4];
    bool set[4] = {};
    int count = 0;
    for (int i = 0; i < 4; ++i) {
        want[i] = rdv<int32_t>(rec + kMoney[i].off);
        const ce::Json &v = args[kMoney[i].name];
        if (v.type == ce::Json::Null)
            continue;
        if (v.type != ce::Json::Num || v.n < 0 || v.n > 1000000000.0 || v.n != std::floor(v.n))
            return op_fail(op, "bad_value", std::string(kMoney[i].name) + " must be an integer 0..1000000000");
        const ce::Json &e = args["expect"][kMoney[i].name];
        if (e.type == ce::Json::Num && static_cast<int64_t>(e.n) != want[i])
            return op_fail(op, "stale", std::string(kMoney[i].name) + " is " + std::to_string(want[i]));
        want[i] = static_cast<int32_t>(v.n);
        set[i] = true;
        ++count;
    }
    if (!count)
        return op_fail(op, "bad_args", "nothing to set (salary, signingBonus, teamPerfMoney, teamPerfRaceValue)");

    ce::Writer wb;
    record_json(wb, rec);
    ce::OpResult r;
    r.before = wb.text();
    std::string detail;
    for (int i = 0; i < 4; ++i) {
        if (!set[i])
            continue;
        int32_t old = rdv<int32_t>(rec + kMoney[i].off);
        if (!wr(rec + kMoney[i].off, want[i]) || rdv<int32_t>(rec + kMoney[i].off) != want[i])
            return op_fail(op, "write_failed", kMoney[i].name);
        detail += (detail.empty() ? "" : ", ") + std::string(kMoney[i].name) + " " + std::to_string(old) + " -> " +
                  std::to_string(want[i]);
    }
    ce::Writer wa;
    record_json(wa, rec);
    r.after = wa.text();
    r.ok = true;
    ce::log("drivers.contract.set driver %u %s contract (team %u): %s (raw record write, unsafe)",
            save_id(key), list.c_str(), key_at(rec + offsetof(ContractRecord, new_team)) >> 16, detail.c_str());
    return r;
}

} // namespace

CE_SECTION("drivers", section);
CE_OP("drivers.ratings.set", op_ratings_set);
CE_OP("drivers.contract.set", op_contract_set);
CE_OP("drivers.swap", op_swap);
