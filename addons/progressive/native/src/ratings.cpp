// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: each season's real driver ratings (ratings.hpp, ratings_logic.hpp).

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
#include <vector>

#include <swofty/career.hpp>
#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>
#include <swofty/storage.hpp>

#include "names.hpp"
#include "ratings.hpp"
#include "ratings_logic.hpp"

SWOFTY_SETTING(bool, season_ratings, true);

namespace pg::ratings {

namespace {

using namespace swofty::game;
using namespace progressive::ratings;

constexpr uintptr_t kCompData = offsetof(DriverStatisticsComponent, stats_data);
constexpr uintptr_t kCompCtx = offsetof(DriverStatisticsComponent, stat_context);
constexpr uintptr_t kCompTime = offsetof(DriverStatisticsComponent, career_time);
constexpr uintptr_t kCompGate = offsetof(DriverStatisticsComponent, use_story_mode_data);
constexpr uintptr_t kCompApplied = offsetof(DriverStatisticsComponent, last_patch_applied_checksum);
constexpr uintptr_t kCompDrivers = offsetof(DriverStatisticsComponent, driver_data_begin);
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
constexpr uintptr_t kStatDynamic = offsetof(DriverStatistic, is_dynamic_stat);
constexpr uintptr_t kStatLevel = offsetof(DriverStatistic, dynamic_level);
constexpr uintptr_t kCurveEnd = offsetof(EvolutionCurve, end_value);
constexpr uintptr_t kCurveMinY = offsetof(EvolutionCurve, min_y);
constexpr uintptr_t kCurveMaxY = offsetof(EvolutionCurve, max_y);
constexpr uintptr_t kCurveLut = offsetof(EvolutionCurve, lut);
constexpr uintptr_t kCurvePoints = offsetof(EvolutionCurve, points_begin);
constexpr size_t kLutSize = sizeof(EvolutionCurve::lut);
constexpr size_t kPointSize = sizeof(CurvePoint);
constexpr size_t kPointY = offsetof(CurvePoint, y);

constexpr uintptr_t kVtFixedProgress = 0x278;
constexpr uintptr_t kVtGetStat = 0x280;
constexpr uintptr_t kCtxVtFill = 0xc8;

constexpr size_t kMaxDrivers = 1024;
constexpr size_t kMaxStats = 64;
constexpr size_t kMaxSets = 32;
constexpr size_t kMaxPoints = 256;

using GetStatFn = float (*)(void *self, void *data, const uint32_t *driver, const uint16_t *info, const uint8_t *ctx,
                            float x, bool patched, bool clamp, bool round);
using FixedFn = bool (*)(void *self, const uint32_t *driver);
using CtxFn = void (*)(void *self, uint8_t *out);

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

bool vec(uintptr_t at, size_t stride, size_t cap, uintptr_t &b, size_t &n)
{
    uintptr_t e = 0;
    if (!swofty::mem::read(at, b) || !swofty::mem::read(at + 8, e))
        return false;
    if (!b || e < b || (e - b) % stride || (e - b) / stride > cap)
        return false;
    n = (e - b) / stride;
    return n == 0 || swofty::mem::readable(b, e - b);
}

struct Comp {
    uintptr_t at = 0, data = 0, vt = 0;
    bool player_class = false;
};

bool find_comp(Comp &c, const char **why)
{
    c = {};
    void *p = swofty::career::component_by_desc(k_Career_DriverCareerPlayerDriverStatisticsComponentDesc.get());
    c.player_class = p != nullptr;
    if (!p)
        p = swofty::career::component_by_desc(k_Career_DriverStatisticsComponentDesc.get());
    if (!p) {
        *why = "no DriverStatisticsComponent on the championship entity";
        return false;
    }
    uintptr_t at = reinterpret_cast<uintptr_t>(p), vt = 0;
    if (!swofty::mem::read(at, vt) || !swofty::mem::readable(vt, kVtGetStat + sizeof(void *))) {
        *why = "component vtable unreadable";
        return false;
    }
    uintptr_t b;
    size_t n;
    if (!vec(at + kCompDrivers, kDriverSize, kMaxDrivers, b, n)) {
        *why = "driver table unreadable";
        return false;
    }
    if (!Career_Eval_Curve.address() || !Career_Get_DriverOverallRating.address() ||
        !Career_Get_CareerLengthDays_Thunk.address()) {
        *why = "Career_Eval_Curve, Career_Get_DriverOverallRating or Career_Get_CareerLengthDays_Thunk unresolved";
        return false;
    }
    c.at = at;
    c.data = at + kCompData;
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

std::vector<uint32_t> all_keys(const Comp &c)
{
    std::vector<uint32_t> out;
    uintptr_t b;
    size_t n;
    if (!vec(c.at + kCompDrivers, kDriverSize, kMaxDrivers, b, n))
        return out;
    for (size_t i = 0; i < n; ++i) {
        uintptr_t e = b + i * kDriverSize;
        out.push_back(static_cast<uint32_t>(rdv<uint8_t>(e + offsetof(DriverStatsEntry, kind))) |
                      (static_cast<uint32_t>(rdv<uint16_t>(e + offsetof(DriverStatsEntry, stat_id))) << 16));
    }
    return out;
}

void context(const Comp &c, uint8_t ctx[16])
{
    std::memset(ctx, 0, 16);
    uintptr_t obj = rdv<uintptr_t>(c.at + kCompCtx);
    uintptr_t vt = obj ? rdv<uintptr_t>(obj) : 0;
    uintptr_t fn = vt ? rdv<uintptr_t>(vt + kCtxVtFill) : 0;
    if (swofty::mem::in_image(fn))
        reinterpret_cast<CtxFn>(fn)(reinterpret_cast<void *>(obj), ctx);
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

float shown(const Comp &c, uint32_t key, uint16_t info, bool clamp_round)
{
    auto get = slot<GetStatFn>(c, kVtGetStat);
    if (!get)
        return NAN;
    uint8_t ctx[16];
    context(c, ctx);
    return get(reinterpret_cast<void *>(c.at), reinterpret_cast<void *>(c.data), &key, &info, ctx, progress(c, key),
               patched_gate(c), clamp_round, clamp_round);
}

float overall_now(const Comp &c, uint32_t key)
{
    return Career_Get_DriverOverallRating(reinterpret_cast<void *>(c.at), reinterpret_cast<void *>(c.data), &key, true);
}

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
        if (!swofty::mem::read(v.pts + i * kPointSize + kPointY, ys[i]))
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

uintptr_t used_curve(const Comp &c, uintptr_t stat)
{
    if (patched_gate(c) && rdv<uint8_t>(stat + kStatHasPatched))
        return stat + kStatPatched;
    return stat + kStatCurve;
}

bool base_now(const Comp &c, uint32_t key, uintptr_t driver, uint16_t info, float x, double &out)
{
    uint8_t ctx[16];
    context(c, ctx);
    uintptr_t st = find_stat(active_set(driver, ctx), info);
    if (!st)
        return false;
    if (rdv<uint8_t>(st + kStatDynamic)) {
        float v = shown(c, key, info, false);
        out = v;
        return std::isfinite(v);
    }
    CurveView v;
    uintptr_t cv = used_curve(c, st);
    if (!curve_view(cv, v))
        return false;
    out = curve_at(cv, v, x);
    return std::isfinite(out);
}

struct Undo {
    uintptr_t curve = 0, stat = 0;
    std::vector<float> ys;
    float level = 0;
    bool curve_changed = false;
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

int write_stat(const Comp &c, uint32_t key, uintptr_t driver, uint16_t info, double target, float x,
               std::vector<Undo> &log)
{
    int n = 0;
    for (uintptr_t set : sets_of(driver)) {
        uintptr_t st = find_stat(set, info);
        if (!st)
            continue;
        if (rdv<uint8_t>(st + kStatDynamic)) {
            float raw = shown(c, key, info, false);
            float old = rdv<float>(st + kStatLevel);
            if (!std::isfinite(raw) || !wr(st + kStatLevel, old + static_cast<float>(target - raw)))
                return -1;
            log.push_back({0, st, {}, old, false});
            ++n;
            continue;
        }
        std::vector<uintptr_t> curves{st + kStatCurve};
        if (rdv<uint8_t>(st + kStatHasPatched))
            curves.push_back(st + kStatPatched);
        for (uintptr_t cv : curves) {
            CurveView v;
            std::vector<float> old;
            if (!curve_view(cv, v) || !curve_read_y(v, old))
                continue;
            float want = std::clamp(static_cast<float>((target - v.miny) / (v.maxy - v.miny)), 0.0f, 1.0f);
            float shift = want - eval_curve(cv, x);
            std::vector<float> ys = old;
            for (float &y : ys)
                y = std::clamp(y + shift, 0.0f, 1.0f);
            if (!curve_write_y(cv, v, ys)) {
                curve_write_y(cv, v, old);
                return -1;
            }
            log.push_back({cv, st, old, 0.0f, true});
            ++n;
        }
    }
    return n;
}

Data g_ratings;
int g_state = 0;
std::string g_said;

bool data()
{
    if (g_state)
        return g_state > 0;
    std::string text, err;
    if (swofty::storage::read("data/years.json", text) != swofty::storage::Result::ok) {
        swofty::log("[progressive] ratings: data/years.json is missing; drivers keep the game's ratings");
        g_state = -1;
        return false;
    }
    bool ok = parse(text, g_ratings, err);
    g_state = ok ? 1 : -1;
    size_t n = 0;
    for (auto &[y, ds] : g_ratings.years)
        n += ds.size();
    swofty::log("[progressive] ratings: years.json ratings %s (%zu years, %zu driver seasons)%s%s",
                ok ? "loaded" : "REFUSED", g_ratings.years.size(), n, err.empty() ? "" : ": ", err.c_str());
    return ok;
}

std::string values_text(const std::array<double, kStats> &v)
{
    std::string out;
    for (int i = 0; i < kStats; ++i) {
        char b[24];
        std::snprintf(b, sizeof b, "%s%s %.0f", i ? " " : "", kShort[i], v[i]);
        out += b;
    }
    return out;
}

void say_once(const std::string &key, const char *fmt, const std::string &a, const std::string &b)
{
    if (g_said == key)
        return;
    g_said = key;
    swofty::log(fmt, a.c_str(), b.c_str());
}

} // namespace

void season(uintptr_t, store::Career *c, uint32_t index, uint32_t player, const std::vector<uint32_t> &movers)
{
    if (!c || !data())
        return;
    Input in;
    in.start = c->start;
    in.index = index;
    in.player = player;
    in.created_season = c->created_season;
    in.player_ratings = c->player_ratings;
    in.ratings_season = c->ratings_season;
    in.enabled = season_ratings;
    in.movers = movers;
    auto stat_of = [](const std::string &label) {
        uint32_t s = 0;
        return names::stat(names::Kind::driver, label, s) ? static_cast<int>(s) : -1;
    };
    Plan p = plan(g_ratings, in, stat_of);
    std::string notes;
    for (auto &n : p.notes)
        notes += "; " + n;
    std::string key = c->key + "|" + std::to_string(index) + "|" + std::to_string(c->player_ratings) + "|" +
                      std::to_string(c->ratings_season) + "|" + notes + p.player_note;
    if (p.targets.empty()) {
        say_once(key, "[progressive] ratings: nothing to write%s; %s", notes, p.player_note);
        if (p.first && in.enabled) {
            c->ratings_season = static_cast<int>(index);
            store::save();
        }
        return;
    }
    Comp comp;
    const char *why = "";
    if (!find_comp(comp, &why)) {
        say_once(key, "[progressive] ratings: NOT written: %s%s", why, notes);
        return;
    }
    struct Pending {
        const Target *t;
        uintptr_t driver;
        uint32_t key;
        std::array<double, kStats> before{};
        std::array<bool, kStats> have{};
        float overall_before = NAN;
    };
    std::vector<Pending> todo;
    int kept = 0, missing = 0;
    bool player_kept = false;
    std::string missing_text;
    for (auto &t : p.targets) {
        uint32_t k = t.stat << 16;
        uintptr_t d = find_driver(comp, k);
        if (!d) {
            ++missing;
            missing_text += (missing_text.empty() ? "" : ", ") + t.driver;
            continue;
        }
        Pending q{&t, d, k};
        float x = progress(comp, k);
        for (int i = 0; i < kStats; ++i)
            q.have[i] = base_now(comp, k, d, static_cast<uint16_t>(g_ratings.stats[i].stat_id), x, q.before[i]);
        if (!needs(t, q.before, q.have)) {
            ++kept;
            if (t.player) {
                player_kept = true;
                swofty::log("[progressive] ratings %d: the player's driver %s(%u) already has his %d ratings (%s); "
                            "never written again",
                            p.year, t.driver.c_str(), t.stat, p.year, values_text(q.before).c_str());
            }
            continue;
        }
        q.overall_before = overall_now(comp, k);
        todo.push_back(q);
    }
    if (todo.empty()) {
        if (p.first || !missing_text.empty())
            say_once(key, "[progressive] ratings %s: every driver already has his ratings%s", std::to_string(p.year),
                     missing ? " (not in the career's statistics table: " + missing_text + ")" : std::string());
        if (player_kept)
            c->player_ratings = p.year;
        if (p.first)
            c->ratings_season = static_cast<int>(index);
        if (p.first || player_kept)
            store::save();
        return;
    }
    std::vector<uint32_t> others;
    for (uint32_t k : all_keys(comp)) {
        bool target = false;
        for (auto &q : todo)
            target |= q.key == k;
        if (!target)
            others.push_back(k);
    }
    std::vector<float> before_others;
    for (uint32_t k : others)
        for (int i = 0; i < kStats; ++i)
            before_others.push_back(shown(comp, k, static_cast<uint16_t>(g_ratings.stats[i].stat_id), false));
    std::vector<Undo> log;
    std::vector<int> sets(todo.size(), 0);
    for (size_t j = 0; j < todo.size(); ++j) {
        auto &q = todo[j];
        float x = progress(comp, q.key);
        for (int i = 0; i < kStats; ++i) {
            if (!q.have[i])
                continue;
            int n = write_stat(comp, q.key, q.driver, static_cast<uint16_t>(g_ratings.stats[i].stat_id),
                               q.t->values.v[i], x, log);
            if (n < 0) {
                roll_back(log);
                swofty::log("[progressive] ratings %d: WRITE FAILED for %s (%s): curve empty or memory not writable; "
                            "every write of this batch rolled back",
                            p.year, q.t->driver.c_str(), kNames[i]);
                return;
            }
            sets[j] = std::max(sets[j], n);
        }
    }
    size_t at = 0;
    for (uint32_t k : others)
        for (int i = 0; i < kStats; ++i) {
            float now = shown(comp, k, static_cast<uint16_t>(g_ratings.stats[i].stat_id), false);
            float was = before_others[at++];
            if (!(std::fabs(now - was) < 1e-3f || (std::isnan(now) && std::isnan(was)))) {
                roll_back(log);
                swofty::log("[progressive] ratings %d: WARNING driver key 0x%x %s moved %.2f -> %.2f although he was "
                            "not written; the batch is rolled back",
                            p.year, k, kNames[i], was, now);
                return;
            }
        }
    int wrote = 0, ai = 0;
    bool player_done = false;
    for (size_t j = 0; j < todo.size(); ++j) {
        auto &q = todo[j];
        float x = progress(comp, q.key);
        std::array<double, kStats> after{};
        std::array<double, kStats> show{};
        for (int i = 0; i < kStats; ++i) {
            uint16_t info = static_cast<uint16_t>(g_ratings.stats[i].stat_id);
            after[i] = q.before[i];
            base_now(comp, q.key, q.driver, info, x, after[i]);
            show[i] = shown(comp, q.key, info, true);
        }
        ++wrote;
        if (q.t->player)
            player_done = true;
        else
            ++ai;
        std::string who = q.t->player ? "the player's driver " : q.t->mover ? "mid-season, coming in: " : "";
        swofty::log("[progressive] ratings %d: %s%s(%u) base %s -> %s (shown %s), overall %.0f -> %.0f (data %.1f; %s; "
                    "%d set(s), curve position %.3f)",
                    p.year, who.c_str(), q.t->driver.c_str(), q.t->stat, values_text(q.before).c_str(),
                    values_text(after).c_str(), values_text(show).c_str(), q.overall_before, overall_now(comp, q.key),
                    q.t->values.overall, q.t->values.source.c_str(), sets[j], x);
    }
    swofty::log("[progressive] ratings %d: %d driver(s) written (%d AI%s), %d already at their %d ratings, %zu other "
                "driver(s) unchanged (%s)%s%s; %s",
                p.year, wrote, ai, player_done ? " + the player" : "", kept, p.year, others.size(),
                comp.player_class ? "DriverCareerPlayerDriverStatistics" : "DriverStatistics",
                missing ? ", not in the career's statistics table: " : "", missing_text.c_str(), p.player_note.c_str());
    g_said = key;
    if (player_done || player_kept)
        c->player_ratings = p.year;
    if (p.first)
        c->ratings_season = static_cast<int>(index);
    store::save();
}

} // namespace pg::ratings
