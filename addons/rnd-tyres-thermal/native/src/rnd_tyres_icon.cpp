// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - the Tyres department's own icon frames (v0.8.0).
//
// A department icon is a slice of a BC7 texture array, named by the entry at the same index
// in an overrides.uidb list. erp/rnd_tyres_icon.json appends a slice and an entry to three arrays, in both the plain and the rnd_om_xm lists:
//   rnd_dept_icons_small.tyres        48x48 white   (box card, R&D lists, Develop Upgrade card)
//   rnd_wheel_icon.tyres              60x60 orange  (the Tyres tile on R&D > Performance)
//   rnd_department_strips.rnd_tyres_strip  1112x4 orange (In Progress list colour strip)
//
// The DLL never names a frame the game lacks: probe() reads the index of the installed
// flow_career_hub.erp and needs every one of the six arrays at its grown size. Anything
// else (a launcher that delivered no ERP ops, a stock archive) keeps the
// v0.7.2 borrowed frames.
//
// probe_art() does the same for the card art (v0.9.0): the
// rnd_part_image arrays in flow_team_hq.erp must hold exactly the 45 Tyres slices more than
// stock, the rnd_part_mask arrays 15 and the rnd_category_icons arrays in flow_career_hub.erp
// 4 (tools/build_card_art.py). Each of the three switches on its own.

#include <windows.h>

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include <swofty/mixin.hpp>

#include "rnd_tyres_icon.hpp"

