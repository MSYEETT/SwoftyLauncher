/* SPDX-License-Identifier: MIT */
#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SWOFTY_API_VERSION 5

enum {
    SWOFTY_MIXIN_WRAP = 1,
    SWOFTY_MIXIN_SITE = 2,
    SWOFTY_MIXIN_VTABLE = 3,
    SWOFTY_MIXIN_REDIRECT = 4,
    SWOFTY_MIXIN_PATCH = 5,
    SWOFTY_MIXIN_ON_LOAD = 6,
};
#define SWOFTY_MIXIN_OVERWRITE 0x100u
#define SWOFTY_MIXIN_KIND(k) ((k) & 0xffu)
#define SWOFTY_MIXIN_ARG(k) ((k) >> 16)
#define SWOFTY_MIXIN_WITH_ARG(k, arg) ((uint32_t)(k) | (uint32_t)(arg) << 16)

typedef struct SwoftyMixin {
    const char *target;
    uint32_t kind;
    int32_t priority;
    void *handler;
    void **orig;
} SwoftyMixin;

typedef union SwoftyXmm {
    uint8_t u8[16];
    uint32_t u32[4];
    uint64_t u64[2];
    float f32[4];
    double f64[2];
} SwoftyXmm;

typedef struct SwoftyContext {
    SwoftyXmm xmm0, xmm1, xmm2, xmm3, xmm4, xmm5, xmm6, xmm7, xmm8, xmm9, xmm10, xmm11, xmm12, xmm13, xmm14, xmm15;
    uintptr_t rflags, r15, r14, r13, r12, r11, r10, r9, r8, rdi, rsi, rdx, rcx, rbx, rax, rbp, rsp, trampoline_rsp, rip;
} SwoftyContext;

enum {
    SWOFTY_SETTING_TOGGLE = 1,
    SWOFTY_SETTING_SLIDER = 2,
    SWOFTY_SETTING_CHOICE = 3,
};

typedef struct SwoftySettingValue {
    uint32_t type;
    int32_t toggle;
    double slider;
    const char *choice;
} SwoftySettingValue;

typedef void (*SwoftySettingFn)(const char *id, const SwoftySettingValue *value, void *user);

enum {
    SWOFTY_ASSET_OK = 0,
    SWOFTY_ASSET_LATE = 1,
    SWOFTY_ASSET_UNAVAILABLE = 2,
    SWOFTY_ASSET_INVALID = 3,
};

typedef struct SwoftyFragment {
    const char *file;
    const char *fourcc;
    uint32_t flags;
} SwoftyFragment;

typedef struct SwoftyAssets {
    int (*available)(const char *feature);
    int (*register_group)(const char *group, const char *const *erps, size_t erp_count, const char *const *deps,
                          size_t dep_count, const char *templ);
    int (*put_resource)(const char *uri, const SwoftyFragment *fragments, size_t fragment_count, const char *const *erps,
                        size_t erp_count);
    int (*db_set)(const char *key, const char *type, const char *value);
    int (*db_set_raw)(uint64_t hash, uint8_t tag, const void *value, size_t size);
    int (*lang_set)(const char *language, const char *key, const char *text);
    int (*lang_table)(const char *language, const char *file);
    int (*overlay_file)(const char *game_path, const char *file);
} SwoftyAssets;

/* The standard-library tables below use the SWOFTY_ASSET_* result codes: ok, late,
   unavailable (older core, or the feature is off), invalid (bad argument). Each table's
   available() returns non-zero only when its feature is on and the core implements it. */

#define SWOFTY_EVENT_SESSION_START "session.start"
#define SWOFTY_EVENT_SESSION_END "session.end"
#define SWOFTY_EVENT_RACE_START "race.start"
#define SWOFTY_EVENT_LAP "lap"
#define SWOFTY_EVENT_FLAG "flag"
#define SWOFTY_EVENT_STATE "state"
#define SWOFTY_EVENT_SCREEN "screen"
#define SWOFTY_EVENT_PAUSE "pause"
#define SWOFTY_EVENT_RESUME "resume"
#define SWOFTY_EVENT_RESULTS "results"

typedef struct SwoftyEvent {
    const char *name;
    uint64_t frame;
    double time;
    int64_t a, b;
    const void *data;
} SwoftyEvent;

typedef void (*SwoftyEventFn)(const SwoftyEvent *ev, void *user);
typedef void (*SwoftyFrameFn)(double dt, void *user);

typedef struct SwoftyEvents {
    int (*available)(void);
    int (*subscribe)(const char *event, SwoftyEventFn fn, void *user, uint32_t *token);
    int (*on_frame)(SwoftyFrameFn fn, void *user, uint32_t *token);
    int (*unsubscribe)(uint32_t token);
} SwoftyEvents;

typedef struct SwoftyPaint SwoftyPaint;
typedef void (*SwoftyPaintFn)(SwoftyPaint *paint, void *user);

typedef struct SwoftyOverlay {
    int (*available)(void);
    int (*on_paint)(SwoftyPaintFn fn, void *user, uint32_t *token);
    int (*remove)(uint32_t token);
    void (*size)(const SwoftyPaint *paint, float *width, float *height);
    void (*rect)(SwoftyPaint *paint, float x, float y, float w, float h, uint32_t rgba);
    void (*rect_outline)(SwoftyPaint *paint, float x, float y, float w, float h, float thickness, uint32_t rgba);
    void (*line)(SwoftyPaint *paint, float x0, float y0, float x1, float y1, float thickness, uint32_t rgba);
    void (*text)(SwoftyPaint *paint, float x, float y, float size, uint32_t rgba, const char *utf8);
    void (*measure)(const SwoftyPaint *paint, float size, const char *utf8, float *width, float *height);
    int (*load_image)(const char *id, const char *file);
    int (*image)(SwoftyPaint *paint, const char *id, float x, float y, float w, float h, uint32_t tint);
} SwoftyOverlay;

