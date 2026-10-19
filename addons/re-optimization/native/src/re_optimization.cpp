// SPDX-License-Identifier: MIT
// Copyright (c) 2026 xanpavle. Ported to a Swofty Launcher addon by Swofty.
//
// F1 25 RE-Optimization as a Swofty Launcher engine plugin.
// Original: https://github.com/xanpavle/F1-25-RE-Optimization
//
// Engine overrides: wraps Ne_Get_SettingBool / Ne_Get_SettingU32 (the generic
// setting getters at 0x143a43b90 / 0x143a43240 on build d873a1c8), lets the
// original run, then overwrites *out for the names of the enabled groups.
//
// Graphics profile: redirects the two calls that parse hardware_settings_config.xml
// (site_Ne_HwXmlParseCall, site_Ne_HwConfigParseCall) and hands the parser the
// tuned text instead of the file's text, so the engine reads the delta and the
// file on disk is never written by this plugin. The fwrite inside the engine's
// own settings save (site_Ne_HwXmlWriteCall) puts the player's original values
// back for every attribute that still holds a tuned value, so a save never
// stores the delta either. Every target is a signature binding; a binding that
// does not resolve leaves its feature off and logs SAFE.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <atomic>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include <swofty/mixin.hpp>

#include "delta.hpp"
#include "gpu.hpp"
#include "overrides.hpp"

#define REOPT_VERSION "0.1.0"

SWOFTY_SETTING(bool, apply_profile, true);
SWOFTY_SETTING(std::string_view, upscaler, "auto");
SWOFTY_SETTING(bool, override_vrs_async, true);
SWOFTY_SETTING(bool, override_ground_cover, true);
SWOFTY_SETTING(bool, override_instance_caps, true);
SWOFTY_SETTING(bool, log_reads, false);

