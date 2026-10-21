#include "runtime.hpp"

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <deque>
#include <mutex>
#include <string>

#include "perf.hpp"

// Message popups (NeUIMessageSystem). The game raises every popup the same way: construct a message
// by its UIMD id (Ui_Message_Construct, 0x1442d9bf0: system, out handle, fixed-capacity id string,
// response callback, two unused zeros), set its user attributes through the handle
// (Ui_Message_SetAttribute, 0x1406a79c0, takes an NeStr reference), then drop the handle
// (Ui_Message_ReleaseHandle, 0x1442aa980). The message stays queued and the persistent screen's
// multichoice popup draws it. The beta_motd poster (0x1436d2120) does exactly this with a literal
// "message" attribute, so one signature over its body yields the id string vtable, the constructor
// and the attribute setter.
//
// The text comes from two attributes, "title" and "message", which the swofty_message definition
// the launcher adds to onlineservices_messages.uimsg reads through lng_value_tag tokens. When that
// definition is missing (an old UI build), the core falls back to the stock beta_motd message, whose
// body token reads "message"; the title then leads the body.
//
// Every construct passes through the wrap below, which records the system pointer (the game
// constructs messages from boot on, the online-services wait box first). Requests queue from any
// thread and are shown from the flow tick.

std::string settings_addon(const void *from, std::string &dll);

