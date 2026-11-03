#pragma once

#include "json.hpp"

#include <algorithm>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <string>
#include <string_view>
#include <vector>
#include <type_traits>

// The parts of the post-process passes that are plain logic: which views a pass runs in, the key
// names a toggle option holds, texture formats and sizes, and option values from settings text.
namespace postfx {

enum View : uint32_t { UNKNOWN = 0, COCKPIT, TCAM, TCAM_OFFSET, CHASE_NEAR, CHASE_FAR, OTHER, VIEWS };
enum Flag : uint32_t { PAUSED = 1, REPLAY = 2, PHOTO = 4, MENU = 8, CUTSCENE = 16, SPECTATING = 32 };

inline constexpr std::string_view VIEW_NAMES[VIEWS] = {"unknown",    "cockpit",   "tcam", "tcam_offset",
                                                      "chase_near", "chase_far", "other"};
inline constexpr std::string_view FLAG_NAMES[] = {"paused", "replay", "photo", "menu", "cutscene", "spectating"};

inline uint32_t view_bit(std::string_view name)
{
    for (uint32_t i = 1; i < VIEWS; i++)
        if (VIEW_NAMES[i] == name)
            return 1u << i;
    return 0;
}

inline uint32_t flag_bit(std::string_view name)
{
    for (uint32_t i = 0; i < std::size(FLAG_NAMES); i++)
        if (FLAG_NAMES[i] == name)
            return 1u << i;
    return 0;
}

inline std::string flag_text(uint32_t flags)
{
    std::string out;
    for (uint32_t i = 0; i < std::size(FLAG_NAMES); i++)
        if (flags & (1u << i))
            out += (out.empty() ? "" : "+") + std::string(FLAG_NAMES[i]);
    return out.empty() ? "-" : out;
}

inline std::string_view view_text(uint32_t kind)
{
    return kind < VIEWS ? VIEW_NAMES[kind] : "?";
}

struct When {
    uint32_t views = 0, skip = 0;
};

// A pass with views runs only while the view is known and one of them; skip flags stop it while the
// view is known to have them. Without either it always runs.
inline const char *ineligible(const When &w, uint32_t kind, uint32_t flags, bool valid)
{
    if (w.views && !valid)
        return "the view is unknown";
    if (w.views && !(w.views & (1u << kind)))
        return "not in one of its views";
    if (valid && (w.skip & flags))
        return "a skipped state";
    return nullptr;
}

// Win32 virtual-key codes for a toggle option's key names; 0 for None and unknown names.
inline int key_code(std::string_view key)
{
    if (key.size() == 1 && ((key[0] >= 'A' && key[0] <= 'Z') || (key[0] >= '0' && key[0] <= '9')))
        return key[0];
    if (key.size() == 1 && key[0] >= 'a' && key[0] <= 'z')
        return key[0] - 'a' + 'A';
    if (key.size() >= 2 && key.size() <= 3 && (key[0] == 'F' || key[0] == 'f')) {
        int n = 0;
        for (char c : key.substr(1)) {
            if (c < '0' || c > '9')
                return 0;
            n = n * 10 + (c - '0');
        }
        return n >= 1 && n <= 24 ? 0x70 + n - 1 : 0;
    }
    struct Named {
        std::string_view name;
        int vk;
    };
    static constexpr Named NAMED[] = {{"Insert", 0x2D}, {"Home", 0x24},      {"End", 0x23},
                                      {"Page Up", 0x21}, {"Page Down", 0x22}, {"Scroll Lock", 0x91},
                                      {"Pause", 0x13}};
    for (auto &n : NAMED)
        if (n.name == key)
            return n.vk;
    return 0;
}

// DXGI_FORMAT values for the texture format names.
inline int format_of(std::string_view name)
{
    struct Named {
        std::string_view name;
        int dxgi;
    };
    static constexpr Named NAMED[] = {{"rgba8", 28}, {"rgba16f", 10}, {"rg16f", 34},      {"r16f", 54},
                                      {"r32f", 41},  {"rgb10a2", 24}, {"r11g11b10f", 26}};
    for (auto &n : NAMED)
        if (n.name == name)
            return n.dxgi;
    return 0;
}

struct Size {
    uint32_t w = 0, h = 0;
};

// A texture's size for a picture: its own width and height, or the picture's scaled, at least 1x1.
inline Size texture_size(double scale, uint32_t width, uint32_t height, uint32_t pic_w, uint32_t pic_h)
{
    if (width && height)
        return {width, height};
    auto at = [&](uint32_t v) {
        double s = v * scale + 0.5;
        return s < 1 ? 1u : uint32_t(s);
    };
    return {at(pic_w), at(pic_h)};
}

// An option's value from the text of its current state in the settings: true and false read as 1 and
// 0, numbers as themselves; anything else is not a value.
inline bool option_value(std::string_view text, float &out)
{
    if (text == "true" || text == "false") {
        out = text == "true";
        return true;
    }
    std::string s(text);
    char *end = nullptr;
    double v = std::strtod(s.c_str(), &end);
    if (s.empty() || end != s.c_str() + s.size())
        return false;
    out = float(v);
    return true;
}

inline uint32_t groups(uint32_t size, uint32_t threads)
{
    return threads ? (size + threads - 1) / threads : 0;
}

// A frame count far enough back that the passes count as not run lately.
inline constexpr unsigned LONG_AGO = 0u - 8;

// Whether the passes ran on a back buffer at frame at, seen at frame now, within this use of it. A back
// buffer comes round again two or more frames later.
inline bool placed_lately(unsigned now, unsigned at)
{
    return now - at < 2;
}

// A view a slot wrote into one entry of its descriptor tables: the resource and its format.
template <class R, class F> struct Written {
    R *r = nullptr;
    F f{};
    bool set = false;
};

// Whether the entry holding v must be written again for r in format f, noting it as written when it
// must: a frame whose views are the ones its slot wrote last time keeps them.
template <class R, class F> bool rewrite(Written<R, F> &v, R *r, F f)
{
    if (v.set && v.r == r && v.f == f)
        return false;
    v = {r, f, true};
    return true;
}

// The depth and UI barriers one list moved since the locked path last looked at it, written without a
// lock by the thread recording the list.
template <class L, class R, class S> struct SideMark {
    std::atomic<L *> list{nullptr};
    std::atomic<R *> depth{nullptr}, bound{nullptr};
    std::atomic<S> depth_state{};
    std::atomic<bool> ui{false}, marked{false};
};

// The lists' side marks. A list keeps its entry once it has one; a list that finds no room makes the
// table full for good, and from then on every batch goes through the lock.
template <class L, class R, class S, size_t N = 1024, size_t PROBES = 32> class SideMarks {
    SideMark<L, R, S> at_[N];
    std::atomic<bool> full_{false};

public:
    SideMark<L, R, S> *find(L *list, bool add)
    {
        size_t h = size_t((uint64_t(uintptr_t(list)) >> 4) * 0x9E3779B97F4A7C15ull >> 32);
        for (size_t i = 0; i < PROBES && i < N; i++) {
            SideMark<L, R, S> &s = at_[(h + i) % N];
            L *k = s.list.load(std::memory_order_acquire);
            if (k == list)
                return &s;
            if (k)
                continue;
            if (!add)
                return nullptr;
            if (s.list.compare_exchange_strong(k, list, std::memory_order_acq_rel) || k == list)
                return &s;
        }
        if (add)
            full_.store(true, std::memory_order_relaxed);
        return nullptr;
    }

    bool full() const { return full_.load(std::memory_order_relaxed); }
};

// Notes one barrier of a batch in a list's side mark the way the locked path notes it in the list's
// Mark: a depth barrier always, a UI barrier once the list has a Mark or a depth barrier.
template <class L, class R, class S>
void note(SideMark<L, R, S> &s, bool &marked, bool depth, bool ui, R *r, S after)
{
    if (depth) {
        s.depth_state.store(after, std::memory_order_relaxed);
        s.depth.store(r, std::memory_order_relaxed);
        marked = true;
    }
    if (ui && marked)
        s.ui.store(true, std::memory_order_relaxed);
}

// Makes at point to r with a reference of its own on r and lets go of the one it held, so a resource
// the game releases stays alive while the core still points at it.
template <class T> void hold(T *&at, std::type_identity_t<T> *r)
{
    if (at == r)
        return;
    if (r)
        r->AddRef();
    if (at)
        at->Release();
    at = r;
}


inline constexpr int IN_NONE = -1, IN_ORIGINAL = -2, IN_PREVIOUS = -3;
inline constexpr size_t MAX_INPUTS = 8, SLOTS = 32;

// Root constant indices of swofty_options and swofty_params in the SwoftyPostFx cbuffer.
inline constexpr size_t OPTIONS_AT = 8, PARAMS_AT = 36, CONSTANT_COUNT = 56;

struct TexDecl {
    std::string name, file;
    double scale = 1;
    uint32_t width = 0, height = 0;
    int format = 10;
    bool persistent = false;

