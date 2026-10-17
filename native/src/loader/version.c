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
static int section_count, mods_loaded, mods_ready;
static volatile LONG load_state;
static DWORD attach_tick;

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

static const SwoftyApi api = {SWOFTY_API_VERSION, slog, module_base, scan, NULL, NULL, NULL, NULL, NULL};

static int wcmp(const void *a, const void *b)
{
    return _wcsicmp(*(wchar_t *const *)a, *(wchar_t *const *)b);
}

static void load_mod(const wchar_t *dir, const wchar_t *name)
{
    wchar_t path[MAX_PATH * 2];
    swprintf(path, MAX_PATH * 2, L"%ls\\%ls", dir, name);
    slog("mod %ls: loading %ls", name, path);
    HMODULE m = LoadLibraryW(path);
    if (!m) {
        slog("mod %ls: LoadLibrary failed, error %lu", name, GetLastError());
        return;
    }
    mods_loaded++;
    SwoftyModInitFn init = (SwoftyModInitFn)(void *)GetProcAddress(m, "SwoftyModInit");
    if (!init) {
        slog("mod %ls: loaded at %p but exports no SwoftyModInit, left idle", name, (void *)m);
        return;
    }
    slog("mod %ls: loaded at %p, calling SwoftyModInit", name, (void *)m);
    int r = init(&api);
    slog("mod %ls: SwoftyModInit returned %d (%s)", name, r, r ? "ok" : "FAILED");
    if (r)
        mods_ready++;
}

static void load_dir(const wchar_t *sub)
{
    wchar_t dir[MAX_PATH * 2], query[MAX_PATH * 2], *names[256];
    swprintf(dir, MAX_PATH * 2, L"%ls\\swofty\\%ls", game_dir, sub);
    swprintf(query, MAX_PATH * 2, L"%ls\\*.dll", dir);
    WIN32_FIND_DATAW fd;
    HANDLE f = FindFirstFileW(query, &fd);
    if (f == INVALID_HANDLE_VALUE) {
        DWORD e = GetLastError();
        slog(e == ERROR_FILE_NOT_FOUND ? "mods: %ls: no .dll files" : "mods: %ls: cannot list folder (error %lu)", dir, e);
        return;
    }
    int n = 0;
    do {
        size_t len = wcslen(fd.cFileName);
        if (!(fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) && len > 4 && !_wcsicmp(fd.cFileName + len - 4, L".dll") &&
            n < 256)
            names[n++] = _wcsdup(fd.cFileName);
    } while (FindNextFileW(f, &fd));
    FindClose(f);
    qsort(names, n, sizeof *names, wcmp);
    slog("mods: %ls: %d dll(s)", dir, n);
    for (int i = 0; i < n; i++) {
        load_mod(dir, names[i]);
        free(names[i]);
    }
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
static ATOM WINAPI trigger_RegisterClassExW(const WNDCLASSEXW *wc);
static HWND WINAPI trigger_CreateWindowExW(DWORD ex, LPCWSTR cls, LPCWSTR name, DWORD style, int x, int y, int w,
                                           int h, HWND parent, HMENU menu, HINSTANCE inst, LPVOID param);

static struct {
    const char *name;
    void *hook, **slot, *orig;
} iat[] = {
    {"RegisterClassExW", (void *)trigger_RegisterClassExW, NULL, NULL},
    {"CreateWindowExW", (void *)trigger_CreateWindowExW, NULL, NULL},
};
#define NIAT (sizeof iat / sizeof *iat)

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
    for (const IMAGE_IMPORT_DESCRIPTOR *d = (const IMAGE_IMPORT_DESCRIPTOR *)(image_base + dir->VirtualAddress);
         dir->VirtualAddress && d->Name; d++) {
        if (strcmp((const char *)(image_base + d->Name), "USER32.dll") || !d->OriginalFirstThunk)
            continue;
        const IMAGE_THUNK_DATA64 *names = (const IMAGE_THUNK_DATA64 *)(image_base + d->OriginalFirstThunk);
        void **slots = (void **)(image_base + d->FirstThunk);
        for (int i = 0; names[i].u1.AddressOfData; i++) {
            if (IMAGE_SNAP_BY_ORDINAL64(names[i].u1.Ordinal))
                continue;
            const char *fn = (const char *)((const IMAGE_IMPORT_BY_NAME *)(image_base + names[i].u1.AddressOfData))->Name;
            for (size_t k = 0; k < NIAT; k++)
                if (!iat[k].slot && !strcmp(fn, iat[k].name)) {
                    iat[k].slot = &slots[i];
                    iat[k].orig = slots[i];
                }
        }
    }
    for (size_t k = 0; k < NIAT; k++) {
        if (iat[k].slot && iat[k].orig && set_slot(iat[k].slot, iat[k].hook))
            slog("trigger armed: game IAT slot USER32!%s at %p (was %p)", iat[k].name, (void *)iat[k].slot, iat[k].orig);
        else
            slog("trigger: game IAT slot USER32!%s not found or not writable", iat[k].name);
    }
}

