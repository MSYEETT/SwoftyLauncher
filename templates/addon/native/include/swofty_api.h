/* SPDX-License-Identifier: MIT */
#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SWOFTY_API_VERSION 3

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
