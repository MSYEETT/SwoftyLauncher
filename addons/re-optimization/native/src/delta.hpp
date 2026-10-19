// SPDX-License-Identifier: MIT
// Copyright (c) 2026 xanpavle. Ported to a Swofty Launcher addon by Swofty.
//
// The tuned attribute delta for hardware_settings_config.xml (src/profile.cpp of
// F1-25-RE-Optimization), plus the reverse step that puts the player's own values
// back into any copy of the file the engine writes. Pure code, no OS dependency.
#pragma once
#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

namespace reopt {

enum class Upscaler { Keep = 0, Taa, Dlss, Fsr3, Xess, Off };

Upscaler parse_upscaler(std::string_view mode, bool *is_auto = nullptr);
const char *upscaler_name(Upscaler up);

struct Change {
    std::string tag;
    std::string attr;
    std::string original;
    std::string tuned;
};

struct Record {
    std::vector<Change> changes;
    bool empty() const { return changes.empty(); }
};

bool is_hardware_settings_config(const char *text, size_t n);

std::string transform(std::string_view in, Upscaler up, bool profile, Record *record = nullptr);

std::string untransform(std::string_view in, const Record &record, size_t *restored = nullptr);

size_t profile_size();

}
