// SPDX-License-Identifier: MIT
#pragma once
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <deque>
#include <string>
#include <string_view>
#include <swofty/overlay.hpp>

// A small immediate-mode toolkit on the overlay: panels, labels, buttons, toggles, sliders, lists and
// toasts. Keep one ui::State per addon (it outlives frames) and make a ui::Ui at the top of each paint
// callback. Rects are framebuffer pixels; theme sizes are for 1920x1080 and scale with ui_scale().
// Buttons, toggles, sliders and lists react only while the addon holds input focus (focus(true)).

namespace swofty::overlay::ui {

struct Theme {
    uint32_t panel_top = 0x16161ef2, panel_bottom = 0x24242ff0, edge = 0xffffff24, accent = 0xe10600ff;
    uint32_t text = 0xffffffff, dim = 0xffffffa8, hover = 0xffffff18, press = 0xffffff2e, track = 0xffffff2a;
    uint32_t toast = 0x111118f2;
    float radius = 10, title = 22, body = 18, pad = 14, row = 34;
};

struct Toast {
    std::string text;
    uint64_t start = 0, until = 0;
};

struct State {
    uint32_t hot = 0, active = 0;
    float grab = 0;
    std::deque<Toast> toasts;
};

inline uint32_t id_of(std::string_view s)
{
    uint32_t h = 2166136261u;
    for (unsigned char c : s)
        h = (h ^ c) * 16777619u;
    return h ? h : 1;
}

inline uint64_t now_ms()
{
    return GetTickCount64();
}

class Ui {
  public:
    Ui(Paint *paint, State &state, const Theme &theme = {})
        : p(paint), st(state), th(theme), s(ui_scale(paint)), in(input(paint))
    {
        st.hot = 0;
        if (!in.focused)
            st.active = 0;
    }

    Paint *p;
    State &st;
    Theme th;
    float s;
    Input in;

    float u(float v) const { return v * s; }

    bool over(const Rect &r) const { return in.focused && in.inside(r.x, r.y, r.w, r.h); }

    // Hot/active bookkeeping for one widget: returns true on the frame a press inside it is released
    // inside it.
    bool interact(uint32_t id, const Rect &r)
    {
        bool inside = over(r);
        if (inside && (!st.active || st.active == id))
            st.hot = id;
        if (inside && in.clicked())
            st.active = id;
        bool click = false;
        if (st.active == id && (in.released & mouse::left)) {
            click = inside;
            st.active = 0;
        }
        return click;
    }

