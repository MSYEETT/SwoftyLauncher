#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "swofty_api.h"

#define HOST_EXE L"F1_25.exe"
#define EXPECTED_SIZE 408063872ULL
#define EXPECTED_STAMP 1789118616UL
#define WINDOW_TIMEOUT_MS 300000
#define POLL_MS 50

static const char *const fwd_names[] = {
    "GetFileVersionInfoA", "GetFileVersionInfoByHandle", "GetFileVersionInfoExA", "GetFileVersionInfoExW",
    "GetFileVersionInfoSizeA", "GetFileVersionInfoSizeExA", "GetFileVersionInfoSizeExW", "GetFileVersionInfoSizeW",
    "GetFileVersionInfoW", "VerFindFileA", "VerFindFileW", "VerInstallFileA", "VerInstallFileW",
    "VerLanguageNameA", "VerLanguageNameW", "VerQueryValueA", "VerQueryValueW",
};
#define NFWD (sizeof fwd_names / sizeof *fwd_names)
__attribute__((used)) void *swofty_fwd[NFWD];

#define FWD(i, n) __asm__(".text\n.globl fwd_" #n "\n.def fwd_" #n "; .scl 2; .type 32; .endef\n" \
                          "fwd_" #n ": jmp *swofty_fwd+" #i "*8(%rip)\n");
FWD(0, GetFileVersionInfoA)
FWD(1, GetFileVersionInfoByHandle)
FWD(2, GetFileVersionInfoExA)
FWD(3, GetFileVersionInfoExW)
FWD(4, GetFileVersionInfoSizeA)
FWD(5, GetFileVersionInfoSizeExA)
FWD(6, GetFileVersionInfoSizeExW)
FWD(7, GetFileVersionInfoSizeW)
FWD(8, GetFileVersionInfoW)
FWD(9, VerFindFileA)
FWD(10, VerFindFileW)
FWD(11, VerInstallFileA)
FWD(12, VerInstallFileW)
FWD(13, VerLanguageNameA)
FWD(14, VerLanguageNameW)
FWD(15, VerQueryValueA)
FWD(16, VerQueryValueW)

static ULONG_PTR WINAPI fwd_missing(void)
{
    SetLastError(ERROR_PROC_NOT_FOUND);
    return 0;
}

typedef struct {
    char name[9];
    uintptr_t start;
    size_t size;
    DWORD flags;
} Section;

static HANDLE log_file = INVALID_HANDLE_VALUE;
static CRITICAL_SECTION log_lock;
static wchar_t exe_path[MAX_PATH], game_dir[MAX_PATH];
static uintptr_t image_base;
static Section sections[96];
static int section_count, addons_loaded, addons_ready;
static volatile LONG load_state, window_done;
static DWORD attach_tick, load_thread;
static HANDLE loaded_event;
static int early_enabled, early_env, loaded_early;
static char early_source[160] = "off";
static wchar_t early_plugins[64][MAX_PATH], plugins[256][MAX_PATH];
static int early_count, plugin_count;

static void vlog(const char *fmt, va_list ap)
{
    if (log_file == INVALID_HANDLE_VALUE)
        return;
    char buf[2048];
    SYSTEMTIME t;
    GetLocalTime(&t);
    int n = snprintf(buf, sizeof buf, "%04u-%02u-%02u %02u:%02u:%02u.%03u [%5lu] ", t.wYear, t.wMonth, t.wDay,
                     t.wHour, t.wMinute, t.wSecond, t.wMilliseconds, GetCurrentThreadId());
    int m = vsnprintf(buf + n, sizeof buf - n - 2, fmt, ap);
    n += m < 0 ? 0 : m > (int)sizeof buf - n - 3 ? (int)sizeof buf - n - 3 : m;
    buf[n++] = '\r';
    buf[n++] = '\n';
    DWORD w;
    EnterCriticalSection(&log_lock);
    WriteFile(log_file, buf, n, &w, NULL);
    LeaveCriticalSection(&log_lock);
}

static void slog(const char *fmt, ...)
{
    va_list ap;
    va_start(ap, fmt);
    vlog(fmt, ap);
    va_end(ap);
}

static uintptr_t module_base(void)
{
    return image_base;
}

static int parse_pattern(const char *p, int *out, int max)
{
    int n = 0;
    while (*p) {
        while (*p == ' ')
            p++;
        if (!*p)
            break;
        if (n == max)
            return -1;
        if (*p == '?') {
            out[n++] = -1;
            while (*p == '?')
                p++;
        } else {
            char *end;
            long v = strtol(p, &end, 16);
            if (end == p || end - p > 2 || v < 0 || v > 255)
                return -1;
            out[n++] = (int)v;
            p = end;
        }
    }
    return n;
}

