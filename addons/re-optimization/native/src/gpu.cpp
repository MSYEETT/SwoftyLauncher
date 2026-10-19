// SPDX-License-Identifier: MIT
// Copyright (c) 2026 xanpavle. Ported to a Swofty Launcher addon by Swofty.
#include "gpu.hpp"

#include <cstdlib>
#include <cstring>
#include <cwctype>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <cstdio>
#endif

namespace reopt::gpu {
namespace {

bool wcontains(const wchar_t *hay, const wchar_t *needle)
{
    if (!hay || !needle || !*needle)
        return false;
    const size_t n = wcslen(needle);
    for (const wchar_t *p = hay; *p; ++p) {
        size_t i = 0;
        while (i < n && p[i] && towlower(p[i]) == towlower(needle[i]))
            ++i;
        if (i == n)
            return true;
    }
    return false;
}

int first_model(const char *s)
{
    for (const char *p = s; *p;) {
        if (*p >= '0' && *p <= '9') {
            char *end = nullptr;
            const long v = strtol(p, &end, 10);
            if (end - p >= 3)
                return int(v);
            p = end;
        } else {
            ++p;
        }
    }
    return 0;
}

int nv_series(int n)
{
    if (n >= 5000)
        return 50;
    if (n >= 4000)
        return 40;
    if (n >= 3000)
        return 30;
    if (n >= 2000)
        return 20;
    if (n >= 1600)
        return 16;
    if (n >= 1000)
        return 10;
    return 0;
}

int amd_series(int n)
{
    if (n >= 9000)
        return 9000;
    if (n >= 7000)
        return 7000;
    if (n >= 6000)
        return 6000;
    if (n >= 5000)
        return 5000;
    if (n >= 4000)
        return 4000;
    if (n >= 300)
        return 300;
    return 0;
}

void to_narrow(const wchar_t *w, char *out, size_t cap)
{
    size_t i = 0;
    for (; w && w[i] && i + 1 < cap; i++)
        out[i] = w[i] < 0x80 ? char(w[i]) : '?';
    out[i] = 0;
}

}

Vendor vendor_from_id(const wchar_t *id)
{
    if (wcontains(id, L"VEN_10DE"))
        return VendorNvidia;
    if (wcontains(id, L"VEN_1002"))
        return VendorAmd;
    if (wcontains(id, L"VEN_8086"))
        return VendorIntel;
    return VendorUnknown;
}

Vendor vendor_from_name(const wchar_t *name)
{
    if (wcontains(name, L"nvidia") || wcontains(name, L"geforce"))
        return VendorNvidia;
    if (wcontains(name, L"amd") || wcontains(name, L"radeon"))
        return VendorAmd;
    if (wcontains(name, L"intel"))
        return VendorIntel;
    return VendorUnknown;
}

Info classify(Vendor v, const wchar_t *name)
{
    Info g;
    memset(&g, 0, sizeof g);
    g.vendor = v;
    to_narrow(name, g.name, sizeof g.name);
    const bool arc = v == VendorIntel && wcontains(name, L"arc");
    const int n = first_model(g.name);
    if (v == VendorNvidia)
        g.series = nv_series(n);
    else if (v == VendorAmd)
        g.series = amd_series(n);
    g.supports_dlss = v == VendorNvidia && g.series >= 20;
    g.supports_xess = v == VendorIntel && arc;
    return g;
}

int score(const Info &g)
{
    switch (g.vendor) {
    case VendorNvidia:
        return 30;
    case VendorAmd:
        return 28;
    case VendorIntel:
        return g.supports_xess ? 20 : 5;
    default:
        return 0;
    }
}

const char *vendor_name(Vendor v)
{
    switch (v) {
    case VendorNvidia:
        return "NVIDIA";
    case VendorAmd:
        return "AMD";
    case VendorIntel:
        return "Intel";
    default:
        return "unknown";
    }
}

Upscaler choose(std::string_view mode, const Info &g)
{
    bool is_auto = false;
    Upscaler forced = parse_upscaler(mode, &is_auto);
    if (!is_auto)
        return forced;
    switch (g.vendor) {
    case VendorNvidia:
        return g.supports_dlss ? Upscaler::Dlss : Upscaler::Fsr3;
    case VendorAmd:
        return Upscaler::Fsr3;
    case VendorIntel:
        return g.supports_xess ? Upscaler::Xess : Upscaler::Fsr3;
    default:
        return Upscaler::Keep;
    }
}

#ifdef _WIN32
Info detect()
{
    Info best;
    memset(&best, 0, sizeof best);
    int best_score = -1;

    DISPLAY_DEVICEW dd;
    for (DWORD i = 0;; ++i) {
        memset(&dd, 0, sizeof dd);
        dd.cb = sizeof dd;
        if (!EnumDisplayDevicesW(nullptr, i, &dd, 0))
            break;
        if (wcontains(dd.DeviceString, L"microsoft basic") || wcontains(dd.DeviceString, L"remote"))
            continue;
        Vendor v = vendor_from_id(dd.DeviceID);
        if (v == VendorUnknown)
            v = vendor_from_name(dd.DeviceString);
        if (v == VendorUnknown)
            continue;
        Info cand = classify(v, dd.DeviceString);
        if (score(cand) > best_score) {
            best_score = score(cand);
            best = cand;
        }
    }

    if (best_score < 0) {
        HKEY root;
        if (RegOpenKeyExW(HKEY_LOCAL_MACHINE,
                          L"SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}", 0,
                          KEY_READ, &root) == ERROR_SUCCESS) {
            wchar_t sub[16];
            for (int i = 0; i < 16; ++i) {
                _snwprintf(sub, 16, L"%04d", i);
                HKEY k;
                if (RegOpenKeyExW(root, sub, 0, KEY_READ, &k) != ERROR_SUCCESS)
                    continue;
                wchar_t desc[256];
                memset(desc, 0, sizeof desc);
                DWORD cb = sizeof desc - sizeof(wchar_t);
                DWORD type = 0;
                if (RegQueryValueExW(k, L"DriverDesc", nullptr, &type, reinterpret_cast<LPBYTE>(desc), &cb) ==
                        ERROR_SUCCESS &&
                    type == REG_SZ) {
                    const Vendor v = vendor_from_name(desc);
                    if (v != VendorUnknown) {
                        const Info cand = classify(v, desc);
                        if (score(cand) > best_score) {
                            best_score = score(cand);
                            best = cand;
                        }
                    }
                }
                RegCloseKey(k);
            }
            RegCloseKey(root);
        }
    }
    return best;
}
#endif

}