namespace {

const runtime::Sig S_MOTD{
    "Ui_Message_PostMotd",
    "48 B9 62 65 74 61 5F 6D 6F 74 48 89 4C 24 58 66 C7 44 24 60 64 00 48 8D 0D ?? ?? ?? ?? 48 89 4C 24 40 C5 F9 57 "
    "C0 C5 F9 29 44 24 30 C5 F9 11 44 24 20 48 8D 9C 24 E0 00 00 00 4C 8D 44 24 40 4C 8D 4C 24 30 48 89 C1 48 89 DA "
    "E8 ?? ?? ?? ?? 48 8B 05 ?? ?? ?? ?? 48 8B 50 68 48 C7 84 24 D8 00 00 00 00 00 00 00 4C 8D B4 24 D8 00 00 00 4C "
    "89 F1 45 31 C0 45 31 C9 E8 ?? ?? ?? ?? 48 C7 44 24 20 00 00 00 00 48 8D 15 ?? ?? ?? ?? 48 89 D9 4D 89 F0 45 31 "
    "C9 E8 ?? ?? ?? ??",
    0x36d23ca};
const runtime::Sig S_RELEASE{"Ui_Message_ReleaseHandle",
                             "56 57 53 48 83 EC 30 48 8B 79 08 48 85 FF 0F 84 ?? ?? ?? ?? 48 89 CE 48 8B 8F 90 00 00 00 "
                             "48 8B 87 98 00 00 00",
                             0x42aa980};
constexpr uintptr_t MOTD_VT_DISP = 0x19, MOTD_VT_END = 0x1d, MOTD_CONSTRUCT = 0x4b, MOTD_MAKE_STR = 0x78,
                    MOTD_SET_ATTR = 0x96;
constexpr const uint8_t CONSTRUCT_HEAD[] = {0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x83, 0xEC, 0x30};
constexpr const char *OWN_ID = "swofty_message", *FALLBACK_ID = "beta_motd";
constexpr size_t MAX_QUEUED = 8, MAX_TEXT = 4000;

struct FixedId {
    const void *vt;
    uint64_t cap;
    char *data;
    char buf[128];
};

struct Handle {
    void *sys;
    void *msg;
};

struct Callback {
    void *fn;
    void *ctx;
};

using Construct_t = Handle *(void *sys, Handle *out, FixedId *id, Callback *cb, const char *a5, uint64_t a6);
using SetAttr_t = bool(Handle *h, const char *name, void **value, uint32_t flags, const char *a5);
using Release_t = void(Handle *h);
using MakeStr_t = uint32_t(void **out, const char *s, uint32_t flags, uintptr_t len);

struct Request {
    std::string title, body, by;
};

uintptr_t a_construct, a_set_attr, a_release, a_make_str;
const void *id_vt;
safetyhook::InlineHook h_construct;
std::atomic<void *> sys;
std::atomic<bool> live, own_missing;
std::mutex queue_lock;
std::deque<Request> queue;
std::atomic<size_t> pending;

uintptr_t rel32(uintptr_t at)
{
    return at + 4 + *reinterpret_cast<const int32_t *>(at);
}

Handle *construct_hook(void *s, Handle *out, FixedId *id, Callback *cb, const char *a5, uint64_t a6)
{
    void *was = sys.exchange(s);
    if (was != s)
        swofty::log("[popup] message system %p%s (first message '%.64s')", s, was ? " replaces an earlier one" : "",
                    id && swofty::mem::readable(id, sizeof(FixedId)) && id->data && swofty::mem::readable(id->data, 1)
                        ? id->data
                        : "?");
    return h_construct.call<Handle *>(s, out, id, cb, a5, a6);
}

bool make_id(FixedId &id, const char *name)
{
    id.vt = id_vt;
    id.cap = sizeof id.buf;
    id.data = id.buf;
    return snprintf(id.buf, sizeof id.buf, "%s", name) > 0;
}

bool set(Handle &h, const char *name, const std::string &text)
{
    void *s = nullptr;
    reinterpret_cast<MakeStr_t *>(a_make_str)(&s, text.c_str(), 0, 0);
    return reinterpret_cast<SetAttr_t *>(a_set_attr)(&h, name, &s, 0, nullptr);
}

Handle open(void *s, const char *name)
{
    Handle h{};
    FixedId id{};
    Callback cb{};
    if (make_id(id, name))
        reinterpret_cast<Construct_t *>(a_construct)(s, &h, &id, &cb, nullptr, 0);
    return h;
}

void show(void *s, const Request &r)
{
    Handle h = own_missing ? Handle{} : open(s, OWN_ID);
    bool own = h.msg != nullptr;
    if (!own) {
        if (!own_missing.exchange(true))
            swofty::log("[popup] the game has no '%s' message (the launcher's UI build adds it); falling back to "
                        "'%s', whose title is fixed, so the title leads the body",
                        OWN_ID, FALLBACK_ID);
        h = open(s, FALLBACK_ID);
    }
    if (!h.msg) {
        swofty::log("[popup] popup from %s dropped: the game constructed no message ('%s' and '%s' both refused)",
                    r.by.c_str(), OWN_ID, FALLBACK_ID);
        return;
    }
    bool ok = own ? set(h, "title", r.title) && set(h, "message", r.body)
                  : set(h, "message", r.title.empty() ? r.body : r.body.empty() ? r.title : r.title + "\n\n" + r.body);
    reinterpret_cast<Release_t *>(a_release)(&h);
    swofty::log("[popup] shown for %s as '%s': \"%.80s\"%s", r.by.c_str(), own ? OWN_ID : FALLBACK_ID, r.title.c_str(),
                ok ? "" : " (an attribute was refused; the popup may show empty text)");
}

int api_available()
{
    return live && runtime::enabled("popup", nullptr);
}

std::string who(const void *from)
{
    std::string dll, id = settings_addon(from, dll);
    return id.empty() ? (dll.empty() ? std::string("swofty_core") : dll) : id;
}

int enqueue(std::string t, std::string b, const std::string &by)
{
    if (t.empty() && b.empty()) {
        swofty::log("[popup] show by %s refused: empty title and body", by.c_str());
        return SWOFTY_ASSET_INVALID;
    }
    if (!api_available()) {
        swofty::log("[popup] show by %s: unavailable (%s)", by.c_str(),
                    runtime::enabled("popup", nullptr) ? "the message system did not resolve on this build"
                                                       : "the popup feature is off");
        return SWOFTY_ASSET_UNAVAILABLE;
    }
    if (t.size() > MAX_TEXT)
        t.resize(MAX_TEXT);
    if (b.size() > MAX_TEXT)
        b.resize(MAX_TEXT);
    std::lock_guard<std::mutex> g(queue_lock);
    for (auto &q : queue)
        if (q.title == t && q.body == b)
            return SWOFTY_ASSET_OK;
    if (queue.size() >= MAX_QUEUED) {
        swofty::log("[popup] queue full (%zu); dropping the oldest popup, from %s", queue.size(), queue.front().by.c_str());
        queue.pop_front();
    }
    queue.push_back({std::move(t), std::move(b), by});
    pending = queue.size();
    swofty::log("[popup] queued by %s: \"%.80s\"%s", by.c_str(), queue.back().title.c_str(),
                sys.load() ? "" : " (waiting for the game's first message to find the message system)");
    return SWOFTY_ASSET_OK;
}

int api_show(const char *title, const char *body)
{
    return enqueue(title ? title : "", body ? body : "", who(__builtin_return_address(0)));
}

const SwoftyPopup table = {api_available, api_show};

}