static int scan_range(const uint8_t *lo, const uint8_t *hi, const int *pat, int n, int anchor, uintptr_t *first)
{
    int hits = 0;
    if (hi - lo < n)
        return 0;
    const uint8_t *last = hi - n + anchor;
    for (const uint8_t *p = lo + anchor; p <= last; p++) {
        p = memchr(p, pat[anchor], last - p + 1);
        if (!p)
            break;
        const uint8_t *c = p - anchor;
        int i = 0;
        while (i < n && (pat[i] < 0 || c[i] == pat[i]))
            i++;
        if (i == n && hits++ == 0)
            *first = (uintptr_t)c;
    }
    return hits;
}

static int is_readable(const MEMORY_BASIC_INFORMATION *m)
{
    return m->State == MEM_COMMIT && (m->Protect & 0xEE) && !(m->Protect & (PAGE_GUARD | PAGE_NOACCESS));
}

static int scan(const char *section, const char *pattern, uintptr_t *out)
{
    int pat[256], n = parse_pattern(pattern, pat, 256), anchor = 0;
    uintptr_t first = 0;
    int hits = 0, searched = 0;
    if (out)
        *out = 0;
    while (anchor < n && pat[anchor] < 0)
        anchor++;
    if (n <= 0 || anchor == n) {
        slog("scan: invalid pattern '%s'", pattern);
        return 0;
    }
    for (int s = 0; s < section_count; s++) {
        const Section *sec = &sections[s];
        if (section ? strcmp(sec->name, section) != 0 : !(sec->flags & IMAGE_SCN_MEM_EXECUTE))
            continue;
        searched++;
        uintptr_t a = sec->start, end = sec->start + sec->size, run = 0;
        while (a < end) {
            MEMORY_BASIC_INFORMATION m;
            if (!VirtualQuery((void *)a, &m, sizeof m))
                break;
            uintptr_t next = (uintptr_t)m.BaseAddress + m.RegionSize;
            if (next > end)
                next = end;
            if (is_readable(&m)) {
                if (!run)
                    run = a;
            } else if (run) {
                uintptr_t f = 0;
                int h = scan_range((const uint8_t *)run, (const uint8_t *)a, pat, n, anchor, &f);
                if (h && !hits)
                    first = f;
                hits += h;
                run = 0;
            }
            a = next;
        }
        if (run) {
            uintptr_t f = 0;
            int h = scan_range((const uint8_t *)run, (const uint8_t *)a, pat, n, anchor, &f);
            if (h && !hits)
                first = f;
            hits += h;
        }
    }
    if (!searched)
        slog("scan: section '%s' not found", section ? section : "<executable>");
    if (hits == 1) {
        slog("scan %s [%s] -> 1 hit at 0x%llx (base+0x%llx)", section ? section : "<exec>", pattern,
             (unsigned long long)first, (unsigned long long)(first - image_base));
        if (out)
            *out = first;
    } else if (hits) {
        slog("scan %s [%s] -> %d hits (first at 0x%llx), rejected: need exactly 1", section ? section : "<exec>",
             pattern, hits, (unsigned long long)first);
    } else {
        slog("scan %s [%s] -> 0 hits, rejected: need exactly 1", section ? section : "<exec>", pattern);
    }
    return hits;
}

static SwoftyApi api = {1, slog, module_base, scan, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL};
static const SwoftyApi api_v1 = {1, slog, module_base, scan, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL};
static SwoftyCoreInitFn core_init;
static SwoftyCoreCommitFn core_commit;
static SwoftyCoreStageFn core_stage;
static struct {
    wchar_t name[MAX_PATH];
    const char *symbol;
    SwoftyAddonInitFn init;
} deferred[256];
static int deferred_count;

static const wchar_t *const ROOTS[] = {L"addons", L"mods"};

#define PICK_ALL 0
#define PICK_EARLY 1
#define PICK_REST 2

typedef struct {
    int root;
    wchar_t *name;
} Dll;

static int dll_cmp(const void *a, const void *b)
{
    const Dll *x = a, *y = b;
    int c = _wcsicmp(x->name, y->name);
    return c ? c : x->root - y->root;
}

static void finish_init(const wchar_t *name, const char *symbol, int r)
{
    slog("addon %ls: %s returned %d (%s)", name, symbol, r, r ? "ok" : "FAILED");
    if (r)
        addons_ready++;
}

