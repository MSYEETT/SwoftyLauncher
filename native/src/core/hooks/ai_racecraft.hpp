#pragma once
#include <cstdint>
#include <cstring>
#include <functional>
#include <mutex>
#include <vector>

// Per-car AI racecraft fields (car_live.hpp, ai.blocking.*). The AI system S (the 0x1a8 object Sess_Build_AiSystem
// builds, captured at site_Ai_BuildSystem_RecordPushed) keeps one record R per vehicle, stride 0x508, in the vector
// [S+0xa8, S+0xb0). R+0x00 is the vehicle index and R+0x238 is the driver config D copied by value; every config field
// of D is an 8-byte slot holding a pointer P to the game's database cell: rec = *P, value = *(float *)(rec + 8).
// The blocking evaluator (Ai_Blocking_Evaluate, 0x140624e60) re-reads the slot, P, rec and the float on every call,
// so a swapped slot is seen on the next tick.
//
// A car's field is changed by pointing its slot at a core-owned cell: cell[0] holds a pointer to an own record (a copy
// of the original record's first bytes) whose float at +8 is the composed value. The shared database cell is never
// written. The original slot pointer is kept and written back when the car's last layer on the field goes away.
// Owned cells are never freed: the game's AI threads may still be reading one when it is swapped out or the session
// ends, so a session leaks at most MAX_CARS * FIELD_COUNT small blocks, handed to `retired` on forget().
//
// Every method takes the lock. pushed() runs on the thread that builds the AI system; the rest run on the flow thread
// (apply, tick, forget) or any thread (get).

namespace swofty_core::hooks::ai_racecraft {

constexpr uintptr_t RECORDS_BEGIN = 0xa8, RECORDS_END = 0xb0, STRIDE = 0x508;
constexpr uintptr_t R_VEHICLE_INDEX = 0x00, R_DRIVER = 0x238;
constexpr uintptr_t VALUE_AT = 8, USER_BASE = 0x10000, USER_END = 0x800000000000ull;
constexpr uint32_t MAX_RECORDS = 64, MAX_CARS = 32, FIELD_COUNT = 4;
constexpr uint32_t SETTLE_FRAMES = 30;
constexpr size_t RECORD_COPY = 0x40, RECORD_MIN = 0x10;

enum Slot : uint32_t { fraction, fraction_race_start, max_distance, max_distance_race_start };
constexpr uintptr_t DRIVER_OFFSET[FIELD_COUNT] = {0x90, 0x98, 0xa0, 0xa8};

enum class Result { ok, unavailable, late };

using Read = std::function<bool(uintptr_t addr, void *out, size_t n)>;
using Write = std::function<bool(uintptr_t addr, const void *in, size_t n)>;

struct Mem {
    Read read;
    Write write;
};

inline bool user_pointer(uint64_t p) { return p >= USER_BASE && p < USER_END && !(p & 7); }

struct Cell {
    alignas(16) unsigned char bytes[0x10 + RECORD_COPY] = {};
    uint64_t *pointer() { return reinterpret_cast<uint64_t *>(bytes); }
    unsigned char *record() { return bytes + 0x10; }
    float *value() { return reinterpret_cast<float *>(bytes + 0x10 + VALUE_AT); }
};

struct Swap {
    bool active = false;
    uintptr_t slot = 0, original = 0;
    Cell *cell = nullptr;
};

class Fields {
public:
    void pushed(const Mem &m, uintptr_t system)
    {
        std::lock_guard<std::mutex> g(lock);
        uint32_t n = 0;
        if (!records(m, system, n))
            return;
        if (system != system_ || n <= count_)
            forget_locked();
        system_ = system;
        count_ = n;
        quiet_ = 0;
    }

    void forget()
    {
        std::lock_guard<std::mutex> g(lock);
        forget_locked();
    }

    bool tick(const Mem &m)
    {
        std::lock_guard<std::mutex> g(lock);
        if (!system_)
            return false;
        uint32_t n = 0;
        if (!records(m, system_, n) || n != count_) {
            forget_locked();
            return false;
        }
        if (quiet_ < SETTLE_FRAMES)
            quiet_++;
        bool now = quiet_ == SETTLE_FRAMES && waiting_;
        if (now)
            waiting_ = false;
        return now;
    }

    bool settled() const
    {
        std::lock_guard<std::mutex> g(lock);
        return system_ && quiet_ >= SETTLE_FRAMES;
    }

    bool read(const Mem &m, uint32_t car, uint32_t field, float &out)
    {
        std::lock_guard<std::mutex> g(lock);
        uintptr_t r = 0;
        if (field >= FIELD_COUNT || !record(m, car, r))
            return false;
        uint64_t p = 0;
        float v = 0;
        if (!m.read(r + R_DRIVER + DRIVER_OFFSET[field], &p, 8) || !original_value(m, p, v))
            return false;
        out = v;
        return true;
    }

