// SPDX-License-Identifier: MIT
// Copyright (c) 2026 xanpavle. Ported to a Swofty Launcher addon by Swofty.
//
// GPU detection (vendor, series, upscaler support) for upscaler = auto.
// classify() and choose() are pure; detect() uses EnumDisplayDevicesW with the
// display-adapter registry class as a fallback (no DXGI, no WMI).
#pragma once
#include "delta.hpp"

namespace reopt::gpu {

enum Vendor { VendorUnknown = 0, VendorNvidia, VendorAmd, VendorIntel };

struct Info {
    Vendor vendor;
    int series;
    char name[128];
    bool supports_dlss;
    bool supports_xess;
};

Vendor vendor_from_id(const wchar_t *id);
Vendor vendor_from_name(const wchar_t *name);
Info classify(Vendor v, const wchar_t *name);
int score(const Info &g);
const char *vendor_name(Vendor v);
Upscaler choose(std::string_view mode, const Info &g);

#ifdef _WIN32
Info detect();
#endif

}