static void load_addon(const wchar_t *dir, const wchar_t *name, int core_dir)
{
    wchar_t path[MAX_PATH * 2];
    swprintf(path, MAX_PATH * 2, L"%ls\\%ls", dir, name);
    slog("addon %ls: loading %ls", name, path);
    HMODULE m = LoadLibraryW(path);
    if (!m) {
        slog("addon %ls: LoadLibrary failed, error %lu", name, GetLastError());
        return;
    }
    addons_loaded++;
    SwoftyCoreAttachFn attach = (SwoftyCoreAttachFn)(void *)GetProcAddress(m, "SwoftyCoreAttach");
    if (core_dir && attach && !core_init) {
        SwoftyCoreInitFn ci = (SwoftyCoreInitFn)(void *)GetProcAddress(m, "SwoftyCoreInit");
        SwoftyCoreCommitFn cc = (SwoftyCoreCommitFn)(void *)GetProcAddress(m, "SwoftyCoreCommit");
        if (ci && cc && attach(&api)) {
            core_init = ci;
            core_commit = cc;
            core_stage = (SwoftyCoreStageFn)(void *)GetProcAddress(m, "SwoftyCoreStage");
            slog("addon %ls: provides the mixin API v%u%s", name, api.version, core_stage ? ", stage handlers" : "");
        } else {
            slog("addon %ls: exports SwoftyCoreAttach but did not attach", name);
        }
    }
    const char *symbol = "SwoftyAddonInit", *api_symbol = "SwoftyAddonApi";
    SwoftyAddonInitFn init = (SwoftyAddonInitFn)(void *)GetProcAddress(m, symbol);
    if (!init) {
        symbol = "SwoftyModInit", api_symbol = "SwoftyModApi";
        init = (SwoftyAddonInitFn)(void *)GetProcAddress(m, symbol);
    }
    if (!init) {
        slog("addon %ls: loaded at %p but exports no SwoftyAddonInit or SwoftyModInit, left idle", name, (void *)m);
        return;
    }
    const uint32_t *wants = (const uint32_t *)(void *)GetProcAddress(m, api_symbol);
    if (!wants || *wants < 2) {
        if (deferred_count == 256) {
            slog("addon %ls: too many API v1 plugins, left idle", name);
            return;
        }
        wcsncpy(deferred[deferred_count].name, name, MAX_PATH - 1);
        deferred[deferred_count].symbol = symbol;
        deferred[deferred_count++].init = init;
        slog("addon %ls: loaded at %p, API v1 plugin, %s runs after the mixin commit", name, (void *)m, symbol);
        return;
    }
    slog("addon %ls: loaded at %p, calling %s (API v%u)", name, (void *)m, symbol, *wants);
    finish_init(name, symbol, core_init ? core_init(name, init) : init(&api));
}

static int list_dlls(int root, const wchar_t *sub, Dll *out, int n)
{
    wchar_t query[MAX_PATH * 2];
    swprintf(query, MAX_PATH * 2, L"%ls\\swofty\\%ls%ls%ls\\*.dll", game_dir, ROOTS[root], *sub ? L"\\" : L"", sub);
    WIN32_FIND_DATAW fd;
    HANDLE f = FindFirstFileW(query, &fd);
    if (f == INVALID_HANDLE_VALUE) {
        DWORD e = GetLastError();
        if (e != ERROR_FILE_NOT_FOUND && e != ERROR_PATH_NOT_FOUND)
            slog("addons: %ls: cannot list folder (error %lu)", query, e);
        return n;
    }
    int found = 0;
    do {
        size_t len = wcslen(fd.cFileName);
        if (!(fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) && len > 4 && !_wcsicmp(fd.cFileName + len - 4, L".dll") &&
            n < 256)
            out[n].root = root, out[n++].name = _wcsdup(fd.cFileName), found++;
    } while (FindNextFileW(f, &fd));
    FindClose(f);
    slog("addons: swofty\\%ls%ls%ls: %d dll(s)%s", ROOTS[root], *sub ? L"\\" : L"", sub, found,
         root ? " (legacy folder)" : "");
    return n;
}

static int is_early(const wchar_t *name)
{
    for (int i = 0; i < early_count; i++)
        if (!_wcsicmp(early_plugins[i], name))
            return 1;
    return 0;
}

static const wchar_t *base_of(const wchar_t *path)
{
    const wchar_t *s = wcsrchr(path, L'\\');
    return s ? s + 1 : path;
}

static int listed(const wchar_t *name, int before)
{
    for (int i = 0; i < before; i++)
        if (!_wcsicmp(base_of(plugins[i]), name))
            return 1;
    return 0;
}

static void load_plugins(int pick)
{
    for (int i = 0; i < plugin_count; i++) {
        const wchar_t *name = base_of(plugins[i]);
        wchar_t dir[MAX_PATH * 2];
        swprintf(dir, MAX_PATH * 2, L"%ls\\swofty\\%ls", game_dir, plugins[i]);
        dir[wcslen(dir) - wcslen(name) - 1] = 0;
        if (listed(name, i))
            slog("addon %ls: listed twice in settings.json, %ls skipped", name, plugins[i]);
        else if (pick == PICK_EARLY && !is_early(name))
            slog("addon %ls: loads at the window stage", name);
        else if (pick != PICK_REST || !is_early(name))
            load_addon(dir, name, 0);
    }
}