    bool operator==(const TexDecl &o) const
    {
        return name == o.name && file == o.file && scale == o.scale && width == o.width && height == o.height &&
               format == o.format && persistent == o.persistent;
    }
};

struct PassDecl {
    std::string name, dxil, output;
    std::vector<std::string> inputs;
    uint32_t threads[2] = {8, 8};
    bool raw = false;
    double budget_ms = 1.5;
};

struct EffectDecl {
    std::string id, name, addon;
    unsigned slots = 0;
    std::vector<TexDecl> textures;
    std::vector<PassDecl> passes;
};

// The effects of swofty/shaders/effects.json by id; an entry without an id or without passes is left out.
inline std::vector<EffectDecl> parse_effects(const std::string &text)
{
    std::vector<EffectDecl> out;
    json::Value root;
    if (!json::parse(text, root))
        return out;
    for (auto &e : root["effects"].a) {
        EffectDecl d;
        d.id = e["id"].str();
        d.name = e["name"].str(d.id.c_str());
        d.addon = e["addon"].str();
        d.slots = unsigned(e["slots"].num());
        for (auto &t : e["textures"].a) {
            TexDecl x;
            x.name = t["name"].str();
            x.file = t["file"].str();
            if (t["scale"].type == json::Value::Num)
                x.scale = t["scale"].n;
            if (t["width"].type == json::Value::Num && t["height"].type == json::Value::Num) {
                x.width = uint32_t(t["width"].n);
                x.height = uint32_t(t["height"].n);
            }
            x.persistent = t["persistent"].type == json::Value::Bool && t["persistent"].b;
            int f = format_of(t["format"].str("rgba16f"));
            x.format = f ? f : 10;
            d.textures.push_back(std::move(x));
        }
        for (auto &p : e["passes"].a) {
            PassDecl x;
            x.name = p["name"].str("pass");
            x.dxil = p["dxil"].str();
            x.output = p["output"].str();
            for (auto &in : p["inputs"].a)
                x.inputs.push_back(in.str());
            const auto &th = p["threads"].a;
            if (th.size() == 2 && th[0].n >= 1 && th[1].n >= 1) {
                x.threads[0] = uint32_t(th[0].n);
                x.threads[1] = uint32_t(th[1].n);
            }
            x.raw = p["raw"].type == json::Value::Bool && p["raw"].b;
            if (p["budget_ms"].type == json::Value::Num && p["budget_ms"].n > 0)
                x.budget_ms = p["budget_ms"].n;
            d.passes.push_back(std::move(x));
        }
        if (!d.id.empty() && !d.passes.empty())
            out.push_back(std::move(d));
    }
    return out;
}

struct InstanceSpec {
    std::string effect, label;
    std::vector<float> params;
};

// The config swofty/shaders/active.json says runs now.
struct ActiveSpec {
    uint64_t serial = 0;
    std::string name, source, forced_by, toggle_key;
    std::vector<InstanceSpec> instances;
};

inline bool parse_active(const std::string &text, ActiveSpec &out)
{
    json::Value root;
    if (!json::parse(text, root) || root.type != json::Value::Obj)
        return false;
    out = {};
    out.serial = uint64_t(root["serial"].num());
    out.name = root["name"].str();
    out.source = root["source"].str();
    out.forced_by = root["forced_by"].str();
    out.toggle_key = root["toggle_key"].str();
    for (auto &i : root["instances"].a) {
        InstanceSpec s;
        s.effect = i["effect"].str();
        s.label = i["label"].str();
        for (auto &v : i["params"].a)
            s.params.push_back(v.type == json::Value::Num ? float(v.n) : 0.0f);
        if (!s.effect.empty())
            out.instances.push_back(std::move(s));
    }
    return true;
}

// An instance's params as the 32 option slots: missing ones are 0, extra ones are dropped.
inline void pack_slots(const std::vector<float> &params, float out[SLOTS])
{
    for (size_t i = 0; i < SLOTS; i++)
        out[i] = i < params.size() ? params[i] : 0.0f;
}

// One texture of one instance. Its name is "<instance>/<texture>".
struct PlanTex {
    int instance = 0;
    std::string effect, name;
    TexDecl decl;
};

// One pass of one instance. Inputs and output index the plan's textures, or are IN_ORIGINAL /
// IN_PREVIOUS / IN_NONE (an output of IN_NONE writes the picture).
struct PlanPass {
    int instance = 0;
    const PassDecl *decl = nullptr;
    int inputs[MAX_INPUTS] = {IN_NONE, IN_NONE, IN_NONE, IN_NONE, IN_NONE, IN_NONE, IN_NONE, IN_NONE};
    size_t n_inputs = 0;
    int output = IN_NONE;
};

struct PlanInstance {
    std::string effect, label;
    const EffectDecl *decl = nullptr;
    float options[SLOTS] = {};
};

struct Plan {
    std::vector<PlanInstance> instances;
    std::vector<PlanTex> textures;
    std::vector<PlanPass> passes;
    std::vector<std::string> dropped;

