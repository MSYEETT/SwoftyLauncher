// SPDX-License-Identifier: MIT
// Copyright (c) 2026 xanpavle. Ported to a Swofty Launcher addon by Swofty.
//
// Deliberately NOT touched (as in the original profile.cpp):
//   mirrors.*, rt_* / RayTracing, weather rain*, and aa_quality. Only the backend
//   booleans of <antialiasing> change, and only when an upscaler is chosen.
#include "delta.hpp"

#include <cstring>

namespace reopt {
namespace {

struct Ov {
    const char *tag;
    const char *attr;
    const char *val;
};

const Ov kOv[] = {
    {"ssao", "algorithm", "1"},
    {"ssrt", "quality", "2"},

    {"lighting", "volumetricFogMapWidth", "512"},
    {"lighting", "volumetricFogMapHeight", "288"},

    {"postprocess", "godRays", "false"},
    {"postprocess", "lensStreak", "false"},
    {"postprocess", "motionBlurV2", "false"},

    {"shadows", "worldShadowMapSize", "2048"},
    {"shadows", "nightShadowMapSize", "2048"},
    {"shadows", "skyShadowMap32Bits", "false"},
    {"shadows", "sampling", "2"},
    {"shadows", "spotShadowSampling", "2"},
    {"shadows", "crowdInShadows", "false"},

    {"particles", "high", "false"},
    {"crowd", "billboardDistance", "2500"},

    {"vehicle_reflections", "envMapScale", "1.0"},
    {"vehicle_reflections", "cubeMapScale", "1.0"},
    {"vehicle_reflections", "cubeMapUpdateMode", "oneFacePerFrame"},

    {"weather_effects", "proceduralCloudRTScale", "2.0"},

    {"ground_cover", "drawDistance", "80.0"},
    {"hair", "enabled", "false"},
    {"skidmarks", "parallax", "false"},

    {"cs_culling", "enabled", "true"},

    {"texture_streaming", "texelDensityBias", "0.25"},
};

bool ieq(std::string_view a, std::string_view b)
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

bool find_attr(const std::string &line, std::string_view attr, size_t *vs, size_t *ve)
{
    size_t p = 0;
    while ((p = line.find(attr, p)) != std::string::npos) {
        char before = p ? line[p - 1] : '\0';
        if (p != 0 && before != ' ' && before != '\t' && before != '\r' && before != '\n' && before != '<') {
            p += attr.size();
            continue;
        }
        size_t eq = p + attr.size();
        if (eq + 1 >= line.size() || line[eq] != '=' || line[eq + 1] != '"') {
            p += attr.size();
            continue;
        }
        size_t s = eq + 2;
        size_t e = line.find('"', s);
        if (e == std::string::npos)
            return false;
        *vs = s;
        *ve = e;
        return true;
    }
    return false;
}

void replace_attr(std::string &line, const std::string &tag, std::string_view attr, std::string_view val, Record *rec)
{
    size_t vs, ve;
    if (!find_attr(line, attr, &vs, &ve))
        return;
    std::string original = line.substr(vs, ve - vs);
    if (original == val)
        return;
    line.replace(vs, ve - vs, val);
    if (rec)
        rec->changes.push_back({tag, std::string(attr), std::move(original), std::string(val)});
}

void apply_upscaler(std::string &line, const std::string &tag, Upscaler up, Record *rec)
{
    const char *taa = "false", *dlss = "false", *fsr = "false", *xess = "false";
    switch (up) {
    case Upscaler::Taa:
        taa = "true";
        break;
    case Upscaler::Dlss:
        dlss = "true";
        break;
    case Upscaler::Fsr3:
        fsr = "true";
        break;
    case Upscaler::Xess:
        xess = "true";
        break;
    case Upscaler::Off:
        break;
    case Upscaler::Keep:
        return;
    }
    replace_attr(line, tag, "taa", taa, rec);
    replace_attr(line, tag, "dlss", dlss, rec);
    replace_attr(line, tag, "fsr3", fsr, rec);
    replace_attr(line, tag, "xess", xess, rec);
}

std::string elem_tag(const std::string &line)
{
    size_t p = 0;
    while (p < line.size() && (line[p] == ' ' || line[p] == '\t'))
        p++;
    if (p >= line.size() || line[p] != '<')
        return {};
    p++;
    if (p < line.size() && (line[p] == '/' || line[p] == '?' || line[p] == '!'))
        return {};
    size_t s = p;
    while (p < line.size() && line[p] != ' ' && line[p] != '\t' && line[p] != '>' && line[p] != '/' && line[p] != '\r' &&
           line[p] != '\n' && p - s < 63)
        p++;
    return line.substr(s, p - s);
}

template <class F> std::string each_element(std::string_view in, F &&f)
{
    std::string out;
    out.reserve(in.size() + 256);
    size_t p = 0;
    while (p < in.size()) {
        size_t lt = in.find('<', p);
        if (lt == std::string_view::npos) {
            out.append(in.substr(p));
            break;
        }
        out.append(in.substr(p, lt - p));
        size_t end;
        if (in.substr(lt, 4) == "<!--") {
            end = in.find("-->", lt);
            end = end == std::string_view::npos ? in.size() : end + 3;
            out.append(in.substr(lt, end - lt));
            p = end;
            continue;
        }
        char quote = 0;
        size_t q = lt + 1;
        for (; q < in.size(); q++) {
            char c = in[q];
            if (quote) {
                if (c == quote)
                    quote = 0;
            } else if (c == '"' || c == '\'') {
                quote = c;
            } else if (c == '>') {
                break;
            }
        }
        end = q < in.size() ? q + 1 : in.size();
        std::string elem(in.substr(lt, end - lt));
        std::string tag = elem_tag(elem);
        if (!tag.empty())
            f(elem, tag);
        out += elem;
        p = end;
    }
    return out;
}

}

Upscaler parse_upscaler(std::string_view m, bool *is_auto)
{
    if (is_auto)
        *is_auto = false;
    if (ieq(m, "keep") || ieq(m, "none"))
        return Upscaler::Keep;
    if (ieq(m, "off"))
        return Upscaler::Off;
    if (ieq(m, "taa"))
        return Upscaler::Taa;
    if (ieq(m, "dlss"))
        return Upscaler::Dlss;
    if (ieq(m, "fsr3") || ieq(m, "fsr"))
        return Upscaler::Fsr3;
    if (ieq(m, "xess"))
        return Upscaler::Xess;
    if (is_auto)
        *is_auto = true;
    return Upscaler::Keep;
}

const char *upscaler_name(Upscaler up)
{
    switch (up) {
    case Upscaler::Taa:
        return "TAA";
    case Upscaler::Dlss:
        return "DLSS";
    case Upscaler::Fsr3:
        return "FSR 3";
    case Upscaler::Xess:
        return "XeSS";
    case Upscaler::Off:
        return "off (no AA)";
    default:
        return "keep (unchanged)";
    }
}

bool is_hardware_settings_config(const char *text, size_t n)
{
    static const char root[] = "<hardware_settings_config";
    const size_t rl = sizeof root - 1;
    const std::string_view all(text, n);
    size_t p = 0;
    if (n >= 3 && (unsigned char)text[0] == 0xEF && (unsigned char)text[1] == 0xBB && (unsigned char)text[2] == 0xBF)
        p = 3;
    while (p < n) {
        char c = text[p];
        if (c == ' ' || c == '\t' || c == '\r' || c == '\n') {
            p++;
            continue;
        }
        if (c != '<')
            return false;
        if (p + 1 < n && text[p + 1] == '?') {
            size_t e = all.find("?>", p);
            if (e == std::string_view::npos)
                return false;
            p = e + 2;
            continue;
        }
        if (p + 3 < n && text[p + 1] == '!' && text[p + 2] == '-' && text[p + 3] == '-') {
            size_t e = all.find("-->", p);
            if (e == std::string_view::npos)
                return false;
            p = e + 3;
            continue;
        }
        if (n - p <= rl || memcmp(text + p, root, rl) != 0)
            return false;
        char after = text[p + rl];
        return after == ' ' || after == '\t' || after == '\r' || after == '\n' || after == '>' || after == '/';
    }
    return false;
}

std::string transform(std::string_view in, Upscaler up, bool profile, Record *record)
{
    return each_element(in, [&](std::string &line, const std::string &tag) {
        if (up != Upscaler::Keep && tag == "antialiasing")
            apply_upscaler(line, tag, up, record);
        if (!profile)
            return;
        for (const Ov &o : kOv)
            if (tag == o.tag)
                replace_attr(line, tag, o.attr, o.val, record);
    });
}

std::string untransform(std::string_view in, const Record &record, size_t *restored)
{
    size_t n = 0;
    std::string out = each_element(in, [&](std::string &line, const std::string &tag) {
        for (const Change &c : record.changes) {
            if (c.tag != tag)
                continue;
            size_t vs, ve;
            if (!find_attr(line, c.attr, &vs, &ve))
                continue;
            if (std::string_view(line).substr(vs, ve - vs) != c.tuned)
                continue;
            line.replace(vs, ve - vs, c.original);
            n++;
        }
    });
    if (restored)
        *restored = n;
    return out;
}

size_t profile_size()
{
    return sizeof kOv / sizeof kOv[0];
}

}