static void load_dir(const wchar_t *sub, int core_dir, int pick)
{
    Dll dlls[256];
    int n = 0;
    for (int r = 0; r < (int)(sizeof ROOTS / sizeof *ROOTS); r++)
        n = list_dlls(r, sub, dlls, n);
    if (!n)
        slog("addons: swofty\\addons%ls%ls: no .dll files", *sub ? L"\\" : L"", sub);
    qsort(dlls, n, sizeof *dlls, dll_cmp);
    for (int i = 0; i < n; i++) {
        wchar_t dir[MAX_PATH * 2];
        swprintf(dir, MAX_PATH * 2, L"%ls\\swofty\\%ls%ls%ls", game_dir, ROOTS[dlls[i].root], *sub ? L"\\" : L"", sub);
        if (i && !_wcsicmp(dlls[i].name, dlls[i - 1].name))
            slog("addon %ls: also in %ls, skipped", dlls[i].name, dir);
        else if (!core_dir && listed(dlls[i].name, plugin_count))
            slog("addon %ls: settings.json lists it in an addon folder, the copy in %ls is skipped", dlls[i].name, dir);
        else if (pick == PICK_EARLY && !is_early(dlls[i].name))
            slog("addon %ls: loads at the window stage", dlls[i].name);
        else if (pick != PICK_REST || !is_early(dlls[i].name))
            load_addon(dir, dlls[i].name, core_dir);
    }
    for (int i = 0; i < n; i++)
        free(dlls[i].name);
}

static BOOL CALLBACK find_window(HWND h, LPARAM out)
{
    DWORD pid = 0;
    GetWindowThreadProcessId(h, &pid);
    if (pid != GetCurrentProcessId() || !IsWindowVisible(h) || GetWindow(h, GW_OWNER))
        return TRUE;
    *(HWND *)out = h;
    return FALSE;
}

typedef ATOM(WINAPI *RegisterClassExWFn)(const WNDCLASSEXW *);
typedef HWND(WINAPI *CreateWindowExWFn)(DWORD, LPCWSTR, LPCWSTR, DWORD, int, int, int, int, HWND, HMENU, HINSTANCE,
                                         LPVOID);
typedef HANDLE(WINAPI *CreateMutexWFn)(LPSECURITY_ATTRIBUTES, BOOL, LPCWSTR);
static ATOM WINAPI trigger_RegisterClassExW(const WNDCLASSEXW *wc);
static HWND WINAPI trigger_CreateWindowExW(DWORD ex, LPCWSTR cls, LPCWSTR name, DWORD style, int x, int y, int w,
                                           int h, HWND parent, HMENU menu, HINSTANCE inst, LPVOID param);
static HANDLE WINAPI trigger_CreateMutexW(LPSECURITY_ATTRIBUTES sa, BOOL owner, LPCWSTR name);

#define STAGE_EARLY 1
#define STAGE_LATE 2
#define EARLY_MUTEX L"Codemasters F1 25"

static struct {
    const char *dll, *mod, *name;
    void *hook, **slot, *orig;
    int stage;
} iat[] = {
    {"USER32.dll", "USER32", "RegisterClassExW", (void *)trigger_RegisterClassExW, NULL, NULL, STAGE_LATE},
    {"USER32.dll", "USER32", "CreateWindowExW", (void *)trigger_CreateWindowExW, NULL, NULL, STAGE_LATE},
    {"KERNEL32.dll", "KERNEL32", "CreateMutexW", (void *)trigger_CreateMutexW, NULL, NULL, STAGE_EARLY},
};
#define NIAT (sizeof iat / sizeof *iat)
#define IAT_REGISTER 0
#define IAT_CREATE 1
#define IAT_MUTEX 2

static int set_slot(void **slot, void *value)
{
    DWORD old;
    if (!VirtualProtect(slot, sizeof *slot, PAGE_READWRITE, &old))
        return 0;
    InterlockedExchangePointer(slot, value);
    VirtualProtect(slot, sizeof *slot, old, &old);
    return 1;
}

