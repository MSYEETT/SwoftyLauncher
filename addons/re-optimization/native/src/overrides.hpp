// SPDX-License-Identifier: MIT
// Copyright (c) 2026 xanpavle. Ported to a Swofty Launcher addon by Swofty.
//
// The [override] table of f125_tune.ini, split into the groups the addon's
// options switch on and off. Values parse exactly as the original ini did
// (true/on/yes, false/off/no, integers with any base, floats as raw bits).
#pragma once
#include <cstdint>

namespace reopt {

enum Group : uint32_t {
    GroupVrsAsync = 1u << 0,
    GroupGroundCover = 1u << 1,
    GroupInstanceCaps = 1u << 2,
};

struct Override {
    const char *name;
    uint32_t group;
    uint32_t u32;
    bool is_float;
};

int override_count();
const Override *override_at(int i);
const Override *find_override(const char *name, uint32_t groups);
bool lookup_bool(const char *name, uint32_t groups, uint8_t *out);
bool lookup_scalar(const char *name, uint32_t groups, uint32_t *out);
bool parse_value(const char *text, uint32_t *u32, bool *is_float);

}