namespace rnd_tyres_icon {
namespace {

bool g_live = false;
bool g_part_art = false;
bool g_part_mask = false;
bool g_category = false;

struct Want {
    const char *name;
    uint64_t size;
    bool seen;
};

constexpr const char kArrays[] = "eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.career.";

std::wstring archive_path(const wchar_t *file = L"flow_career_hub.erp")
{
    wchar_t exe[MAX_PATH * 2];
    DWORD n = GetModuleFileNameW(nullptr, exe, MAX_PATH * 2);
    if (n == 0 || n >= MAX_PATH * 2)
        return {};
    std::wstring p(exe, n);
    size_t cut = p.find_last_of(L"\\/");
    if (cut == std::wstring::npos)
        return {};
    return p.substr(0, cut + 1) + L"2025_asset_groups\\ui_package\\" + file;
}

template <class T> bool take(const std::vector<uint8_t> &b, size_t &at, T &out)
{
    if (at + sizeof(T) > b.size())
        return false;
    std::memcpy(&out, b.data() + at, sizeof(T));
    at += sizeof(T);
    return true;
}

int check(std::vector<Want> &want, char *why, size_t why_len, const wchar_t *file = L"flow_career_hub.erp")
{
    std::wstring path = archive_path(file);
    FILE *f = path.empty() ? nullptr : _wfopen(path.c_str(), L"rb");
    if (!f) {
        std::snprintf(why, why_len, "flow_career_hub.erp not readable");
        return -1;
    }
    uint8_t head[0x38];
    std::vector<uint8_t> index;
    uint64_t data_off = 0;
    uint32_t nres = 0;
    bool ok = std::fread(head, 1, sizeof head, f) == sizeof head && std::memcmp(head, "ERPK", 4) == 0;
    if (ok) {
        std::memcpy(&data_off, head + 0x20, 8);
        std::memcpy(&nres, head + 0x30, 4);
        ok = data_off > sizeof head && data_off < (64ull << 20);
    }
    if (ok) {
        index.resize(static_cast<size_t>(data_off - sizeof head));
        ok = std::fread(index.data(), 1, index.size(), f) == index.size();
    }
    std::fclose(f);
    if (!ok) {
        std::snprintf(why, why_len, "flow_career_hub.erp header or index unreadable");
        return -1;
    }
    size_t at = 0;
    int found = 0;
    for (uint32_t i = 0; i < nres; ++i) {
        uint32_t esz;
        uint16_t nlen;
        if (!take(index, at, esz) || !take(index, at, nlen) || at + nlen + 16 + 4 + 1 > index.size())
            break;
        std::string name(reinterpret_cast<const char *>(index.data() + at), nlen);
        at += nlen + 16 + 4;
        while (!name.empty() && name.back() == '\0')
            name.pop_back();
        uint8_t nfrag = index[at++];
        uint64_t sizes[4] = {};
        for (uint8_t k = 0; k < nfrag; ++k) {
            if (at + 33 > index.size())
                return found;
            uint64_t size;
            std::memcpy(&size, index.data() + at + 12, 8);
            if (k < 4)
                sizes[k] = size;
            at += 33;
        }
        at += 16;
        for (auto &w : want)
            if (!w.seen && name == w.name) {
                w.seen = true;
                if (nfrag >= 2 && sizes[1] == w.size)
                    ++found;
            }
    }
    return found;
}

} // namespace

void probe()
{
    struct Spec {
        const char *list;
        uint64_t slice;
        uint32_t stock;
    };
    static const Spec specs[] = {
        {"rnd_dept_icons_small", 48 * 48, 4},
        {"rnd_wheel_icon", 60 * 60, 6},
        {"rnd_department_strips", 1112 * 4, 4},
    };
    std::vector<std::string> names;
    std::vector<Want> want;
    for (const char *prefix : {"", "rnd_om_xm."})
        for (const auto &s : specs)
            names.push_back(std::string(kArrays) + prefix + s.list + ".texarray.image");
    size_t i = 0;
    for (int p = 0; p < 2; ++p)
        for (const auto &s : specs)
            want.push_back({names[i++].c_str(), s.slice * (s.stock + 1), false});
    char why[96] = "";
    int found = check(want, why, sizeof why);
    g_live = found == static_cast<int>(want.size());
    if (g_live) {
        swofty::log("[rnd-tyres] tyres icon LIVE: %d/%zu grown R&D icon arrays in flow_career_hub.erp; Tyres shows "
                    "rnd_dept_icons_small.tyres, rnd_wheel_icon.tyres and rnd_department_strips.rnd_tyres_strip",
                    found, want.size());
        return;
    }
    if (found < 0)
        swofty::log("[rnd-tyres] tyres icon off: %s; Tyres keeps the borrowed stock frames (SAFE)", why);
    else
        swofty::log("[rnd-tyres] tyres icon off: %d/%zu grown R&D icon arrays in flow_career_hub.erp (the icon ERP ops "
                    "not installed); Tyres keeps the borrowed stock frames (SAFE)",
                    found, want.size());
}

void probe_art()
{
    constexpr uint64_t kPart = 240 * 240, kIcon = 32 * 32;
    constexpr uint64_t kUpgrades = 15, kStates = 3, kTypes = 4;
    std::string img = std::string(kArrays) + "rnd_part_image.texarray.image";
    std::string img_om = std::string(kArrays) + "rnd_om_xm.rnd_part_image.texarray.image";
    std::string msk = std::string(kArrays) + "rnd_part_mask.texarray.image";
    std::string msk_om = std::string(kArrays) + "rnd_om_xm.rnd_part_mask.texarray.image";
    std::string cat = std::string(kArrays) + "rnd_category_icons.texarray.image";
    std::string cat_om = std::string(kArrays) + "rnd_om_xm.rnd_category_icons.texarray.image";
    char why[96] = "";
    std::vector<Want> art = {{img.c_str(), (362 + kUpgrades * kStates) * kPart, false},
                             {img_om.c_str(), (337 + kUpgrades * kStates) * kPart, false}};
    std::vector<Want> mask = {{msk.c_str(), (120 + kUpgrades) * kPart, false},
                              {msk_om.c_str(), (110 + kUpgrades) * kPart, false}};
    std::vector<Want> cats = {{cat.c_str(), (21 + kTypes) * kIcon, false},
                              {cat_om.c_str(), (20 + kTypes) * kIcon, false}};
    int a = check(art, why, sizeof why, L"flow_team_hq.erp");
    int m = check(mask, why, sizeof why, L"flow_team_hq.erp");
    int k = check(cats, why, sizeof why);
    g_part_art = a == 2;
    g_part_mask = m == 2;
    g_category = k == 2;
    swofty::log("[rnd-tyres] tyres art: part images %s (%d/2 arrays with the 45 Tyres slices in flow_team_hq.erp), "
                "part masks %s (%d/2 with 15), category icons %s (%d/2 with 4 in flow_career_hub.erp); a borrowed "
                "piece uses stock frames (SAFE)",
                g_part_art ? "OWN" : "borrowed", a, g_part_mask ? "OWN" : "borrowed", m,
                g_category ? "OWN" : "borrowed", k);
}

bool part_art_live()
{
    return g_part_art;
}

bool part_mask_live()
{
    return g_part_mask;
}

bool category_live()
{
    return g_category;
}

bool live()
{
    return g_live;
}

const char *dept(const char *borrowed)
{
    return g_live ? "tyres" : borrowed;
}

const char *wheel(const char *borrowed)
{
    return g_live ? "tyres" : borrowed;
}

const char *strip(const char *borrowed)
{
    return g_live ? "rnd_tyres_strip" : borrowed;
}

} // namespace rnd_tyres_icon