namespace runtime {

const SwoftyPopup *popup_table()
{
    return &table;
}

int popup_show(const std::string &title, const std::string &body, const std::string &by)
{
    return enqueue(title, body, by);
}

void popup_setup()
{
    if (!enabled("popup", "popup")) {
        swofty::log("[popup] feature off; popup.show reports unavailable");
        return;
    }
    uintptr_t base = swofty::api->module_base();
    uintptr_t motd = find(S_MOTD, "message system", "popup");
    a_release = find(S_RELEASE, "message system", "popup");
    if (motd) {
        id_vt = reinterpret_cast<const void *>(motd + MOTD_VT_END + *reinterpret_cast<const int32_t *>(motd + MOTD_VT_DISP));
        a_construct = rel32(motd + MOTD_CONSTRUCT);
        a_make_str = rel32(motd + MOTD_MAKE_STR);
        a_set_attr = rel32(motd + MOTD_SET_ATTR);
    }
    if (!motd || !a_release || !swofty::mem::in_image(a_construct) ||
        memcmp(reinterpret_cast<const void *>(a_construct), CONSTRUCT_HEAD, sizeof CONSTRUCT_HEAD)) {
        swofty::log("[popup] the message system did not resolve (poster %s, release %s, constructor %s); popup.show "
                    "reports unavailable",
                    motd ? "found" : "missing", a_release ? "found" : "missing",
                    a_construct ? "shape differs" : "missing");
        return;
    }
    auto r = safetyhook::InlineHook::create(reinterpret_cast<void *>(a_construct), reinterpret_cast<void *>(&construct_hook));
    if (!r) {
        swofty::log("[popup] hook on Ui_Message_Construct at base+0x%llx failed; popup.show reports unavailable",
                    (unsigned long long)(a_construct - base));
        return;
    }
    h_construct = std::move(*r);
    live = true;
    swofty::log("[popup] setup: live; construct base+0x%llx, set attribute base+0x%llx, release base+0x%llx, id vtable "
                "base+0x%llx",
                (unsigned long long)(a_construct - base), (unsigned long long)(a_set_attr - base),
                (unsigned long long)(a_release - base), (unsigned long long)(uintptr_t(id_vt) - base));
}

void popup_tick()
{
    if (!pending.load(std::memory_order_relaxed))
        return;
    void *s = sys.load();
    if (!s)
        return;
    static const uint32_t probe = perf::probe("core", "popup show");
    perf::Scope p(probe);
    std::deque<Request> now;
    {
        std::lock_guard<std::mutex> g(queue_lock);
        now.swap(queue);
        pending = 0;
    }
    for (auto &r : now) {
        perf::Pause game;
        show(s, r);
    }
}

}
