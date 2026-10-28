/* The C mirror of every standard library 1.6 event payload, with its x86_64 size and field offsets.
   sdk_api_c_test.c checks the mirror compiles as C with these numbers; sdk_headers_test.cpp checks the
   C++ payload structs have the same size and offsets, and are standard layout and trivially copyable. */
#ifndef SWOFTY_TEST_SDK_PAYLOADS_H
#define SWOFTY_TEST_SDK_PAYLOADS_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct SwoftyEvShell {
    uint32_t size, payload_version;
} SwoftyEvShell;

typedef struct SwoftyEvCar {
    uint32_t index;
    void *handle;
} SwoftyEvCar;

typedef struct SwoftyEvSeasonStart {
    uint32_t size, payload_version;
    int32_t year;
    int32_t season_index;
} SwoftyEvSeasonStart;

typedef struct SwoftyEvAddActivity {
    uint32_t size, payload_version;
    void *calendar;
    const void *activity;
    uint32_t driver;
    uint32_t day;
    uint8_t type;
    bool player;
} SwoftyEvAddActivity;

typedef struct SwoftyEvStandingsRebuild {
    uint32_t size, payload_version;
    void *championship;
} SwoftyEvStandingsRebuild;

typedef struct SwoftyEvUpgradesFit {
    uint32_t size, payload_version;
    int32_t car;
    const char *vehicle_id;
    int32_t department;
    float driver_stat;
    float rating;
} SwoftyEvUpgradesFit;

typedef struct SwoftyEvVtfRead {
    uint32_t size, payload_version;
    SwoftyEvCar car;
    const char *section_str;
    const char *field_str;
    const char *upgrade_str;
    float value;
} SwoftyEvVtfRead;

typedef struct SwoftyEvBuilt {
    uint32_t size, payload_version;
    SwoftyEvCar car;
} SwoftyEvBuilt;

typedef struct SwoftyEvReset {
    uint32_t size, payload_version;
    SwoftyEvCar car;
    bool upgrade;
} SwoftyEvReset;

typedef struct SwoftyEvPartsRegister {
    uint32_t size, payload_version;
    int32_t car;
    const char *vehicle_id;
    void *front_wing;
    int32_t (*add_fn)(void *user, const char *node, float detach_at, bool debris);
    void *user;
} SwoftyEvPartsRegister;

typedef struct SwoftyEvBundleResolve {
    uint32_t size, payload_version;
    int32_t car;
    const char *vehicle_id;
    const char *switch_name_str;
    const char *engine_sound_id;
    int32_t (*set_engine_sound_fn)(void *user, const char *id);
    void *user;
} SwoftyEvBundleResolve;

/* X(C struct, C++ namespace, C++ struct, size) */
#define SWOFTY_EV_PAYLOADS(X)                                                                                   \
    X(SwoftyEvShell, career, Created, 8)                                                                       \
    X(SwoftyEvShell, career, Loaded, 8)                                                                        \
    X(SwoftyEvShell, career, Unloaded, 8)                                                                      \
    X(SwoftyEvShell, career, SeasonEnd, 8)                                                                     \
    X(SwoftyEvShell, career, SeasonBreak, 8)                                                                   \
    X(SwoftyEvShell, career, Saving, 8)                                                                        \
    X(SwoftyEvSeasonStart, career, SeasonStart, 16)                                                            \
    X(SwoftyEvAddActivity, career, AddActivity, 40)                                                            \
    X(SwoftyEvStandingsRebuild, career, StandingsRebuild, 16)                                                  \
    X(SwoftyEvUpgradesFit, cars, UpgradesFit, 40)                                                              \
    X(SwoftyEvVtfRead, cars, VtfRead, 56)                                                                      \
    X(SwoftyEvBuilt, cars, Built, 24)                                                                          \
    X(SwoftyEvReset, cars, Reset, 32)                                                                          \
    X(SwoftyEvPartsRegister, cars, PartsRegister, 48)                                                          \
    X(SwoftyEvBundleResolve, sounds, BundleResolve, 56)