static void hook_iat(void)
{
    const IMAGE_NT_HEADERS *nt = (const IMAGE_NT_HEADERS *)(image_base + ((const IMAGE_DOS_HEADER *)image_base)->e_lfanew);
    const IMAGE_DATA_DIRECTORY *dir = &nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];
    const IMAGE_DATA_DIRECTORY *iatdir = &nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IAT];
    uintptr_t iat_lo = image_base + iatdir->VirtualAddress, iat_hi = iat_lo + iatdir->Size;
    for (const IMAGE_IMPORT_DESCRIPTOR *d = (const IMAGE_IMPORT_DESCRIPTOR *)(image_base + dir->VirtualAddress);
         dir->VirtualAddress && d->Name; d++) {
        const char *dll = (const char *)(image_base + d->Name);
        if (!d->OriginalFirstThunk)
            continue;
        const IMAGE_THUNK_DATA64 *names = (const IMAGE_THUNK_DATA64 *)(image_base + d->OriginalFirstThunk);
        void **slots = (void **)(image_base + d->FirstThunk);
        for (int i = 0; names[i].u1.AddressOfData; i++) {
            if (IMAGE_SNAP_BY_ORDINAL64(names[i].u1.Ordinal))
                continue;
            const char *fn = (const char *)((const IMAGE_IMPORT_BY_NAME *)(image_base + names[i].u1.AddressOfData))->Name;
            for (size_t k = 0; k < NIAT; k++)
                if (!iat[k].slot && !strcmp(dll, iat[k].dll) && !strcmp(fn, iat[k].name) &&
                    (!iatdir->Size || ((uintptr_t)&slots[i] >= iat_lo && (uintptr_t)&slots[i] < iat_hi))) {
                    iat[k].slot = &slots[i];
                    iat[k].orig = slots[i];
                }
        }
    }
    for (size_t k = 0; k < NIAT; k++) {
        if (iat[k].stage == STAGE_EARLY && !early_enabled)
            continue;
        if (iat[k].slot && iat[k].orig && set_slot(iat[k].slot, iat[k].hook))
            slog("trigger armed: game IAT slot %s!%s at %p (was %p)%s", iat[k].mod, iat[k].name, (void *)iat[k].slot,
                 iat[k].orig, iat[k].stage == STAGE_EARLY ? ", early stage" : "");
        else
            slog("trigger: game IAT slot %s!%s not found or not writable%s", iat[k].mod, iat[k].name,
                 iat[k].stage == STAGE_EARLY ? "; the early stage cannot fire" : "");
    }
}

static void unhook_iat(int stage)
{
    for (size_t k = 0; k < NIAT; k++)
        if ((!stage || iat[k].stage == stage) && iat[k].slot && *iat[k].slot == iat[k].hook &&
            set_slot(iat[k].slot, iat[k].orig))
            slog("trigger disarmed: %s!%s slot restored to %p", iat[k].mod, iat[k].name, iat[k].orig);
}

static const Section *section_of(uintptr_t a)
{
    for (int i = 0; i < section_count; i++)
        if (a - sections[i].start < sections[i].size)
            return &sections[i];
    return NULL;
}

static void finish_batch(int first)
{
    if (core_commit)
        slog("addons: mixin commit done, %d mixin addon(s) active", core_commit());
    for (int i = first; i < deferred_count; i++) {
        slog("addon %ls: calling %s (API v1)", deferred[i].name, deferred[i].symbol);
        finish_init(deferred[i].name, deferred[i].symbol, deferred[i].init(&api_v1));
    }
    slog("addons: done, %d addon(s) loaded, %d initialised", addons_loaded, addons_ready);
}

static void load_addons_once(const char *why, LPCWSTR detail, int stage)
{
    if (InterlockedCompareExchange(&load_state, 1, 0) != 0)
        return;
    load_thread = GetCurrentThreadId();
    if (detail && !((uintptr_t)detail >> 16))
        detail = NULL;
    loaded_early = stage == STAGE_EARLY;
    slog("trigger: %s%s%ls%s on thread %lu, %lu ms after attach; loading addons now%s", why,
         detail ? (loaded_early ? " name '" : " class '") : "", detail ? detail : L"", detail ? "'" : "",
         GetCurrentThreadId(), GetTickCount() - attach_tick, loaded_early ? " (early stage)" : "");
    if (early_enabled && !loaded_early)
        slog("early stage: the early anchor never fired; early-stage mixins (game database, config and file-system "
             "overlay of files read before the window) missed their first calls");
    SetEnvironmentVariableW(L"SWOFTY_LOAD_STAGE", loaded_early ? L"early" : L"window");
    unhook_iat(loaded_early ? STAGE_EARLY : STAGE_LATE);
    load_dir(L"core", 1, PICK_ALL);
    load_plugins(loaded_early ? PICK_EARLY : PICK_ALL);
    load_dir(L"", 0, loaded_early ? PICK_EARLY : PICK_ALL);
    finish_batch(0);
    if (!loaded_early)
        unhook_iat(0);
    InterlockedExchange(&load_state, 2);
    if (loaded_event)
        SetEvent(loaded_event);
}

static void load_rest(void)
{
    int first = deferred_count;
    slog("addons: loading the addons without \"early\": true at the window stage");
    load_plugins(PICK_REST);
    load_dir(L"", 0, PICK_REST);
    finish_batch(first);
}

static void wait_loaded(const char *who)
{
    if (load_state != 1 || !loaded_event || load_thread == GetCurrentThreadId())
        return;
    DWORD t0 = GetTickCount();
    DWORD r = WaitForSingleObject(loaded_event, 60000);
    slog("%s: waited %lu ms for the addon load running on another thread (%s)", who, GetTickCount() - t0,
         r == WAIT_OBJECT_0 ? "done" : "timed out");
}