    Result apply(const Mem &m, uint32_t car, uint32_t field, bool want, const std::function<float(float base)> &compose)
    {
        std::lock_guard<std::mutex> g(lock);
        if (field >= FIELD_COUNT || car >= MAX_CARS)
            return Result::unavailable;
        Swap &w = swaps_[car][field];
        uintptr_t r = 0;
        if (!record(m, car, r)) {
            if (want)
                waiting_ = true;
            return Result::unavailable;
        }
        uintptr_t slot = r + R_DRIVER + DRIVER_OFFSET[field];
        uint64_t current = 0;
        if (!m.read(slot, &current, 8))
            return Result::late;
        if (w.active && current != reinterpret_cast<uintptr_t>(w.cell->bytes)) {
            w.active = false;
            if (!want)
                return Result::ok;
        }
        if (w.active)
            w.slot = slot;
        if (!want) {
            if (w.active) {
                if (!m.write(slot, &w.original, 8))
                    return Result::late;
                w.active = false;
            }
            return Result::ok;
        }
        uint64_t original = w.active ? w.original : current;
        float base = 0;
        if (!original_value(m, original, base))
            return Result::unavailable;
        float v = compose(base);
        if (!w.active) {
            if (!w.cell)
                w.cell = new Cell();
            uint64_t rec = 0;
            m.read(original, &rec, 8);
            std::memset(w.cell->bytes, 0, sizeof w.cell->bytes);
            if (!m.read(rec, w.cell->record(), RECORD_COPY) && !m.read(rec, w.cell->record(), RECORD_MIN))
                return Result::unavailable;
            *w.cell->pointer() = reinterpret_cast<uintptr_t>(w.cell->record());
        }
        *w.cell->value() = v;
        if (!w.active) {
            uint64_t mine = reinterpret_cast<uintptr_t>(w.cell->bytes);
            if (!m.write(slot, &mine, 8))
                return Result::late;
            w.active = true;
            w.slot = slot;
            w.original = original;
        }
        return Result::ok;
    }

    bool swapped(uint32_t car, uint32_t field) const
    {
        std::lock_guard<std::mutex> g(lock);
        return car < MAX_CARS && field < FIELD_COUNT && swaps_[car][field].active;
    }

    size_t retired_cells() const
    {
        std::lock_guard<std::mutex> g(lock);
        return retired.size();
    }

    uintptr_t system() const
    {
        std::lock_guard<std::mutex> g(lock);
        return system_;
    }

private:
    mutable std::mutex lock;
    uintptr_t system_ = 0;
    uint32_t count_ = 0, quiet_ = 0;
    bool waiting_ = false;
    Swap swaps_[MAX_CARS][FIELD_COUNT];
    std::vector<Cell *> retired;

    static bool records(const Mem &m, uintptr_t system, uint32_t &n)
    {
        uint64_t begin = 0, end = 0;
        if (!user_pointer(system) || !m.read(system + RECORDS_BEGIN, &begin, 8) ||
            !m.read(system + RECORDS_END, &end, 8) || !user_pointer(begin) || end < begin + STRIDE ||
            (end - begin) % STRIDE != 0 || (end - begin) / STRIDE > MAX_RECORDS)
            return false;
        n = uint32_t((end - begin) / STRIDE);
        return true;
    }

    bool record(const Mem &m, uint32_t car, uintptr_t &r)
    {
        if (!system_)
            return false;
        uint32_t n = 0;
        if (!records(m, system_, n) || n != count_) {
            forget_locked();
            return false;
        }
        if (quiet_ < SETTLE_FRAMES || car >= count_)
            return false;
        uint64_t begin = 0;
        uint8_t index = 0;
        if (!m.read(system_ + RECORDS_BEGIN, &begin, 8))
            return false;
        r = begin + uintptr_t(car) * STRIDE;
        return m.read(r + R_VEHICLE_INDEX, &index, 1) && index == car;
    }

    static bool original_value(const Mem &m, uint64_t p, float &v)
    {
        uint64_t rec = 0;
        return user_pointer(p) && m.read(p, &rec, 8) && user_pointer(rec) && m.read(rec + VALUE_AT, &v, 4);
    }

    void forget_locked()
    {
        for (auto &row : swaps_)
            for (auto &w : row) {
                if (w.cell)
                    retired.push_back(w.cell);
                w = Swap{};
            }
        system_ = 0;
        count_ = quiet_ = 0;
    }
};

}
