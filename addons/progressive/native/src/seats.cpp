// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: entrant rows and their writer (seats.hpp).

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <cstring>

#include <swofty/career.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include "seats.hpp"

using namespace swofty::game;

namespace pg {

namespace {

constexpr uintptr_t kVtWeekendIndex = 0x108;    // weekend source vtable slot: the weekend index
constexpr uintptr_t kVtCurrentWeekendOf = 0xa8; // entity component registry vtable slot: the current weekend
constexpr uintptr_t kWeekendRowsBegin = 0x20;   // Weekend: its line-up vector object at +0x18 {alloc, begin, end}
constexpr uintptr_t kWeekendRowsEnd = 0x28;
constexpr uint32_t kMinutesPerDay = 1440;
constexpr size_t kMaxSeasons = 64;

using WeekendFn = uint32_t (*)(void *self);
using CurrentWeekendFn = uintptr_t (*)(void *registry);

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

uintptr_t component(const void *desc)
{
    auto c = reinterpret_cast<uintptr_t>(swofty::career::component_by_desc(desc));
    return c && swofty::mem::object_like(c) ? c : 0;
}

} // namespace

uintptr_t entity()
{
    auto e = reinterpret_cast<uintptr_t>(swofty::career::entity());
    CareerEntity ent{};
    if (!e || !swofty::mem::object_like(e) || !swofty::mem::read(e, ent) ||
        !swofty::mem::in_image(reinterpret_cast<uintptr_t>(ent.component_registry)))
        return 0;
    return e;
}

uintptr_t contracts_manager()
{
    return component(k_Career_DriverCareerContractsManagerComponentDesc.get());
}

uintptr_t championship(uintptr_t e)
{
    uintptr_t want = reinterpret_cast<uintptr_t>(vt_Career_Championship.get());
    uintptr_t cand[2] = {rdv<uintptr_t>(e + offsetof(CareerEntity, current_championship)),
                         component(k_Career_ChampionshipDesc.get())};
    for (uintptr_t c : cand)
        if (c && rdv<uintptr_t>(c) == want)
            return c;
    return 0;
}

bool season(uintptr_t e, SeasonInfo &out)
{
    out = {};
    uintptr_t c = e ? championship(e) : 0;
    Championship ch{};
    if (!c || !swofty::mem::read(c, ch))
        return false;
    auto b = reinterpret_cast<uintptr_t>(ch.seasons_begin), en = reinterpret_cast<uintptr_t>(ch.seasons_end);
    if (en < b || (en - b) % sizeof(ChampionshipSeasonEntry) || (en - b) / sizeof(ChampionshipSeasonEntry) > kMaxSeasons)
        return false;
    out.index = ch.season_index;
    out.seasons = (en - b) / sizeof(ChampionshipSeasonEntry);
    out.completion = ch.completion;
    return true;
}

bool entrant_rows(uintptr_t e, std::vector<Row> &out)
{
    out.clear();
    uintptr_t b = rdv<uintptr_t>(e + offsetof(CareerEntity, entrants_begin));
    uintptr_t en = rdv<uintptr_t>(e + offsetof(CareerEntity, entrants_end));
    if (!e || !b || en <= b || (en - b) % sizeof(EntrantRecord) || (en - b) / sizeof(EntrantRecord) > kMaxEntrants ||
        !swofty::mem::readable(b, en - b))
        return false;
    size_t n = (en - b) / sizeof(EntrantRecord);
    out.resize(n);
    for (size_t i = 0; i < n; ++i) {
        out[i].at = b + i * sizeof(EntrantRecord);
        std::memcpy(&out[i].rec, reinterpret_cast<const void *>(out[i].at), sizeof(EntrantRecord));
    }
    return true;
}

uint32_t driver_of(const EntrantRecord &r)
{
    uint32_t v;
    std::memcpy(&v, &r.id_lo, sizeof v);
    return v;
}

const Row *row_of(const std::vector<Row> &rows, uint32_t driver)
{
    const Row *hit = nullptr;
    for (auto &r : rows)
        if ((driver_of(r.rec) & 0xffff00ffu) == (driver & 0xffff00ffu)) {
            if (hit)
                return nullptr;
            hit = &r;
        }
    return hit;
}

bool entity_busy(uintptr_t e)
{
    return rdv<uint8_t>(e + offsetof(CareerEntity, entrant_update_busy)) != 0;
}

bool unique_identity(uintptr_t e, const Row &id_row)
{
    std::vector<Row> rows;
    if (!entrant_rows(e, rows))
        return false;
    uint32_t id = id_row.rec.ordinal_flags;
    bool want_human = id & kHumanBit;
    size_t hits = 0;
    uintptr_t at = 0;
    for (auto &r : rows) {
        uint32_t v = r.rec.ordinal_flags;
        bool match = want_human ? (v & kHumanBit) != 0 : !(v & kHumanBit) && v == id;
        if (match) {
            ++hits;
            at = r.at;
        }
    }
    return hits == 1 && at == id_row.at;
}

EntrantWriteData seat_data(const Row &driver_row, const Row &seat_row)
{
    EntrantWriteData d{};
    d.driver = driver_of(driver_row.rec);
    d.driver_customisation = driver_row.rec.driver_customisation;
    d.team_stat_id = seat_row.rec.team_stat_id;
    d.vehicle_ref = seat_row.rec.vehicle_ref;
    d.driver_reg_value = seat_row.rec.driver_reg_value;
    d.vehicle_customisation = seat_row.rec.vehicle_customisation;
    d.badge_stat_id = seat_row.rec.badge_stat_id;
    d.seat_value = seat_row.rec.const_ffff0012;
    return d;
}

EntrantWriteData own_data(const Row &r)
{
    return seat_data(r, r);
}

EntrantWriteData released_data(const Row &r)
{
    EntrantWriteData d{};
    d.driver = driver_of(r.rec);
    d.driver_customisation = r.rec.driver_customisation;
    d.team_stat_id = kNoKey;
    d.vehicle_ref = kNoKey;
    d.driver_reg_value = kNoKey;
    d.vehicle_customisation = kNoKey;
    d.badge_stat_id = kNoKey;
    d.seat_value = 0xffff0012u;
    return d;
}

bool announce(uintptr_t manager, uint32_t driver, uint32_t old_team, uint32_t new_team, uint8_t reason)
{
    using M = DriverCareerContractsManagerComponent;
    if (!manager)
        return false;
    uintptr_t src = rdv<uintptr_t>(manager + offsetof(M, weekend_source));
    uintptr_t time = rdv<uintptr_t>(manager + offsetof(M, career_time));
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

bool write_row(uintptr_t e, const Row &id_row, const EntrantWriteData &data, uint16_t position, uint8_t number)
{
    if (entity_busy(e) || !unique_identity(e, id_row))
        return false;
    uint8_t id[kIdentitySize];
    std::memcpy(id, &id_row.rec, kIdentitySize);
    Career_Set_EntrantData_Thunk(reinterpret_cast<void *>(e), id, reinterpret_cast<const uint32_t *>(&data), position,
                                 number);
    return !entity_busy(e);
}

uintptr_t weekend_row(uintptr_t e, const EntrantRecord &row)
{
    uintptr_t registry = e + offsetof(CareerEntity, component_registry);
    uintptr_t vt = rdv<uintptr_t>(registry);
    uintptr_t fn = vt ? rdv<uintptr_t>(vt + kVtCurrentWeekendOf) : 0;
    if (!swofty::mem::in_image(fn))
        return 0;
    uintptr_t weekend = reinterpret_cast<CurrentWeekendFn>(fn)(reinterpret_cast<void *>(registry));
    uintptr_t b = weekend ? rdv<uintptr_t>(weekend + kWeekendRowsBegin) : 0;
    uintptr_t en = weekend ? rdv<uintptr_t>(weekend + kWeekendRowsEnd) : 0;
    if (!b || en < b || (en - b) % sizeof(EntrantRecord) || (en - b) / sizeof(EntrantRecord) > kMaxEntrants)
        return 0;
    for (uintptr_t p = b; p < en; p += sizeof(EntrantRecord)) {
        uint32_t v = rdv<uint32_t>(p + offsetof(EntrantRecord, ordinal_flags));
        bool want_human = row.ordinal_flags & kHumanBit;
        if (want_human ? (v & kHumanBit) != 0 : !(v & kHumanBit) && v == row.ordinal_flags)
            return driver_of(rdv<EntrantRecord>(p)) == driver_of(row) ? p : 0;
    }
    return 0;
}

bool patch(uintptr_t at, size_t offset, uint32_t value)
{
    if (!writable(at + offset, sizeof value))
        return false;
    std::memcpy(reinterpret_cast<void *>(at + offset), &value, sizeof value);
    return true;
}

bool patch_u8(uintptr_t at, size_t offset, uint8_t value)
{
    if (!writable(at + offset, sizeof value))
        return false;
    *reinterpret_cast<uint8_t *>(at + offset) = value;
    return true;
}

namespace {

constexpr uintptr_t kVtRegistryValue = 0x108; // entity component registry vtable slot: the contract state value
constexpr uint8_t kContractReleased = 3;
constexpr uint8_t kContractRenewed = 4;
constexpr uint8_t kContractLeft = 5;
constexpr size_t kContractSlot = 16;
static_assert(kContractRecord == 0x2c && sizeof(DriverContractChangedMessage) == 0x78);

using RegistryValueFn = uint32_t (*)(void *registry);

bool same_key(uint32_t a, uint32_t b)
{
    return ((a ^ b) & 0xffff00ffu) == 0;
}

bool contract_clock(uintptr_t e, uintptr_t manager, uint16_t &day, uint32_t &value)
{
    uintptr_t time = rdv<uintptr_t>(manager + offsetof(DriverCareerContractsManagerComponent, career_time));
    uintptr_t registry = e + offsetof(CareerEntity, component_registry);
    uintptr_t vt = rdv<uintptr_t>(registry);
    uintptr_t fn = vt ? rdv<uintptr_t>(vt + kVtRegistryValue) : 0;
    if (!time || !swofty::mem::in_image(fn))
        return false;
    value = reinterpret_cast<RegistryValueFn>(fn)(reinterpret_cast<void *>(registry));
    day = static_cast<uint16_t>(rdv<uint32_t>(time + offsetof(CareerTimeComponent, current_minutes)) / kMinutesPerDay);
    return true;
}

} // namespace

bool contract_objects(uintptr_t manager, std::vector<uintptr_t> &out)
{
    out.clear();
    uintptr_t owner = manager ? rdv<uintptr_t>(manager + offsetof(DriverCareerContractsManagerComponent, contract_owner)) : 0;
    uintptr_t b = owner ? rdv<uintptr_t>(owner + offsetof(DriverCareerContractOwner, contracts_begin)) : 0;
    uintptr_t en = owner ? rdv<uintptr_t>(owner + offsetof(DriverCareerContractOwner, contracts_end)) : 0;
    if (!b || en < b || (en - b) % kContractSlot || (en - b) / kContractSlot > 4096 || !swofty::mem::readable(b, en - b))
        return false;
    for (uintptr_t p = b; p < en; p += kContractSlot)
        if (uintptr_t c = rdv<uintptr_t>(p + sizeof(void *)); c && swofty::mem::readable(c, sizeof(DriverContract)))
            out.push_back(c);
    return true;
}

uintptr_t contract_of(const std::vector<uintptr_t> &objects, uint32_t driver)
{
    for (uintptr_t c : objects)
        if (same_key(rdv<uint32_t>(c + offsetof(DriverContract, driver)), driver))
            return c;
    return 0;
}

bool contract_fits(uintptr_t contract, uint32_t team)
{
    return contract && rdv<uint8_t>(contract + offsetof(DriverContract, active)) &&
           same_key(rdv<uint32_t>(contract + offsetof(DriverContract, team)), team);
}

const char *plan_signing(const std::vector<uintptr_t> &objects, uint32_t human, uint32_t driver, uint32_t outgoing,
                         uint32_t team, Signing &s)
{
    s.driver = driver;
    s.contract = contract_of(objects, driver);
    if (!s.contract)
        return "the game keeps no contract for him";
    if (!writable(s.contract, offsetof(DriverContract, active) + 1))
        return "his contract is not writable";
    uintptr_t terms = 0;
    for (uintptr_t c : objects) {
        uint32_t d = rdv<uint32_t>(c + offsetof(DriverContract, driver));
        if (!contract_fits(c, team))
            continue;
        if (same_key(d, outgoing)) {
            terms = c;
            break;
        }
        if (!terms && !same_key(d, human))
            terms = c;
    }
    if (!terms || !swofty::mem::read(terms, s.offer))
        return "no driver holds an active contract with that team to copy the terms from";
    return nullptr;
}

bool sign_contract(uintptr_t e, uintptr_t manager, const Signing &s)
{
    using C = DriverContract;
    uint16_t day;
    uint32_t value;
    if (!contract_clock(e, manager, day, value) || !writable(s.contract, offsetof(C, active) + 1) ||
        !vt_Career_MessageDriverContractChanged.address())
        return false;
    uint8_t offer[kContractRecord];
    std::memcpy(offer, s.offer, sizeof offer);
    std::memcpy(offer + offsetof(C, signed_day), &day, sizeof day);
    uint32_t team;
    std::memcpy(&team, offer + offsetof(C, team), sizeof team);
    bool same = same_key(rdv<uint32_t>(s.contract + offsetof(C, team)), team);
    Career_Set_DriverContractState(reinterpret_cast<void *>(s.contract), day, same ? kContractRenewed : kContractLeft,
                                   value);
    if (rdv<uint8_t>(s.contract + offsetof(C, active)))
        return false;
    DriverContractChangedMessage m{};
    m.vtable = vt_Career_MessageDriverContractChanged.get();
    m.driver = rdv<uint32_t>(s.contract + offsetof(C, driver));
    std::memcpy(m.before, reinterpret_cast<const void *>(s.contract), sizeof m.before);
    std::memcpy(reinterpret_cast<void *>(s.contract), offer, sizeof offer);
    *reinterpret_cast<uint8_t *>(s.contract + offsetof(C, active)) = 1;
    std::memcpy(m.after, offer, sizeof m.after);
    m.kind = offer[offsetof(C, kind)];
    return swofty::career::post(&m) == swofty::career::Result::ok;
}

bool release_contract(uintptr_t e, uintptr_t manager, uint32_t driver, bool &found)
{
    found = false;
    std::vector<uintptr_t> objects;
    uint16_t day;
    uint32_t value;
    if (!contract_objects(manager, objects) || !contract_clock(e, manager, day, value))
        return false;
    uintptr_t contract = contract_of(objects, driver);
    if (!contract)
        return true;
    Career_Set_DriverContractState(reinterpret_cast<void *>(contract), day, kContractReleased, value);
    found = true;
    return true;
}

uintptr_t car_numbers(uintptr_t e, uintptr_t manager)
{
    uintptr_t c = manager ? rdv<uintptr_t>(manager + offsetof(DriverCareerContractsManagerComponent, car_numbers)) : 0;
    return c && swofty::mem::object_like(c) && rdv<uintptr_t>(c + offsetof(CarNumberComponent, entity)) == e ? c : 0;
}

Baseline::Baseline(const std::vector<Row> &rows)
{
    for (auto &r : rows)
        rows_.push_back(r.rec);
    vec_.begin = rows_.data();
    vec_.end = rows_.data() + rows_.size();
    vec_.capacity = vec_.end;
}

uint64_t lineup_changes(uintptr_t e, const Baseline &baseline)
{
    if (!Career_Build_LineupChanges.address())
        return ~0ull;
    uintptr_t current = e + offsetof(CareerEntity, entrants_begin) - sizeof(void *);
    DriverMovesFinalisedMessage m{};
    Career_Build_LineupChanges(&m.changes, reinterpret_cast<void *>(current), baseline.vector());
    return m.count;
}

bool finalise(uintptr_t e, const Baseline &baseline, uint64_t &changes)
{
    changes = 0;
    if (!Career_Build_LineupChanges.address())
        return false;
    uintptr_t current = e + offsetof(CareerEntity, entrants_begin) - sizeof(void *);
    DriverMovesFinalisedMessage m{};
    Career_Build_LineupChanges(&m.changes, reinterpret_cast<void *>(current), baseline.vector());
    if (m.count > 40)
        return false;
    changes = m.count;
    void **vts[] = {vt_Career_MessageDriverMovesFinalisedUpgrades.get(),
                    vt_Career_MessageDriverMovesFinalisedRecognition.get(), vt_Career_MessageDriverMovesFinalised.get()};
    for (void **v : vts) {
        DriverMovesFinalisedMessage copy = m;
        copy.vtable = v;
        copy.reserved = 0;
        if (swofty::career::post(&copy) != swofty::career::Result::ok)
            return false;
    }
    return true;
}

} // namespace pg