static void stage_window(const char *why)
{
    if (!early_enabled || load_state != 2 || InterlockedExchange(&window_done, 1))
        return;
    unhook_iat(0);
    slog("stage: window (%s), addons were loaded at the %s stage", why, loaded_early ? "early" : "window");
    if (loaded_early)
        load_rest();
    if (core_stage)
        core_stage(SWOFTY_STAGE_WINDOW);
}

static int plain_code(const Section *sec)
{
    return sec && (sec->flags & IMAGE_SCN_MEM_EXECUTE) && !(sec->flags & IMAGE_SCN_MEM_WRITE);
}

static void game_call(const char *fn, void *ret, LPCWSTR cls)
{
    const Section *sec = section_of((uintptr_t)ret);
    char why[160];
    snprintf(why, sizeof why, "USER32!%s from %p (%s)", fn, ret, sec ? sec->name : "outside the exe");
    if (plain_code(sec)) {
        load_addons_once(why, cls, STAGE_LATE);
        if (early_enabled) {
            wait_loaded("trigger");
            stage_window(why);
        }
    } else if (early_enabled ? load_state != 2 : !load_state)
        slog("trigger skipped: %s is not plain game code, still waiting", why);
}

static ATOM WINAPI trigger_RegisterClassExW(const WNDCLASSEXW *wc)
{
    game_call("RegisterClassExW", __builtin_return_address(0), wc ? wc->lpszClassName : NULL);
    return ((RegisterClassExWFn)iat[IAT_REGISTER].orig)(wc);
}

static HWND WINAPI trigger_CreateWindowExW(DWORD ex, LPCWSTR cls, LPCWSTR name, DWORD style, int x, int y, int w,
                                           int h, HWND parent, HMENU menu, HINSTANCE inst, LPVOID param)
{
    game_call("CreateWindowExW", __builtin_return_address(0), cls);
    return ((CreateWindowExWFn)iat[IAT_CREATE].orig)(ex, cls, name, style, x, y, w, h, parent, menu, inst, param);
}

static HANDLE WINAPI trigger_CreateMutexW(LPSECURITY_ATTRIBUTES sa, BOOL owner, LPCWSTR name)
{
    static volatile LONG skips;
    void *ret = __builtin_return_address(0);
    if (load_state != 2) {
        const Section *sec = section_of((uintptr_t)ret);
        int named = name && !wcscmp(name, EARLY_MUTEX);
        if (named && plain_code(sec)) {
            char why[160];
            snprintf(why, sizeof why, "KERNEL32!CreateMutexW from %p (%s)", ret, sec->name);
            load_addons_once(why, name, STAGE_EARLY);
            wait_loaded("early trigger");
        } else if (InterlockedIncrement(&skips) <= 8)
            slog("early trigger skipped: KERNEL32!CreateMutexW from %p (%s) name '%ls' is not the game's single-instance "
                 "mutex from plain game code",
                 ret, sec ? sec->name : "outside the exe", name ? name : L"(none)");
    }
    return ((CreateMutexWFn)iat[IAT_MUTEX].orig)(sa, owner, name);
}

static int early_flag(void)
{
    wchar_t path[MAX_PATH * 2];
    swprintf(path, MAX_PATH * 2, L"%ls\\swofty\\assets.json", game_dir);
    HANDLE f = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL);
    if (f == INVALID_HANDLE_VALUE)
        return -1;
    static char buf[1 << 20];
    DWORD got = 0;
    ReadFile(f, buf, sizeof buf - 1, &got, NULL);
    CloseHandle(f);
    buf[got] = 0;
    const char *p = strstr(buf, "\"early_stage\"");
    if (!p)
        return -1;
    p += 13;
    while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n')
        p++;
    if (*p++ != ':')
        return -1;
    while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n')
        p++;
    return !strncmp(p, "true", 4) ? 1 : !strncmp(p, "false", 5) ? 0 : -1;
}

static const char *json_list(const char *buf, const char *key)
{
    size_t k = strlen(key);
    for (const char *p = buf; (p = strstr(p, key)); p += k) {
        const char *q = p + k;
        while (*q == ' ' || *q == '\t' || *q == '\r' || *q == '\n')
            q++;
        if (*q++ != ':')
            continue;
        while (*q == ' ' || *q == '\t' || *q == '\r' || *q == '\n')
            q++;
        if (*q == '[')
            return q + 1;
    }
    return NULL;
}

