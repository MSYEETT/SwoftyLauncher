// SPDX-License-Identifier: MIT
// Copyright (c) 2026 xanpavle. Ported to a Swofty Launcher addon by Swofty.
#include "overrides.hpp"

#include <cstdlib>
#include <cstring>

namespace reopt {
namespace {

struct Source {
    const char *name;
    uint32_t group;
    const char *value;
};

const Source kSource[] = {
    {"Renderer.Gfx.enableVRS", GroupVrsAsync, "1"},
    {"Renderer.Gfx.enableAsyncCompute", GroupVrsAsync, "1"},

    {"Renderer.Gfx.groundCoverForceUltraQuality", GroupGroundCover, "0"},
    {"Renderer.Gfx.groundCoverMaxInstances", GroupGroundCover, "40000"},
    {"Renderer.Gfx.groundCover2MaxInstances", GroupGroundCover, "80000"},
    {"Renderer.Gfx.groundCover2MaxMeshInstances", GroupGroundCover, "4000"},

    {"Renderer.maxStaticInstances", GroupInstanceCaps, "20000"},
    {"Renderer.maxPVSRenderInstances", GroupInstanceCaps, "20000"},
};

constexpr int kCount = int(sizeof kSource / sizeof kSource[0]);

bool ieq(const char *a, const char *b)
{
    for (; *a && *b; a++, b++) {
        char x = *a, y = *b;
        if (x >= 'A' && x <= 'Z')
            x = char(x - 'A' + 'a');
        if (y >= 'A' && y <= 'Z')
            y = char(y - 'A' + 'a');
        if (x != y)
            return false;
    }
    return *a == *b;
}

struct Table {
    Override entries[kCount];
    int count = 0;

    Table()
    {
        for (const Source &s : kSource) {
            Override o{s.name, s.group, 0, false};
            if (parse_value(s.value, &o.u32, &o.is_float))
                entries[count++] = o;
        }
    }
};

const Table &table()
{
    static const Table t;
    return t;
}

}

bool parse_value(const char *v, uint32_t *u32, bool *is_float)
{
    if (ieq(v, "true") || ieq(v, "on") || ieq(v, "yes")) {
        *u32 = 1;
        *is_float = false;
        return true;
    }
    if (ieq(v, "false") || ieq(v, "off") || ieq(v, "no")) {
        *u32 = 0;
        *is_float = false;
        return true;
    }
    char *end = nullptr;
    if (strchr(v, '.') || strchr(v, 'e') || strchr(v, 'E')) {
        float f = float(strtod(v, &end));
        if (end == v)
            return false;
        memcpy(u32, &f, 4);
        *is_float = true;
        return true;
    }
    long n = strtol(v, &end, 0);
    if (end == v)
        return false;
    *u32 = uint32_t(n);
    *is_float = false;
    return true;
}

int override_count()
{
    return table().count;
}

const Override *override_at(int i)
{
    return i >= 0 && i < table().count ? &table().entries[i] : nullptr;
}

const Override *find_override(const char *name, uint32_t groups)
{
    if (!name)
        return nullptr;
    const Table &t = table();
    for (int i = 0; i < t.count; i++)
        if ((t.entries[i].group & groups) && strcmp(t.entries[i].name, name) == 0)
            return &t.entries[i];
    return nullptr;
}

bool lookup_bool(const char *name, uint32_t groups, uint8_t *out)
{
    const Override *e = find_override(name, groups);
    if (!e)
        return false;
    if (e->is_float) {
        float f;
        memcpy(&f, &e->u32, 4);
        *out = f != 0.0f ? 1 : 0;
    } else {
        *out = e->u32 != 0 ? 1 : 0;
    }
    return true;
}

bool lookup_scalar(const char *name, uint32_t groups, uint32_t *out)
{
    const Override *e = find_override(name, groups);
    if (!e)
        return false;
    *out = e->u32;
    return true;
}

}
