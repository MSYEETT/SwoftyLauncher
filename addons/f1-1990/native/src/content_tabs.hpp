// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <cstdint>
#include <cstring>
#include <vector>

#include "weekend.hpp"

namespace f1990 {

// A UI content group's entries as the tab builder reads them: begin +0x08, end +0x10, 0x30 bytes
// each, the tab name an NeStr at +0x08 (text at +0x09).
constexpr size_t kContentTabSize = 0x30;
constexpr size_t kContentTabMax = 64;

inline const char *content_tab_name(const uint8_t *entry)
{
    auto *s = at<const uint8_t *>(entry, 0x08);
    return s ? reinterpret_cast<const char *>(s + 9) : nullptr;
}

// Copies the group's entries without the named tabs; returns how many it dropped, or -1 when the
// group doesn't look like a content group.
template <size_t N>
int drop_content_tabs(const void *group, const char *const (&drop)[N], std::vector<uint8_t> &out)
{
    out.clear();
    auto *b = at<const uint8_t *>(group, 0x08), *e = at<const uint8_t *>(group, 0x10);
    if (!b || e < b || size_t(e - b) % kContentTabSize || size_t(e - b) / kContentTabSize > kContentTabMax)
        return -1;
    int dropped = 0;
    for (; b < e; b += kContentTabSize) {
        const char *name = content_tab_name(b);
        bool skip = false;
        for (const char *d : drop)
            skip |= name && !std::strcmp(name, d);
        if (skip)
            dropped++;
        else
            out.insert(out.end(), b, b + kContentTabSize);
    }
    return dropped;
}

}
