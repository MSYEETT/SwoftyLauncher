// Host unit test for the core's per-car AI racecraft fields (ai_racecraft.hpp and car_live.hpp, ai.blocking.*) on a
// fake AI system laid out as the KB's: the record vector at S+0xa8..0xb0, stride 0x508, the vehicle index at R+0 and
// the driver config slots at R+0x238+off, each slot a pointer to a cell holding a pointer to the record whose float
// is at +8. run_car_live_test.sh builds and runs it.
#include "../../src/core/hooks/car_live.hpp"

#include <cstdio>
#include <cstring>
#include <set>
#include <string>
#include <utility>
#include <vector>

namespace cl = swofty_core::hooks::car_live;
namespace rc = swofty_core::hooks::ai_racecraft;

static int fails;
static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

struct Tag {};
using Ops = cl::Ops<Tag>;

constexpr int CARS = 3;
constexpr uintptr_t SLOT0 = rc::R_DRIVER + 0x90, SLOT2 = rc::R_DRIVER + 0xa0;

alignas(16) static uint8_t car_mem[CARS][0x600];
static uintptr_t table[32];
static uint32_t count;
static bool flow = true;
static int writes;
static std::set<uintptr_t> dead;
static std::vector<std::pair<uintptr_t, uintptr_t>> written;
static std::set<std::string> rows_off;

struct Db {
    alignas(16) uint8_t record[0x40];
    alignas(16) uint64_t cell;
};

struct World {
    alignas(16) uint8_t system[0x1a8];
    std::vector<uint8_t> records;
    Db db[CARS][3];
    uint64_t shared_cell = 0;
    alignas(16) uint8_t shared_rec[0x40];
};

static uint8_t *rec_at(World &w, int car) { return w.records.data() + size_t(car) * rc::STRIDE; }

static void fill_system(World &w, int n)
{
    uint64_t begin = reinterpret_cast<uintptr_t>(w.records.data());
    uint64_t end = begin + uint64_t(n) * rc::STRIDE;
    memcpy(w.system + rc::RECORDS_BEGIN, &begin, 8);
    memcpy(w.system + rc::RECORDS_END, &end, 8);
}

static void build(World &w, bool shared = false)
{
    w.records.assign(size_t(CARS) * rc::STRIDE, 0);
    float stock[3] = {0.875f, 0.975f, 30.0f};
    for (int c = 0; c < CARS; c++) {
        rec_at(w, c)[rc::R_VEHICLE_INDEX] = uint8_t(c);
        for (int s = 0; s < 3; s++) {
            Db &d = w.db[c][s];
            memset(&d, 0, sizeof d);
            memcpy(d.record + 8, &stock[s], 4);
            d.record[0x10] = uint8_t(0xa0 + s);
            d.cell = reinterpret_cast<uintptr_t>(d.record);
            uint64_t p = reinterpret_cast<uintptr_t>(&d.cell);
            memcpy(rec_at(w, c) + rc::R_DRIVER + 0x90 + 8 * s, &p, 8);
        }
    }
    (void)shared;
    fill_system(w, CARS);
}

static uint64_t slot_of(World &w, int car, uintptr_t off)
{
    uint64_t p;
    memcpy(&p, rec_at(w, car) + off, 8);
    return p;
}

static float db_float(World &w, int car, int s)
{
    float v;
    memcpy(&v, w.db[car][s].record + 8, 4);
    return v;
}

static cl::Env make_env()
{
    cl::Env e;
    e.read = [](uintptr_t a, void *out, size_t n) {
        for (uintptr_t d : dead)
            if (a >= d && a < d + 0x10000)
                return false;
        memcpy(out, reinterpret_cast<const void *>(a), n);
        return true;
    };
    e.write = [](uintptr_t a, const void *in, size_t n) {
        for (uintptr_t d : dead)
            if (a >= d && a < d + 0x10000)
                return false;
        writes++;
        uintptr_t v = 0;
        memcpy(&v, in, n < 8 ? n : 8);
        written.emplace_back(a, v);
        memcpy(reinterpret_cast<void *>(a), in, n);
        return true;
    };
    e.flow_thread = [] { return flow; };
    e.enabled = [] { return true; };
    e.row_ready = [](const char *row) { return !rows_off.count(row); };
    e.table = reinterpret_cast<uintptr_t>(table);
    e.count = reinterpret_cast<uintptr_t>(&count);
    return e;
}

static void build_cars()
{
    memset(car_mem, 0, sizeof car_mem);
    for (int i = 0; i < CARS; i++) {
        car_mem[i][cl::CAR_VEHICLE_ID] = uint8_t(i);
        table[i] = reinterpret_cast<uintptr_t>(car_mem[i]);
    }
    count = CARS;
}