    bool uses_previous() const
    {
        for (auto &p : passes)
            for (size_t i = 0; i < p.n_inputs; i++)
                if (p.inputs[i] == IN_PREVIOUS)
                    return true;
        return false;
    }
};

inline std::string instance_name(const std::string &effect, const std::string &label)
{
    return label.empty() ? effect : effect + "." + label;
}

// The instance plan for an active config. An instance whose effect is unknown, or has a pass that names
// an input that is not there, reads its own output or writes a texture from a file, is dropped with the
// reason in dropped; the rest keep their order. Instance numbers are positions among the kept instances.
inline Plan make_plan(const ActiveSpec &active, const std::vector<EffectDecl> &effects)
{
    Plan plan;
    for (size_t n = 0; n < active.instances.size(); n++) {
        const InstanceSpec &spec = active.instances[n];
        std::string who = instance_name(spec.effect, spec.label);
        const EffectDecl *fx = nullptr;
        for (auto &e : effects)
            if (e.id == spec.effect) {
                fx = &e;
                break;
            }
        if (!fx) {
            plan.dropped.push_back(who + ": no such effect");
            continue;
        }
        size_t tex_base = plan.textures.size(), pass_base = plan.passes.size();
        const char *bad = nullptr;
        auto texture_named = [&](const std::string &name) {
            if (name == "original")
                return IN_ORIGINAL;
            if (name == "previous")
                return IN_PREVIOUS;
            for (size_t i = tex_base; i < plan.textures.size(); i++)
                if (plan.textures[i].decl.name == name)
                    return int(i);
            return IN_NONE;
        };
        for (auto &t : fx->textures) {
            PlanTex x;
            x.effect = fx->id;
            x.decl = t;
            plan.textures.push_back(std::move(x));
        }
        for (auto &pd : fx->passes) {
            PlanPass p;
            p.decl = &pd;
            for (auto &in : pd.inputs) {
                int t = texture_named(in);
                if (t == IN_NONE || p.n_inputs >= MAX_INPUTS)
                    bad = "a pass names an input that is not there";
                else
                    p.inputs[p.n_inputs++] = t;
            }
            if (!pd.output.empty()) {
                p.output = texture_named(pd.output);
                if (p.output < 0 || !plan.textures[size_t(p.output)].decl.file.empty())
                    bad = "a pass writes a texture that is not there or comes from a file";
                for (size_t i = 0; i < p.n_inputs; i++)
                    if (p.output >= 0 && p.inputs[i] == p.output)
                        bad = "a pass reads the texture it writes";
            }
            plan.passes.push_back(p);
        }
        if (fx->slots > SLOTS)
            bad = "it needs more than 32 option slots";
        if (bad) {
            plan.dropped.push_back(who + ": " + bad);
            plan.textures.resize(tex_base);
            plan.passes.resize(pass_base);
            continue;
        }
        PlanInstance inst;
        inst.effect = spec.effect;
        inst.label = spec.label;
        inst.decl = fx;
        pack_slots(spec.params, inst.options);
        plan.instances.push_back(std::move(inst));
        for (size_t i = pass_base; i < plan.passes.size(); i++)
            plan.passes[i].instance = int(plan.instances.size() - 1);
        for (size_t i = tex_base; i < plan.textures.size(); i++) {
            plan.textures[i].instance = int(plan.instances.size() - 1);
            plan.textures[i].name = std::to_string(plan.instances.size() - 1) + "/" + plan.textures[i].decl.name;
        }
    }
    return plan;
}

// What decides the passes, textures and pipelines a plan needs: two plans with the same shape differ
// only in their option values.
inline std::string plan_shape(const Plan &plan)
{
    std::string out;
    for (auto &i : plan.instances) {
        out += instance_name(i.effect, i.label) + "{";
        for (auto &p : i.decl->passes)
            out += p.dxil + ";";
        out += "}";
    }
    return out;
}

// For each texture of a new plan, the texture of the old plan it takes over (-1 for none): the same
// instance position running the same effect with the same texture declaration.
inline std::vector<int> reuse_textures(const Plan &old, const Plan &now)
{
    std::vector<int> from(now.textures.size(), -1);
    for (size_t i = 0; i < now.textures.size(); i++)
        for (size_t j = 0; j < old.textures.size(); j++)
            if (old.textures[j].name == now.textures[i].name && old.textures[j].effect == now.textures[i].effect &&
                old.textures[j].decl == now.textures[i].decl) {
                from[i] = int(j);
                break;
            }
    return from;
}

// Pass title for logs: "<config> · <effect>[.<label>] · <pass>".
inline std::string pass_title(const std::string &config, const std::string &effect, const std::string &label,
                              const std::string &pass)
{
    return config + " \xC2\xB7 " + instance_name(effect, label) + " \xC2\xB7 " + pass;
}

// Scales an RGB24 picture to dw x dh by area-weighted averaging: every output pixel is the mean of the
// source area it covers, partial pixels by their share.
inline void box_scale_rgb(const uint8_t *src, uint32_t sw, uint32_t sh, uint8_t *dst, uint32_t dw, uint32_t dh)
{
    if (!sw || !sh || !dw || !dh)
        return;
    std::vector<float> row(size_t(sw) * 3);
    double fx = double(sw) / dw, fy = double(sh) / dh;
    for (uint32_t dy = 0; dy < dh; dy++) {
        std::fill(row.begin(), row.end(), 0.0f);
        double y0 = dy * fy, y1 = y0 + fy;
        for (uint32_t sy = uint32_t(y0); sy < sh && sy < y1; sy++) {
            double w = std::min<double>(sy + 1, y1) - std::max<double>(sy, y0);
            const uint8_t *line = src + size_t(sy) * sw * 3;
            for (size_t i = 0; i < size_t(sw) * 3; i++)
                row[i] += float(w) * line[i];
        }
        for (uint32_t dx = 0; dx < dw; dx++) {
            double x0 = dx * fx, x1 = x0 + fx, acc[3] = {};
            for (uint32_t sx = uint32_t(x0); sx < sw && sx < x1; sx++) {
                double w = std::min<double>(sx + 1, x1) - std::max<double>(sx, x0);
                for (int c = 0; c < 3; c++)
                    acc[c] += w * row[size_t(sx) * 3 + c];
            }
            for (int c = 0; c < 3; c++) {
                double v = acc[c] / (fx * fy) + 0.5;
                dst[(size_t(dy) * dw + dx) * 3 + c] = uint8_t(v > 255 ? 255 : v);
            }
        }
    }
}

}