    TextStyle style(float size, uint32_t rgba, uint32_t align) const
    {
        TextStyle t;
        t.size = u(size);
        t.rgba = rgba;
        t.align = align;
        return t;
    }
};

inline Rect inset(const Rect &r, float d)
{
    return {r.x + d, r.y + d, std::max(0.0f, r.w - 2 * d), std::max(0.0f, r.h - 2 * d)};
}

// A panel with a title bar; returns the content rect under the title.
inline Rect panel(Ui &ui, const Rect &r, std::string_view title)
{
    const Theme &th = ui.th;
    gradient(ui.p, r.x, r.y, r.w, r.h, th.panel_top, th.panel_bottom, false, ui.u(th.radius));
    rect_rounded_outline(ui.p, r.x, r.y, r.w, r.h, ui.u(th.radius), ui.u(1), th.edge);
    float pad = ui.u(th.pad);
    if (title.empty())
        return inset(r, pad);
    float bar = ui.u(th.title * 1.8f);
    TextStyle t = ui.style(th.title, th.text, align::left | align::middle);
    text(ui.p, r.x + pad, r.y + bar / 2, t, title);
    rect(ui.p, r.x + pad, r.y + bar - ui.u(2), ui.u(36), ui.u(3), th.accent);
    return {r.x + pad, r.y + bar + pad / 2, std::max(0.0f, r.w - 2 * pad), std::max(0.0f, r.h - bar - pad * 1.5f)};
}

inline void label(Ui &ui, const Rect &r, std::string_view s, uint32_t rgba = 0, uint32_t a = align::left | align::middle)
{
    text_in(ui.p, r, ui.style(ui.th.body, rgba ? rgba : ui.th.text, a), s);
}

inline bool button(Ui &ui, const Rect &r, std::string_view label_text)
{
    uint32_t id = id_of(label_text) ^ 0x9e3779b9u;
    bool click = ui.interact(id, r);
    const Theme &th = ui.th;
    uint32_t fill = ui.st.active == id ? th.press : ui.st.hot == id ? th.hover : 0x00000000u;
    rect_rounded(ui.p, r.x, r.y, r.w, r.h, ui.u(th.radius * 0.6f), 0xffffff10);
    if (fill)
        rect_rounded(ui.p, r.x, r.y, r.w, r.h, ui.u(th.radius * 0.6f), fill);
    rect_rounded_outline(ui.p, r.x, r.y, r.w, r.h, ui.u(th.radius * 0.6f), ui.u(1.5f),
                         ui.st.hot == id ? th.accent : th.edge);
    label(ui, r, label_text, th.text, align::center | align::middle);
    return click;
}

inline bool toggle(Ui &ui, const Rect &r, std::string_view label_text, bool &on)
{
    uint32_t id = id_of(label_text) ^ 0x7f4a7c15u;
    bool click = ui.interact(id, r);
    if (click)
        on = !on;
    const Theme &th = ui.th;
    if (ui.st.hot == id)
        rect_rounded(ui.p, r.x, r.y, r.w, r.h, ui.u(6), th.hover);
    float h = std::min(r.h * 0.7f, ui.u(24)), w = h * 1.8f, x = r.x + r.w - w - ui.u(6), y = r.y + (r.h - h) / 2;
    rect_rounded(ui.p, x, y, w, h, h / 2, on ? th.accent : th.track);
    circle(ui.p, on ? x + w - h / 2 : x + h / 2, y + h / 2, h / 2 - ui.u(3), th.text);
    label(ui, {r.x + ui.u(6), r.y, r.w - w - ui.u(18), r.h}, label_text);
    return click;
}

// A horizontal slider: the label and the value (printed with fmt) above a track. Returns true while
// the value changes.
inline bool slider(Ui &ui, const Rect &r, std::string_view label_text, float &value, float lo, float hi,
                   const char *fmt = "%.0f")
{
    uint32_t id = id_of(label_text) ^ 0x2545f491u;
    const Theme &th = ui.th;
    float top = r.h * 0.5f, ty = r.y + top + (r.h - top) / 2, knob = ui.u(9);
    Rect track{r.x + knob, ty - ui.u(3), r.w - 2 * knob, ui.u(6)};
    Rect hit{r.x, r.y + top, r.w, r.h - top};
    ui.interact(id, hit);
    float before = value;
    if (ui.st.active == id && ui.in.held() && track.w > 0) {
        float t = std::clamp((ui.in.x - track.x) / track.w, 0.0f, 1.0f);
        value = lo + (hi - lo) * t;
    }
    if (ui.st.hot == id && ui.in.wheel != 0)
        value = std::clamp(value + (hi - lo) * 0.05f * ui.in.wheel, std::min(lo, hi), std::max(lo, hi));
    float t = hi != lo ? std::clamp((value - lo) / (hi - lo), 0.0f, 1.0f) : 0;
    label(ui, {r.x, r.y, r.w, top}, label_text, th.dim);
    char b[48];
    snprintf(b, sizeof b, fmt, double(value));
    label(ui, {r.x, r.y, r.w, top}, b, th.text, align::right | align::middle);
    rect_rounded(ui.p, track.x, track.y, track.w, track.h, track.h / 2, th.track);
    rect_rounded(ui.p, track.x, track.y, track.w * t, track.h, track.h / 2, th.accent);
    float kx = track.x + track.w * t;
    circle(ui.p, kx, ty, ui.st.active == id || ui.st.hot == id ? knob * 1.15f : knob, th.text);
    return value != before;
}

// A scrolling list of count rows; item(i) returns row i's text. The wheel scrolls and a click selects.
// Returns true when the selection changes.
template <class ItemFn>
bool list(Ui &ui, const Rect &r, std::string_view id_text, int count, ItemFn &&item, int &selected, float &scroll)
{
    uint32_t base = id_of(id_text);
    const Theme &th = ui.th;
    float row = ui.u(th.row);
    float content = row * float(std::max(count, 0)), max_scroll = std::max(0.0f, content - r.h);
    if (ui.over(r) && ui.in.wheel != 0)
        scroll -= ui.in.wheel * row * 3;
    scroll = std::clamp(scroll, 0.0f, max_scroll);
    rect_rounded(ui.p, r.x, r.y, r.w, r.h, ui.u(6), 0x00000040);
    int before = selected;
    {
        ClipScope clip(ui.p, r.x, r.y, r.w, r.h);
        int first = int(scroll / row), last = std::min(count, int((scroll + r.h) / row) + 1);
        for (int i = std::max(first, 0); i < last; i++) {
            Rect rr{r.x, r.y + float(i) * row - scroll, r.w - ui.u(8), row};
            uint32_t id = base + uint32_t(i) * 2654435761u;
            if (ui.interact(id, rr) && ui.over(r))
                selected = i;
            if (i == selected) {
                rect(ui.p, rr.x, rr.y, rr.w, rr.h, 0xffffff14);
                rect(ui.p, rr.x, rr.y, ui.u(3), rr.h, th.accent);
            } else if (ui.st.hot == id) {
                rect(ui.p, rr.x, rr.y, rr.w, rr.h, th.hover);
            }
            label(ui, {rr.x + ui.u(12), rr.y, rr.w - ui.u(16), rr.h}, item(i), i == selected ? th.text : th.dim);
        }
    }
    if (max_scroll > 0) {
        float h = std::max(ui.u(20), r.h * r.h / content), y = r.y + (r.h - h) * (scroll / max_scroll);
        rect_rounded(ui.p, r.x + r.w - ui.u(5), y, ui.u(3), h, ui.u(1.5f), th.track);
    }
    return selected != before;
}

// Queue a toast for seconds; draw_toasts() shows the queue.
inline void toast(State &st, std::string text_utf8, float seconds = 3)
{
    uint64_t now = now_ms();
    st.toasts.push_back({std::move(text_utf8), now, now + uint64_t(std::max(0.5f, seconds) * 1000)});
    while (st.toasts.size() > 5)
        st.toasts.pop_front();
}

// Toasts stack down from the top right of the safe area, sliding in and fading out.
inline void draw_toasts(Ui &ui)
{
    uint64_t now = now_ms();
    while (!ui.st.toasts.empty() && ui.st.toasts.front().until <= now)
        ui.st.toasts.pop_front();
    if (ui.st.toasts.empty())
        return;
    const Theme &th = ui.th;
    Rect safe = safe_area(ui.p);
    float w = ui.u(380), y = safe.y;
    TextStyle t = ui.style(th.body, th.text, align::left | align::top);
    t.max_width = w - ui.u(th.pad * 2 + 6);
    for (const Toast &e : ui.st.toasts) {
        float tw = 0, h = 0;
        measure(ui.p, t, e.text, &tw, &h);
        h += ui.u(th.pad * 2);
        float in_t = std::min(1.0f, float(now - e.start) / 180.0f), out_t = std::min(1.0f, float(e.until - now) / 300.0f);
        float a = std::min(in_t, out_t), slide = (1 - in_t) * ui.u(40);
        float x = safe.x + safe.w - w + slide;
        OpacityScope fade(ui.p, a);
        rect_rounded(ui.p, x, y, w, h, ui.u(8), th.toast);
        rect_rounded(ui.p, x, y, ui.u(5), h, ui.u(2.5f), th.accent);
        text(ui.p, x + ui.u(th.pad + 6), y + ui.u(th.pad), t, e.text);
        y += (h + ui.u(10)) * std::max(a, 0.25f);
    }
}

}