static int read_list(const char *buf, const char *key, wchar_t (*out)[MAX_PATH], int max)
{
    int count = 0;
    for (const char *p = json_list(buf, key); p && *p && *p != ']' && count < max;) {
        const char *q = strchr(p, '"'), *e = q ? strchr(q + 1, '"') : NULL;
        const char *close = strchr(p, ']');
        if (!e || (close && q > close))
            break;
        int n = MultiByteToWideChar(CP_UTF8, 0, q + 1, (int)(e - q - 1), out[count], MAX_PATH - 1);
        out[count][n] = 0;
        if (n)
            count++;
        p = e + 1;
    }
    return count;
}

static int inside(wchar_t *path)
{
    wchar_t wrapped[MAX_PATH + 2];
    for (wchar_t *c = path; *c; c++)
        if (*c == L'/')
            *c = L'\\';
    swprintf(wrapped, MAX_PATH + 2, L"\\%ls\\", path);
    return path[0] != L'\\' && !wcschr(path, L':') && !wcsstr(wrapped, L"\\..\\") && !wcsstr(wrapped, L"\\.\\");
}

static void read_settings(void)
{
    wchar_t path[MAX_PATH * 2];
    swprintf(path, MAX_PATH * 2, L"%ls\\swofty\\settings.json", game_dir);
    HANDLE f = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL);
    if (f == INVALID_HANDLE_VALUE)
        return;
    static char buf[4 << 20];
    DWORD got = 0;
    ReadFile(f, buf, sizeof buf - 1, &got, NULL);
    CloseHandle(f);
    buf[got] = 0;
    early_count = read_list(buf, "\"early_plugins\"", early_plugins, 64);
    for (int i = 0; i < early_count; i++)
        slog("early stage: plugin %ls has \"early\": true", early_plugins[i]);
    int n = read_list(buf, "\"plugins\"", plugins, 256);
    for (int i = 0; i < n; i++) {
        if (!inside(plugins[i]))
            slog("addons: plugin %ls in settings.json is not a path inside swofty\\, skipped", plugins[i]);
        else if (plugin_count++ < i)
            wcscpy(plugins[plugin_count - 1], plugins[i]);
    }
    if (plugin_count)
        slog("addons: settings.json lists %d plugin(s) in addon folders", plugin_count);
}

static void read_early_flag(void)
{
    char env[8] = {0};
    if (GetEnvironmentVariableA("SWOFTY_EARLY_STAGE", env, sizeof env) && env[0]) {
        early_enabled = env[0] == '1' || env[0] == 't' || env[0] == 'T';
        early_env = 1;
        snprintf(early_source, sizeof early_source, "%s (SWOFTY_EARLY_STAGE=%s)", early_enabled ? "on" : "off", env);
        return;
    }
    int v = early_flag();
    if (v < 0) {
        snprintf(early_source, sizeof early_source, "off (no \"early_stage\" in swofty\\assets.json)");
        return;
    }
    early_enabled = v;
    snprintf(early_source, sizeof early_source, "%s (\"early_stage\": %s in swofty\\assets.json)", v ? "on" : "off",
             v ? "true" : "false");
}

static DWORD WINAPI worker(LPVOID unused)
{
    (void)unused;
    slog("fallback watcher: waiting for a visible main window if no trigger fires (timeout %d s)",
         WINDOW_TIMEOUT_MS / 1000);
    while (!load_state) {
        HWND w = NULL;
        EnumWindows(find_window, (LPARAM)&w);
        if (w) {
            wchar_t cls[128] = {0};
            GetClassNameW(w, cls, 128);
            load_addons_once("main window visible (fallback, IAT trigger never fired)", cls, STAGE_LATE);
            stage_window("main window visible (fallback)");
            return 0;
        }
        if (GetTickCount() - attach_tick > WINDOW_TIMEOUT_MS &&
            InterlockedCompareExchange(&load_state, 3, 0) == 0) {
            unhook_iat(0);
            slog("fallback watcher: no trigger and no window after %d s, addons NOT loaded", WINDOW_TIMEOUT_MS / 1000);
            return 0;
        }
        Sleep(POLL_MS);
    }
    slog("fallback watcher: trigger already fired, exiting");
    return 0;
}

static void capture_sections(void)
{
    image_base = (uintptr_t)GetModuleHandleW(NULL);
    const IMAGE_NT_HEADERS *nt = (const IMAGE_NT_HEADERS *)(image_base + ((const IMAGE_DOS_HEADER *)image_base)->e_lfanew);
    const IMAGE_SECTION_HEADER *s = IMAGE_FIRST_SECTION(nt);
    for (int i = 0; i < nt->FileHeader.NumberOfSections && section_count < 96; i++, s++) {
        Section *d = &sections[section_count++];
        memcpy(d->name, s->Name, 8);
        d->name[8] = 0;
        d->start = image_base + s->VirtualAddress;
        d->size = s->Misc.VirtualSize ? s->Misc.VirtualSize : s->SizeOfRawData;
        d->flags = s->Characteristics;
    }
}