namespace {

struct MemStream {
    void *vtable;
    char *data;
    size_t size;
    size_t pos;
};

struct Reader {
    void *vtable;
    MemStream *stream;
};

using ParseFn = int (*)(void *, Reader *, uint8_t, uint8_t);
using WriteFn = size_t (*)(const void *, size_t, size_t, void *);
using BoolGetFn = uint64_t (*)(void *, const char *, uint8_t *, uint8_t);
using U32GetFn = uint64_t (*)(void *, const char *, uint32_t *, uint32_t);

BoolGetFn orig_bool;
U32GetFn orig_u32;
ParseFn orig_parse_mount;
ParseFn orig_parse_config;
WriteFn orig_write;

std::atomic<uint32_t> g_groups{0};
std::atomic<bool> g_log_reads{false};

SRWLOCK g_lock = SRWLOCK_INIT;
reopt::Record g_record;
bool g_have_record;

struct Seen {
    const void *node;
    char name[96];
    uint32_t last;
};
constexpr int kMaxSeen = 16384;
Seen g_seen[kMaxSeen];
int g_seen_count;
SRWLOCK g_seen_lock = SRWLOCK_INIT;

constexpr size_t kMaxText = 8u << 20;

bool readable(const void *p, size_t n)
{
    MEMORY_BASIC_INFORMATION mbi;
    auto a = reinterpret_cast<uintptr_t>(p);
    while (n) {
        if (!VirtualQuery(reinterpret_cast<const void *>(a), &mbi, sizeof mbi) || mbi.State != MEM_COMMIT ||
            (mbi.Protect & (PAGE_NOACCESS | PAGE_GUARD)))
            return false;
        uintptr_t end = reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
        size_t chunk = end - a < n ? end - a : n;
        a += chunk;
        n -= chunk;
    }
    return true;
}

uint32_t groups_from_settings()
{
    uint32_t g = 0;
    if (override_vrs_async)
        g |= reopt::GroupVrsAsync;
    if (override_ground_cover)
        g |= reopt::GroupGroundCover;
    if (override_instance_caps)
        g |= reopt::GroupInstanceCaps;
    return g;
}

void log_read(const void *node, const char *name, uint32_t raw, bool is_bool)
{
    if (!name || !g_log_reads.load(std::memory_order_relaxed))
        return;
    bool changed = false;
    AcquireSRWLockExclusive(&g_seen_lock);
    int i = 0;
    for (; i < g_seen_count; ++i)
        if (g_seen[i].node == node && strcmp(g_seen[i].name, name) == 0)
            break;
    if (i == g_seen_count) {
        if (g_seen_count < kMaxSeen) {
            g_seen[i].node = node;
            strncpy(g_seen[i].name, name, sizeof g_seen[i].name - 1);
            g_seen[i].name[sizeof g_seen[i].name - 1] = 0;
            g_seen[i].last = raw;
            ++g_seen_count;
            changed = true;
        }
    } else if (g_seen[i].last != raw) {
        g_seen[i].last = raw;
        changed = true;
    }
    ReleaseSRWLockExclusive(&g_seen_lock);
    if (!changed)
        return;
    if (is_bool) {
        swofty::log("[re-opt] read [%p] %s = %u", node, name, raw != 0 ? 1u : 0u);
    } else {
        float f;
        memcpy(&f, &raw, 4);
        swofty::log("[re-opt] read [%p] %s = %d  (%g)", node, name, int(raw), double(f));
    }
}

uint64_t get_bool(void *node, const char *name, uint8_t *out, uint8_t def)
{
    uint64_t r = orig_bool(node, name, out, def);
    if (out) {
        uint8_t v;
        if (reopt::lookup_bool(name, g_groups.load(std::memory_order_relaxed), &v))
            *out = v;
        log_read(node, name, *out ? 1u : 0u, true);
    }
    return r;
}

uint64_t get_u32(void *node, const char *name, uint32_t *out, uint32_t def)
{
    uint64_t r = orig_u32(node, name, out, def);
    if (out) {
        uint32_t v;
        if (reopt::lookup_scalar(name, g_groups.load(std::memory_order_relaxed), &v))
            *out = v;
        log_read(node, name, *out, false);
    }
    return r;
}

reopt::Upscaler pick_upscaler()
{
    std::string mode(upscaler.get());
    bool is_auto = false;
    reopt::Upscaler forced = reopt::parse_upscaler(mode, &is_auto);
    if (!is_auto) {
        swofty::log("[re-opt] upscaler: mode=%s -> %s", mode.c_str(), reopt::upscaler_name(forced));
        return forced;
    }
    const reopt::gpu::Info g = reopt::gpu::detect();
    if (g.vendor != reopt::gpu::VendorUnknown)
        swofty::log("[re-opt] GPU: %s (%s, series %d, DLSS %s, XeSS %s)", g.name, reopt::gpu::vendor_name(g.vendor),
                    g.series, g.supports_dlss ? "yes" : "no", g.supports_xess ? "yes" : "no");
    else
        swofty::log("[re-opt] GPU: not detected; leaving the in-game upscaler unchanged");
    reopt::Upscaler up = reopt::gpu::choose(mode, g);
    swofty::log("[re-opt] upscaler: mode=%s -> %s", mode.c_str(), reopt::upscaler_name(up));
    return up;
}

int parse_tuned(ParseFn orig, const char *site, void *doc, Reader *reader, uint8_t a, uint8_t b)
{
    MemStream *ms = nullptr;
    if (reader && readable(reader, sizeof *reader) && readable(reader->stream, sizeof(MemStream)))
        ms = reader->stream;
    if (!ms || ms->pos != 0 || !ms->data || ms->size == 0 || ms->size > kMaxText || !readable(ms->data, ms->size) ||
        !reopt::is_hardware_settings_config(ms->data, ms->size))
        return orig(doc, reader, a, b);

    const bool profile = apply_profile;
    const reopt::Upscaler up = pick_upscaler();
    if (!profile && up == reopt::Upscaler::Keep) {
        swofty::log("[re-opt] %s: hardware_settings_config read; profile off and upscaler keep, left as read", site);
        return orig(doc, reader, a, b);
    }

    reopt::Record rec;
    std::string tuned = reopt::transform(std::string_view(ms->data, ms->size), up, profile, &rec);
    char *saved_data = ms->data;
    size_t saved_size = ms->size;
    ms->data = tuned.data();
    ms->size = tuned.size();
    int r = orig(doc, reader, a, b);
    ms->data = saved_data;
    ms->size = saved_size;

    AcquireSRWLockExclusive(&g_lock);
    g_record = rec;
    g_have_record = !rec.empty();
    ReleaseSRWLockExclusive(&g_lock);
    swofty::log("[re-opt] %s: hardware_settings_config parsed with the delta in memory: %zu attribute(s) changed "
                "(profile %s, upscaler %s), parse returned %d; the file on disk is unchanged",
                site, rec.changes.size(), profile ? "on" : "off", reopt::upscaler_name(up), r);
    return r;
}

int parse_mount(void *doc, Reader *reader, uint8_t a, uint8_t b)
{
    return parse_tuned(orig_parse_mount, "mount load", doc, reader, a, b);
}

int parse_config(void *doc, Reader *reader, uint8_t a, uint8_t b)
{
    return parse_tuned(orig_parse_config, "config load", doc, reader, a, b);
}

size_t write_untuned(const void *buf, size_t size, size_t count, void *file)
{
    if (size != 1 || !buf || count == 0 || count > kMaxText ||
        !reopt::is_hardware_settings_config(static_cast<const char *>(buf), count))
        return orig_write(buf, size, count, file);
    AcquireSRWLockShared(&g_lock);
    reopt::Record rec = g_have_record ? g_record : reopt::Record{};
    ReleaseSRWLockShared(&g_lock);
    if (rec.empty())
        return orig_write(buf, size, count, file);
    size_t restored = 0;
    std::string text = reopt::untransform(std::string_view(static_cast<const char *>(buf), count), rec, &restored);
    size_t wrote = orig_write(text.data(), 1, text.size(), file);
    swofty::log("[re-opt] engine saved hardware_settings_config: %zu tuned value(s) written back as your own; %s",
                restored, wrote == text.size() ? "ok" : "short write");
    return wrote == text.size() ? count : 0;
}

bool have(const SwoftyApi *a, const char *id, const char *what)
{
    if (a->resolve(id))
        return true;
    swofty::log("[re-opt] %s: binding %s does not resolve on this build; NOT hooked (SAFE)", what, id);
    return false;
}

}