static void unhook_iat(void)
{
    for (size_t k = 0; k < NIAT; k++)
        if (iat[k].slot && *iat[k].slot == iat[k].hook && set_slot(iat[k].slot, iat[k].orig))
            slog("trigger disarmed: USER32!%s slot restored to %p", iat[k].name, iat[k].orig);
}

static const Section *section_of(uintptr_t a)
{
    for (int i = 0; i < section_count; i++)
        if (a - sections[i].start < sections[i].size)
            return &sections[i];
    return NULL;
}

static void load_mods_once(const char *why, LPCWSTR detail)
{
    if (InterlockedCompareExchange(&load_state, 1, 0) != 0)
        return;
    if (detail && !((uintptr_t)detail >> 16))
        detail = NULL;
    slog("trigger: %s%s%ls%s on thread %lu, %lu ms after attach; loading mods now", why, detail ? " class '" : "",
         detail ? detail : L"", detail ? "'" : "", GetCurrentThreadId(), GetTickCount() - attach_tick);
    unhook_iat();
    load_dir(L"mods\\core");
    load_dir(L"mods");
    slog("mods: done, %d mod(s) loaded, %d initialised", mods_loaded, mods_ready);
    InterlockedExchange(&load_state, 2);
}

static void game_call(const char *fn, void *ret, LPCWSTR cls)
{
    const Section *sec = section_of((uintptr_t)ret);
    char why[160];
    snprintf(why, sizeof why, "USER32!%s from %p (%s)", fn, ret, sec ? sec->name : "outside the exe");
    if (sec && (sec->flags & IMAGE_SCN_MEM_EXECUTE) && !(sec->flags & IMAGE_SCN_MEM_WRITE))
        load_mods_once(why, cls);
    else if (!load_state)
        slog("trigger skipped: %s is not plain game code, still waiting", why);
}

static ATOM WINAPI trigger_RegisterClassExW(const WNDCLASSEXW *wc)
{
    game_call("RegisterClassExW", __builtin_return_address(0), wc ? wc->lpszClassName : NULL);
    return ((RegisterClassExWFn)iat[0].orig)(wc);
}

static HWND WINAPI trigger_CreateWindowExW(DWORD ex, LPCWSTR cls, LPCWSTR name, DWORD style, int x, int y, int w,
                                           int h, HWND parent, HMENU menu, HINSTANCE inst, LPVOID param)
{
    game_call("CreateWindowExW", __builtin_return_address(0), cls);
    return ((CreateWindowExWFn)iat[1].orig)(ex, cls, name, style, x, y, w, h, parent, menu, inst, param);
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
            load_mods_once("main window visible (fallback, IAT trigger never fired)", cls);
            return 0;
        }
        if (GetTickCount() - attach_tick > WINDOW_TIMEOUT_MS &&
            InterlockedCompareExchange(&load_state, 3, 0) == 0) {
            unhook_iat();
            slog("fallback watcher: no trigger and no window after %d s, mods NOT loaded", WINDOW_TIMEOUT_MS / 1000);
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
