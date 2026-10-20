// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - its own profile save.
//
// NeSaveSystem2 names every file of a save from the operation's save name: Steam remote storage names
// are <title dir>@<save name>#<file> (plus <save name>_backup), with the file names inside derived
// from the job (index QKRHMYXE, data NXDSMWW.BWW for the profile). The game picks the profile save
// from the enumerated save list, not by name, so two things are needed:
//   - the save requests (Ne_Request_SaveOpKind1/2) rename the operation's save "profile" to "f1990",
//     so every file the device builds for it lands in savegame@f1990#*;
//   - the device's remote storage enumeration (the ISteamRemoteStorage it caches at helper+0x108,
//     used only for GetFileCount/GetFileNameAndSize) goes through a proxy that hides the stock
//     savegame@profile* files, so the game never selects, loads, backs up or wipes them.
// The proxy forwards every other slot to the real interface unchanged. Optional one-off cleanup
// (setting cleanup_stray_save, off by default) deletes the stray savegame@profile#D1990.UEL left by
// the first build through the real interface's FileDelete.

#include <atomic>
#include <cstring>
#include <mutex>
#include <vector>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "save_name.hpp"

SWOFTY_SETTING(bool, cleanup_stray_save, false);

namespace {

constexpr int kSlots = 64;
constexpr int kFileDelete = 6;
constexpr int kFileExists = 13;
constexpr int kGetFileCount = 18;
constexpr int kGetFileNameAndSize = 19;
constexpr char kStrayFile[] = "savegame@profile#D1990.UEL";

struct RemoteProxy {
    void **vtable;
    void *real;
};

RemoteProxy g_proxy;
std::mutex g_lock;
std::vector<int> g_visible;
std::atomic<uint32_t> g_redirects;
bool g_hidden_logged;

void **real_vtable()
{
    return *static_cast<void ***>(g_proxy.real);
}

int32_t real_count()
{
    return reinterpret_cast<int32_t (*)(void *)>(real_vtable()[kGetFileCount])(g_proxy.real);
}

const char *real_name(int i, int32_t *size)
{
    return reinterpret_cast<const char *(*)(void *, int, int32_t *)>(real_vtable()[kGetFileNameAndSize])(g_proxy.real,
                                                                                                         i, size);
}

void rebuild_locked()
{
    g_visible.clear();
    int32_t n = real_count();
    int hidden = 0;
    for (int i = 0; i < n; i++) {
        int32_t size = 0;
        if (f1990::hidden_remote_name(real_name(i, &size)))
            hidden++;
        else
            g_visible.push_back(i);
    }
    if (hidden && !g_hidden_logged) {
        g_hidden_logged = true;
        swofty::log("[f1-1990] save: %d stock profile file(s) hidden from the save list", hidden);
    }
}

int32_t proxy_count(RemoteProxy *)
{
    std::lock_guard<std::mutex> g(g_lock);
    rebuild_locked();
    return int32_t(g_visible.size());
}

const char *proxy_name(RemoteProxy *, int i, int32_t *size)
{
    std::lock_guard<std::mutex> g(g_lock);
    if (g_visible.empty())
        rebuild_locked();
    if (i < 0 || size_t(i) >= g_visible.size()) {
        if (size)
            *size = 0;
        return "";
    }
    return real_name(g_visible[size_t(i)], size);
}

#define F1990_FWD(n)                                                                                                  \
    extern "C" __attribute__((naked)) void f1990_remote_fwd_##n()                                                     \
    {                                                                                                                 \
        asm volatile("movq 8(%rcx), %rcx\n\tmovq (%rcx), %rax\n\tjmpq *(" #n "*8)(%rax)");                            \
    }
#define F1990_FWD10(a)                                                                                                \
    F1990_FWD(a##0) F1990_FWD(a##1) F1990_FWD(a##2) F1990_FWD(a##3) F1990_FWD(a##4) F1990_FWD(a##5) F1990_FWD(a##6) \
        F1990_FWD(a##7) F1990_FWD(a##8) F1990_FWD(a##9)
F1990_FWD(0) F1990_FWD(1) F1990_FWD(2) F1990_FWD(3) F1990_FWD(4) F1990_FWD(5) F1990_FWD(6) F1990_FWD(7)
F1990_FWD(8) F1990_FWD(9) F1990_FWD10(1) F1990_FWD10(2) F1990_FWD10(3) F1990_FWD10(4) F1990_FWD10(5)
F1990_FWD(60) F1990_FWD(61) F1990_FWD(62) F1990_FWD(63)

#define F1990_REF(n) reinterpret_cast<void *>(&f1990_remote_fwd_##n)
#define F1990_REF10(a)                                                                                                \
    F1990_REF(a##0), F1990_REF(a##1), F1990_REF(a##2), F1990_REF(a##3), F1990_REF(a##4), F1990_REF(a##5),             \
        F1990_REF(a##6), F1990_REF(a##7), F1990_REF(a##8), F1990_REF(a##9)

void *g_vtable[kSlots] = {F1990_REF(0), F1990_REF(1), F1990_REF(2), F1990_REF(3), F1990_REF(4),
                          F1990_REF(5), F1990_REF(6), F1990_REF(7), F1990_REF(8), F1990_REF(9),
                          F1990_REF10(1), F1990_REF10(2), F1990_REF10(3), F1990_REF10(4), F1990_REF10(5),
                          F1990_REF(60), F1990_REF(61), F1990_REF(62), F1990_REF(63)};

void cleanup_stray(void *real)
{
    if (!cleanup_stray_save)
        return;
    void **vt = *static_cast<void ***>(real);
    bool exists = reinterpret_cast<bool (*)(void *, const char *)>(vt[kFileExists])(real, kStrayFile);
    if (!exists) {
        swofty::log("[f1-1990] save cleanup: %s is not in remote storage; nothing to delete", kStrayFile);
        return;
    }
    bool ok = reinterpret_cast<bool (*)(void *, const char *)>(vt[kFileDelete])(real, kStrayFile);
    swofty::log("[f1-1990] save cleanup: FileDelete(%s) %s", kStrayFile, ok ? "ok" : "FAILED");
}

void install_proxy(void *device)
{
    auto *helper = device ? *reinterpret_cast<uint8_t **>(static_cast<uint8_t *>(device) + 0x30) : nullptr;
    if (!helper) {
        for (int i = 0; i < 3; i++)
            swofty::log("[f1-1990] SAVE SEPARATION INCOMPLETE: the save device has no Steam remote storage helper "
                        "(local saves), so the stock profile save stays in the save list. Quit before opening any "
                        "career.");
        return;
    }
    void **slot = reinterpret_cast<void **>(helper + 0x108);
    if (!*slot || *slot == &g_proxy)
        return;
    g_vtable[kGetFileCount] = reinterpret_cast<void *>(&proxy_count);
    g_vtable[kGetFileNameAndSize] = reinterpret_cast<void *>(&proxy_name);
    g_proxy.vtable = g_vtable;
    g_proxy.real = *slot;
    cleanup_stray(g_proxy.real);
    *slot = &g_proxy;
    swofty::log("[f1-1990] save: save list enumeration filtered (stock savegame@profile* hidden)");
}

void rename_request(const char *&name)
{
    const char *mapped = f1990::save_name_for(name);
    if (mapped != name)
        swofty::log("[f1-1990] save: profile -> %s (#%u)", mapped, ++g_redirects);
    name = mapped;
}

}

SWOFTY_WRAP(Ne_SaveSystem2Win_Start, uint64_t, void *device)
{
    install_proxy(device);
    return orig(device);
}

SWOFTY_WRAP(Ne_Request_SaveOpKind1, void *, void *save_system, void *out, void *user, const char *save_name, uint64_t a5,
            void *a6)
{
    rename_request(save_name);
    return orig(save_system, out, user, save_name, a5, a6);
}

SWOFTY_WRAP(Ne_Request_SaveOpKind2, void *, void *save_system, void *out, void *user, const char *save_name, uint64_t a5,
            void *a6)
{
    rename_request(save_name);
    return orig(save_system, out, user, save_name, a5, a6);
}

SWOFTY_ON_LOAD
{
    swofty::log("[f1-1990] save separation on: the profile save is \"%s\" (savegame@%s#*) in this profile%s",
                f1990::kProfileSave1990, f1990::kProfileSave1990,
                cleanup_stray_save ? "; cleanup_stray_save is ON" : "");
}