/* X(C struct, C++ namespace, C++ struct, field, offset) */
#define SWOFTY_EV_FIELDS(X)                                                                                     \
    X(SwoftyEvSeasonStart, career, SeasonStart, size, 0)                                                       \
    X(SwoftyEvSeasonStart, career, SeasonStart, payload_version, 4)                                            \
    X(SwoftyEvSeasonStart, career, SeasonStart, year, 8)                                                       \
    X(SwoftyEvSeasonStart, career, SeasonStart, season_index, 12)                                              \
    X(SwoftyEvAddActivity, career, AddActivity, calendar, 8)                                                   \
    X(SwoftyEvAddActivity, career, AddActivity, activity, 16)                                                  \
    X(SwoftyEvAddActivity, career, AddActivity, driver, 24)                                                    \
    X(SwoftyEvAddActivity, career, AddActivity, day, 28)                                                       \
    X(SwoftyEvAddActivity, career, AddActivity, type, 32)                                                      \
    X(SwoftyEvAddActivity, career, AddActivity, player, 33)                                                    \
    X(SwoftyEvStandingsRebuild, career, StandingsRebuild, championship, 8)                                     \
    X(SwoftyEvUpgradesFit, cars, UpgradesFit, size, 0)                                                         \
    X(SwoftyEvUpgradesFit, cars, UpgradesFit, payload_version, 4)                                              \
    X(SwoftyEvUpgradesFit, cars, UpgradesFit, car, 8)                                                          \
    X(SwoftyEvUpgradesFit, cars, UpgradesFit, vehicle_id, 16)                                                  \
    X(SwoftyEvUpgradesFit, cars, UpgradesFit, department, 24)                                                  \
    X(SwoftyEvUpgradesFit, cars, UpgradesFit, driver_stat, 28)                                                 \
    X(SwoftyEvUpgradesFit, cars, UpgradesFit, rating, 32)                                                      \
    X(SwoftyEvVtfRead, cars, VtfRead, car, 8)                                                                  \
    X(SwoftyEvVtfRead, cars, VtfRead, section_str, 24)                                                         \
    X(SwoftyEvVtfRead, cars, VtfRead, field_str, 32)                                                           \
    X(SwoftyEvVtfRead, cars, VtfRead, upgrade_str, 40)                                                         \
    X(SwoftyEvVtfRead, cars, VtfRead, value, 48)                                                               \
    X(SwoftyEvBuilt, cars, Built, car, 8)                                                                      \
    X(SwoftyEvReset, cars, Reset, car, 8)                                                                      \
    X(SwoftyEvReset, cars, Reset, upgrade, 24)                                                                 \
    X(SwoftyEvPartsRegister, cars, PartsRegister, car, 8)                                                      \
    X(SwoftyEvPartsRegister, cars, PartsRegister, vehicle_id, 16)                                              \
    X(SwoftyEvPartsRegister, cars, PartsRegister, front_wing, 24)                                              \
    X(SwoftyEvPartsRegister, cars, PartsRegister, add_fn, 32)                                                  \
    X(SwoftyEvPartsRegister, cars, PartsRegister, user, 40)                                                    \
    X(SwoftyEvBundleResolve, sounds, BundleResolve, car, 8)                                                    \
    X(SwoftyEvBundleResolve, sounds, BundleResolve, vehicle_id, 16)                                            \
    X(SwoftyEvBundleResolve, sounds, BundleResolve, switch_name_str, 24)                                       \
    X(SwoftyEvBundleResolve, sounds, BundleResolve, engine_sound_id, 32)                                       \
    X(SwoftyEvBundleResolve, sounds, BundleResolve, set_engine_sound_fn, 40)                                   \
    X(SwoftyEvBundleResolve, sounds, BundleResolve, user, 48)

#endif