#define SWOFTY_TELEMETRY_VERSION 1

typedef struct SwoftyCarState {
    uint32_t version;
    uint32_t flags;
    float speed;
    float rpm;
    float max_rpm;
    int32_t gear;
    float throttle, brake, clutch, steer;
    float tyre_temp[4];
    float tyre_wear[4];
    float brake_temp[4];
    float fuel;
    float fuel_laps;
    int32_t position;
    uint32_t lap;
    uint32_t sector;
    float last_lap_time, current_lap_time, best_lap_time;
    float sector_time[3];
    float lap_distance;
    float world_pos[3];
    float world_vel[3];
    float reserved[8];
} SwoftyCarState;

typedef struct SwoftySessionState {
    uint32_t version;
    uint32_t session_type;
    uint32_t track;
    uint32_t flag;
    uint32_t weather;
    uint32_t total_laps;
    uint32_t participant_count;
    uint32_t player_index;
    float session_time;
    float session_time_left;
    float track_temp, air_temp;
    float reserved[8];
} SwoftySessionState;

typedef struct SwoftyParticipant {
    uint32_t version;
    uint32_t flags;
    char name[48];
    uint32_t team;
    int32_t position;
    uint32_t lap;
    float lap_distance;
    float best_lap_time;
    float reserved[4];
} SwoftyParticipant;

typedef struct SwoftyTelemetryFeed {
    uint32_t version;
    uint32_t size;
    uint64_t seq;
    uint64_t frame;
    SwoftySessionState session;
    SwoftyCarState cars[24];
    SwoftyParticipant participants[24];
} SwoftyTelemetryFeed;

typedef struct SwoftyTelemetry {
    int (*available)(void);
    int (*session)(SwoftySessionState *out);
    int (*car)(uint32_t index, SwoftyCarState *out);
    int (*participant)(uint32_t index, SwoftyParticipant *out);
    int (*shared_memory)(const char **name);
} SwoftyTelemetry;

enum {
    SWOFTY_DB_INT = 0,
    SWOFTY_DB_FLOAT = 1,
    SWOFTY_DB_STRING = 2,
    SWOFTY_DB_ENUM = 3,
    SWOFTY_DB_ID = 4,
    SWOFTY_DB_BOOL = 5,
};

typedef struct SwoftyDbValue {
    uint8_t tag;
    int64_t i;
    double f;
    const char *s;
} SwoftyDbValue;

typedef struct SwoftyDbCursor {
    uint64_t table;
    uint64_t index;
    uint64_t count;
    const void *row;
} SwoftyDbCursor;

typedef struct SwoftyDb {
    int (*available)(void);
    int (*get)(const char *key, SwoftyDbValue *out);
    int (*get_hash)(uint64_t hash, SwoftyDbValue *out);
    int (*int_at)(const char *key, int64_t *out);
    int (*float_at)(const char *key, double *out);
    int (*string_at)(const char *key, const char **out);
    int (*bool_at)(const char *key, int32_t *out);
    int (*open)(const char *table, SwoftyDbCursor *cursor);
    int (*row)(const char *table, const char *label, SwoftyDbCursor *cursor);
    int (*step)(SwoftyDbCursor *cursor);
    int (*cell)(const SwoftyDbCursor *cursor, const char *field, SwoftyDbValue *out);
    const char *(*row_name)(const SwoftyDbCursor *cursor);
} SwoftyDb;

typedef struct SwoftyStorage {
    int (*available)(void);
    int (*read)(const char *path, void *buf, size_t cap, size_t *len);
    int (*write)(const char *path, const void *data, size_t len);
    int (*append)(const char *path, const void *data, size_t len);
    int (*remove)(const char *path);
    int (*size)(const char *path, uint64_t *out);
    int (*list)(const char *dir, char *buf, size_t cap, size_t *len);
} SwoftyStorage;

typedef struct SwoftyApi {
    uint32_t version;
    void (*log)(const char *fmt, ...);
    uintptr_t (*module_base)(void);
    int (*scan)(const char *section, const char *pattern, uintptr_t *out);
    uintptr_t (*resolve)(const char *id);
    int (*register_mixins)(const SwoftyMixin *mixins, size_t count);
    void *(*fields)(const void *obj, size_t size);
    int (*setting)(const char *id, uint32_t type, SwoftySettingValue *out);
    int (*watch_setting)(const char *id, SwoftySettingFn fn, void *user);
    const SwoftyAssets *assets;
    const SwoftyEvents *events;
    const SwoftyOverlay *overlay;
    const SwoftyTelemetry *telemetry;
    const SwoftyDb *db;
    const SwoftyStorage *storage;
} SwoftyApi;

typedef int (*SwoftyAddonInitFn)(const SwoftyApi *api);
typedef SwoftyAddonInitFn SwoftyModInitFn;

typedef int (*SwoftyCoreAttachFn)(SwoftyApi *api);
typedef int (*SwoftyCoreInitFn)(const wchar_t *name, SwoftyAddonInitFn init);
typedef int (*SwoftyCoreCommitFn)(void);
typedef int (*SwoftyCoreStageFn)(int stage);

#define SWOFTY_STAGE_WINDOW 1

#if defined(_WIN32)
#define SWOFTY_EXPORT __declspec(dllexport)
#else
#define SWOFTY_EXPORT __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
}
#endif