static void open_log(void)
{
    wchar_t dir[MAX_PATH * 2], path[MAX_PATH * 2], prev[MAX_PATH * 2];
    swprintf(dir, MAX_PATH * 2, L"%ls\\swofty", game_dir);
    CreateDirectoryW(dir, NULL);
    swprintf(path, MAX_PATH * 2, L"%ls\\loader.log", dir);
    swprintf(prev, MAX_PATH * 2, L"%ls\\loader.prev.log", dir);
    MoveFileExW(path, prev, MOVEFILE_REPLACE_EXISTING);
    log_file = CreateFileW(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, CREATE_ALWAYS,
                           FILE_ATTRIBUTE_NORMAL, NULL);
}

static void log_build(void)
{
    const IMAGE_NT_HEADERS *nt = (const IMAGE_NT_HEADERS *)(image_base + ((const IMAGE_DOS_HEADER *)image_base)->e_lfanew);
    WIN32_FILE_ATTRIBUTE_DATA fa;
    unsigned long long size = 0;
    if (GetFileAttributesExW(exe_path, GetFileExInfoStandard, &fa))
        size = ((unsigned long long)fa.nFileSizeHigh << 32) | fa.nFileSizeLow;
    unsigned long stamp = nt->FileHeader.TimeDateStamp;
    int ok = size == EXPECTED_SIZE && stamp == EXPECTED_STAMP;
    slog("exe: size %llu (analysed %llu), PE timestamp %lu (analysed %lu) -> %s", size, EXPECTED_SIZE, stamp,
         EXPECTED_STAMP, ok ? "MATCHES the analysed build" : "DIFFERS from the analysed build, signatures may fail");
    slog("exe: image base %p, %d sections", (void *)image_base, section_count);
    for (int i = 0; i < section_count; i++)
        slog("  section %-8s %p size 0x%llx flags 0x%08lx", sections[i].name, (void *)sections[i].start,
             (unsigned long long)sections[i].size, sections[i].flags);
}

BOOL WINAPI DllMain(HINSTANCE self, DWORD reason, LPVOID reserved)
{
    (void)reserved;
    if (reason != DLL_PROCESS_ATTACH)
        return TRUE;
    DisableThreadLibraryCalls(self);

    wchar_t sys[MAX_PATH * 2];
    UINT len = GetSystemDirectoryW(sys, MAX_PATH);
    wcscpy(sys + len, L"\\version.dll");
    HMODULE real = LoadLibraryW(sys);
    if (real == self)
        real = NULL;
    int missing = 0;
    for (size_t i = 0; i < NFWD; i++) {
        swofty_fwd[i] = real ? (void *)GetProcAddress(real, fwd_names[i]) : NULL;
        if (!swofty_fwd[i]) {
            swofty_fwd[i] = (void *)fwd_missing;
            missing++;
        }
    }

    GetModuleFileNameW(NULL, exe_path, MAX_PATH);
    wchar_t *slash = wcsrchr(exe_path, L'\\');
    const wchar_t *exe_name = slash ? slash + 1 : exe_path;
    if (_wcsicmp(exe_name, HOST_EXE) != 0)
        return TRUE;

    wcscpy(game_dir, exe_path);
    game_dir[slash ? slash - exe_path : 0] = 0;
    InitializeCriticalSection(&log_lock);
    open_log();
    capture_sections();

    wchar_t self_path[MAX_PATH] = L"?", real_path[MAX_PATH] = L"(not loaded)";
    GetModuleFileNameW(self, self_path, MAX_PATH);
    if (real)
        GetModuleFileNameW(real, real_path, MAX_PATH);
    slog("swofty loader (version.dll proxy) attached, pid %lu", GetCurrentProcessId());
    slog("host: %ls", exe_path);
    slog("proxy: %ls", self_path);
    slog("real version.dll: %ls (requested %ls)", real_path, sys);
    slog("forwards: %d of %d resolved", (int)NFWD - missing, (int)NFWD);
    for (size_t i = 0; i < NFWD; i++)
        if (swofty_fwd[i] == (void *)fwd_missing)
            slog("  forward %s missing in the real dll (calls return 0)", fwd_names[i]);
    log_build();
    attach_tick = GetTickCount();
    read_early_flag();
    slog("early stage: %s", early_source);
    read_settings();
    if (early_count && !early_enabled && !early_env) {
        early_enabled = 1;
        slog("early stage: on for %d early plugin(s); SwoftyMod loads with them", early_count);
    }
    if (early_enabled)
        loaded_event = CreateEventW(NULL, TRUE, FALSE, NULL);
    hook_iat();

    HANDLE t = CreateThread(NULL, 0, worker, NULL, 0, NULL);
    if (t) {
        slog("fallback watcher thread started");
        CloseHandle(t);
    } else {
        slog("CreateThread failed, error %lu; relying on the IAT trigger only", GetLastError());
    }
    return TRUE;
}