SWOFTY_ON_CHANGE(override_vrs_async, bool)
{
    g_groups = groups_from_settings();
}

SWOFTY_ON_CHANGE(override_ground_cover, bool)
{
    g_groups = groups_from_settings();
}

SWOFTY_ON_CHANGE(override_instance_caps, bool)
{
    g_groups = groups_from_settings();
}

SWOFTY_ON_CHANGE(log_reads, bool on)
{
    g_log_reads = on;
}

extern "C" __declspec(dllexport) const uint32_t SwoftyAddonApi = SWOFTY_API_VERSION;

extern "C" __declspec(dllexport) int SwoftyAddonInit(const SwoftyApi *a)
{
    if (!a || a->version < 4 || !a->resolve || !a->register_mixins) {
        if (a && a->log)
            a->log("[re-opt] needs the swofty_core mixin API (v4), loader offers v%u; nothing installed (SAFE)",
                   a ? a->version : 0);
        return 0;
    }
    swofty::api = a;
    swofty::log("[re-opt] F1 25 RE-Optimization " REOPT_VERSION
                " by xanpavle (https://github.com/xanpavle/F1-25-RE-Optimization), offline single-player only");

    std::vector<SwoftyMixin> list;
    if (have(a, "Ne_Get_SettingBool", "engine overrides (bool getter)"))
        list.push_back({"Ne_Get_SettingBool", SWOFTY_MIXIN_WRAP, 0, reinterpret_cast<void *>(&get_bool),
                        reinterpret_cast<void **>(&orig_bool)});
    if (have(a, "Ne_Get_SettingU32", "engine overrides (32-bit getter)"))
        list.push_back({"Ne_Get_SettingU32", SWOFTY_MIXIN_WRAP, 0, reinterpret_cast<void *>(&get_u32),
                        reinterpret_cast<void **>(&orig_u32)});

    const bool writer = have(a, "site_Ne_HwXmlWriteCall", "graphics profile (settings save)");
    const bool mount = have(a, "site_Ne_HwXmlParseCall", "graphics profile (settings load)");
    const bool config = have(a, "site_Ne_HwConfigParseCall", "graphics profile (settings load)");
    if (writer && (mount || config)) {
        list.push_back({"site_Ne_HwXmlWriteCall", SWOFTY_MIXIN_REDIRECT, 0, reinterpret_cast<void *>(&write_untuned),
                        reinterpret_cast<void **>(&orig_write)});
        if (mount)
            list.push_back({"site_Ne_HwXmlParseCall", SWOFTY_MIXIN_REDIRECT, 0, reinterpret_cast<void *>(&parse_mount),
                            reinterpret_cast<void **>(&orig_parse_mount)});
        if (config)
            list.push_back({"site_Ne_HwConfigParseCall", SWOFTY_MIXIN_REDIRECT, 0,
                            reinterpret_cast<void *>(&parse_config), reinterpret_cast<void **>(&orig_parse_config)});
    } else {
        swofty::log("[re-opt] graphics profile and upscaler: NOT installed (SAFE); without both a load and the save "
                    "site the settings file could not be kept as the player wrote it");
    }

    if (list.empty()) {
        swofty::log("[re-opt] no binding resolves on this build; nothing installed (SAFE)");
        return 0;
    }
    if (!a->register_mixins(list.data(), list.size())) {
        swofty::log("[re-opt] register_mixins refused the %zu mixin(s); nothing installed (SAFE)", list.size());
        return 0;
    }
    swofty::detail::init_settings(a);
    g_groups = groups_from_settings();
    g_log_reads = log_reads;
    swofty::log("[re-opt] registered %zu mixin(s); profile %s, upscaler %.*s, override groups 0x%x, read log %s",
                list.size(), apply_profile ? "on" : "off", int(upscaler.get().size()), upscaler.get().data(),
                unsigned(g_groups.load()), g_log_reads.load() ? "on" : "off");
    return 1;
}