static void settle()
{
    for (uint32_t i = 0; i < rc::SETTLE_FRAMES + 1; i++)
        Ops::racecraft_tick();
    Ops::flush();
}

static float get(uint32_t car, uint32_t id, int *r = nullptr)
{
    float v = -1;
    int x = Ops::op_get(car, nullptr, id, &v);
    if (r)
        *r = x;
    return v;
}

int main()
{
    build_cars();
    World w;
    build(w);
    Ops::bind(make_env());
    uint32_t id = 0;
    expect(Ops::op_field("ai.blocking.max_blocking_distance", &id) == SWOFTY_ASSET_OK && id == SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE,
           "the four fields resolve by path");
    expect(Ops::op_field("ai.blocking.fraction_of_follow_speed_at_race_start", &id) == SWOFTY_ASSET_OK &&
               id == SWOFTY_CAR_AI_BLOCKING_FRACTION_RACE_START,
           "the race start fraction resolves by path");

    int r = 0;
    get(0, SWOFTY_CAR_AI_BLOCKING_FRACTION, &r);
    expect(r == SWOFTY_ASSET_UNAVAILABLE, "get is unavailable before the AI system is captured");
    uint32_t t1 = 0;
    expect(Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SET, 40.0f, 0, &t1) == SWOFTY_ASSET_OK,
           "a layer made before the capture is accepted");
    expect(writes == 0, "nothing is written before the AI system is captured");

    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w.system));
    for (uint32_t i = 0; i < rc::SETTLE_FRAMES - 2; i++)
        Ops::racecraft_tick();
    Ops::flush();
    expect(writes == 0, "nothing is written while the build is still settling");
    uint64_t original1 = slot_of(w, 1, SLOT2);
    settle();
    expect(slot_of(w, 1, SLOT2) != original1, "once settled the car's slot points at an own cell");
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 40.0f, "get returns the value the AI reads");
    expect(db_float(w, 1, 2) == 30.0f, "the game's cell is never written");
    expect(slot_of(w, 0, SLOT2) == uint64_t(reinterpret_cast<uintptr_t>(&w.db[0][2].cell)) && get(0, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 30.0f,
           "another car's slot is untouched");
    expect(slot_of(w, 1, SLOT0) == uint64_t(reinterpret_cast<uintptr_t>(&w.db[1][0].cell)), "another field's slot is untouched");
    {
        uint64_t cellp = slot_of(w, 1, SLOT2), recp = 0;
        memcpy(&recp, reinterpret_cast<void *>(cellp), 8);
        expect(recp != 0 && recp != reinterpret_cast<uintptr_t>(w.db[1][2].record) &&
                   reinterpret_cast<uint8_t *>(recp)[0x10] == 0xa2,
               "the own record is a copy of the original's first bytes");
    }

    uint32_t t2 = 0, t3 = 0;
    Ops::op_layer("a", 1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SCALE, 0.5f, 5, &t2);
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 20.0f, "layers compose by priority: set 40 then scale 0.5");
    Ops::op_layer("b", SWOFTY_CAR_ALL, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_ADD, -0.1f, 0, &t3);
    expect(get(0, SWOFTY_CAR_AI_BLOCKING_FRACTION) > 0.774f && get(0, SWOFTY_CAR_AI_BLOCKING_FRACTION) < 0.776f &&
               get(2, SWOFTY_CAR_AI_BLOCKING_FRACTION) > 0.774f && get(2, SWOFTY_CAR_AI_BLOCKING_FRACTION) < 0.776f,
           "an all-cars add layer uses each car's own original as the base");
    expect(db_float(w, 0, 0) == 0.875f, "still no write to the game's cells");

    Ops::op_unlayer(t2);
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 40.0f, "dropping one layer recomposes the rest");
    Ops::op_unlayer(t1);
    expect(slot_of(w, 1, SLOT2) == original1, "the last layer going puts the original pointer back");
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 30.0f, "the AI reads the game's value again");
    Ops::op_unlayer(t3);
    expect(slot_of(w, 0, SLOT0) == uint64_t(reinterpret_cast<uintptr_t>(&w.db[0][0].cell)), "all slots are back");

    int before = writes;
    uint32_t t4 = 0;
    Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SET, 55.0f, 0, &t4);
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 55.0f && writes == before + 1,
           "a second swap writes the slot once and reuses the cell");
    uint64_t swapped = slot_of(w, 1, SLOT2);
    {
        uint8_t *old = rec_at(w, 1);
        std::vector<uint8_t> moved(w.records.size() + 4 * rc::STRIDE, 0);
        memcpy(moved.data(), w.records.data(), w.records.size());
        w.records.swap(moved);
        fill_system(w, CARS);
        (void)old;
    }
    Ops::racecraft_tick();
    expect(slot_of(w, 1, SLOT2) == swapped && get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 55.0f,
           "records moved by a regrow keep their swapped slot");
    Ops::op_unlayer(t4);
    expect(slot_of(w, 1, SLOT2) == original1, "a layer dropped after the records moved restores the original in the new buffer");

    rec_at(w, 2)[rc::R_VEHICLE_INDEX] = 9;
    before = writes;
    uint32_t t5 = 0;
    Ops::op_layer("lap1", 2, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SET, 55.0f, 0, &t5);
    int rr = 0;
    get(2, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, &rr);
    expect(writes == before && rr == SWOFTY_ASSET_UNAVAILABLE, "a record whose vehicle index is not the car's is refused");
    rec_at(w, 2)[rc::R_VEHICLE_INDEX] = 2;
    Ops::op_unlayer(t5);

    uint32_t t6 = 0;
    Ops::op_layer("lap1", 0, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.5f, 0, &t6);
    uint64_t game_ptr = reinterpret_cast<uintptr_t>(&w.db[2][0].cell);
    memcpy(rec_at(w, 0) + SLOT0, &game_ptr, 8);
    Ops::op_unlayer(t6);
    expect(slot_of(w, 0, SLOT0) == game_ptr, "a slot the game rewrote is not overwritten on restore");
    {
        uint64_t original = reinterpret_cast<uintptr_t>(&w.db[0][0].cell);
        memcpy(rec_at(w, 0) + SLOT0, &original, 8);
    }

    uint32_t t7 = 0;
    flow = false;
    before = writes;
    Ops::op_layer("addon", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.6f, 0, &t7);
    expect(writes == before, "a layer from another thread writes nothing");
    flow = true;
    Ops::flush();
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION) == 0.6f, "the next flow thread flush applies it");
    expect(Ops::op_set(1, nullptr, SWOFTY_CAR_AI_BLOCKING_FRACTION, 0.7f) == SWOFTY_ASSET_UNAVAILABLE,
           "set is not offered, layers only");

    Ops::racecraft_forget();
    expect(Ops::racecraft.retired_cells() > 0, "a forgotten session retires its cells instead of freeing them");
    int h = 0;
    get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION, &h);
    expect(h == SWOFTY_ASSET_UNAVAILABLE, "after the reset nothing is read until the next AI system");
    World w2;
    build(w2);
    std::vector<uint8_t> w1_records = w.records;
    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w2.system));
    settle();
    expect(w.records == w1_records, "the old session's records are never touched after the reset");
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION) == 0.6f &&
               slot_of(w2, 1, SLOT0) != uint64_t(reinterpret_cast<uintptr_t>(&w2.db[1][0].cell)),
           "a layer still held applies to the next session's cars");
    Ops::op_unlayer(t7);
    expect(slot_of(w2, 1, SLOT0) == uint64_t(reinterpret_cast<uintptr_t>(&w2.db[1][0].cell)), "and restores them");

    uint32_t t8 = 0;
    Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.1f, 0, &t8);
    before = writes;
    fill_system(w2, 2);
    Ops::racecraft_tick();
    Ops::flush();
    get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION, &h);
    expect(h == SWOFTY_ASSET_UNAVAILABLE && writes == before, "a changed record count drops all state and writes nothing");
    fill_system(w2, CARS);
    Ops::op_unlayer(t8);

    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w2.system));
    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w2.system));
    settle();
    uint32_t t9 = 0;
    Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.1f, 0, &t9);
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION) == 0.1f, "a push at an unchanged count keeps the state");
    dead.insert(reinterpret_cast<uintptr_t>(w2.system) & ~uintptr_t(0xffff));
    before = writes;
    Ops::racecraft_tick();
    Ops::op_unlayer(t9);
    expect(writes == before, "an unreadable AI system is forgotten and not written to");
    dead.clear();

    Ops::bind(make_env());
    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w2.system));
    settle();
    rows_off.insert("AiCarRecord.driver_config");
    uint32_t t10 = 0;
    expect(Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.1f, 0, &t10) == SWOFTY_ASSET_UNAVAILABLE,
           "the fields are off until their KB rows have an agree verify row");
    rows_off.clear();
    rows_off.insert("AiDriverConfig.blocking_max_blocking_distance");
    expect(Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SET, 10.0f, 0, &t10) ==
                   SWOFTY_ASSET_UNAVAILABLE &&
               Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.1f, 0, &t10) == SWOFTY_ASSET_OK,
           "each field has its own row");
    rows_off.clear();

    printf("%s\n", fails ? "FAILED" : "all ok");
    return fails ? 1 : 0;
}
