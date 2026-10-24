#pragma once
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <string_view>
#include <vector>

#include "json.hpp"

namespace hud::guard {

constexpr uint64_t REMOVED_AFTER_DRAW_MS = 10000;
constexpr const char *REMOVED_FILE = "overlay-gpu-removed.json";

inline constexpr std::string_view FG_MODULES[] = {
    "sl.dlss_g.dll",
    "nvngx_dlssg.dll",
    "amd_fidelityfx_dx12.dll",
    "amd_fidelityfx_framegeneration_dx12.dll",
    "ffx_frameinterpolation_x64.dll",
    "ffx_fsr3_x64.dll",
    "libxess_fg.dll",
    "dlssg_to_fsr3_amd_is_better.dll",
};

inline bool same_name(std::string_view a, std::string_view b)
{
    if (a.size() != b.size())
        return false;
    for (size_t i = 0; i < a.size(); i++) {
        char x = a[i], y = b[i];
        if (x >= 'A' && x <= 'Z')
            x = char(x - 'A' + 'a');
        if (y >= 'A' && y <= 'Z')
            y = char(y - 'A' + 'a');
        if (x != y)
            return false;
    }
    return true;
}

inline std::string_view base_name(std::string_view path)
{
    size_t at = path.find_last_of("\\/");
    return at == std::string_view::npos ? path : path.substr(at + 1);
}

inline bool fg_module(std::string_view path)
{
    std::string_view name = base_name(path);
    for (auto m : FG_MODULES)
        if (same_name(name, m))
            return true;
    return false;
}

inline bool streamline_hook(std::string_view path)
{
    return same_name(base_name(path), "sl.dlss_g.dll");
}

inline std::string fg_in(const std::vector<std::string> &modules)
{
    for (auto &m : modules)
        if (fg_module(m) && !streamline_hook(m))
            return std::string(base_name(m));
    return {};
}

inline bool streamline_in(const std::vector<std::string> &modules)
{
    for (auto &m : modules)
        if (streamline_hook(m))
            return true;
    return false;
}

inline bool xml_int(const std::string &text, const char *element, const char *attr, int &out)
{
    size_t at = text.find(std::string("<") + element + " ");
    if (at == std::string::npos)
        return false;
    size_t end = text.find('>', at);
    std::string key = std::string(" ") + attr + "=\"";
    size_t k = text.find(key, at);
    if (k == std::string::npos || k > end)
        return false;
    out = atoi(text.c_str() + k + key.size());
    return true;
}

struct FgSetting {
    bool known = false;
    int mode = -1, multi = -1;
    bool on() const { return !known || mode != 0; }
};

inline FgSetting fg_setting(const std::string &hardware_settings)
{
    FgSetting f;
    f.known = xml_int(hardware_settings, "frame_gen", "mode", f.mode);
    xml_int(hardware_settings, "multi_frame_gen", "value", f.multi);
    return f;
}

inline std::string chain(const std::vector<std::string> &modules, std::string_view skip_a, std::string_view skip_b)
{
    std::string out;
    std::string_view last;
    for (auto &m : modules) {
        std::string_view name = base_name(m);
        if (name.empty() || same_name(name, skip_a) || same_name(name, skip_b) || same_name(name, last))
            continue;
        if (!out.empty())
            out += " <- ";
        out += name;
        last = name;
    }
    return out.empty() ? "unknown" : out;
}

inline bool removal_after_draw(uint64_t now_ms, uint64_t last_draw_ms)
{
    return last_draw_ms && now_ms >= last_draw_ms && now_ms - last_draw_ms <= REMOVED_AFTER_DRAW_MS;
}

struct Removal {
    std::string when;
    uint32_t present_hr = 0, reason = 0;
    uint64_t after_draw_ms = 0;
    std::string swapchain_by;
};

inline std::string hex32(uint32_t v)
{
    char b[16];
    snprintf(b, sizeof b, "0x%08x", unsigned(v));
    return b;
}

inline std::string json_str(std::string_view s)
{
    std::string out = "\"";
    for (char c : s) {
        if (c == '"' || c == '\\')
            out += '\\', out += c;
        else if (static_cast<unsigned char>(c) < 0x20)
            out += ' ';
        else
            out += c;
    }
    return out + "\"";
}

inline std::string removal_json(const Removal &r)
{
    return "{\n  \"schema\": 1,\n  \"when\": " + json_str(r.when) + ",\n  \"present_hr\": " +
           json_str(hex32(r.present_hr)) + ",\n  \"removed_reason\": " + json_str(hex32(r.reason)) +
           ",\n  \"after_draw_ms\": " + std::to_string(r.after_draw_ms) + ",\n  \"swapchain_by\": " +
           json_str(r.swapchain_by) + "\n}\n";
}

inline bool parse_removal(const std::string &text, Removal &out)
{
    json::Value v;
    if (!json::parse(text, v) || v.type != json::Value::Obj || v["schema"].type != json::Value::Num)
        return false;
    auto hex = [](const json::Value &x) { return x.type == json::Value::Str ? uint32_t(strtoul(x.s.c_str(), nullptr, 16)) : 0u; };
    out.when = v["when"].type == json::Value::Str ? v["when"].s : "";
    out.present_hr = hex(v["present_hr"]);
    out.reason = hex(v["removed_reason"]);
    out.after_draw_ms = v["after_draw_ms"].type == json::Value::Num ? uint64_t(v["after_draw_ms"].n) : 0;
    out.swapchain_by = v["swapchain_by"].type == json::Value::Str ? v["swapchain_by"].s : "";
    return true;
}

}
