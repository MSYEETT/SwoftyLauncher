#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <algorithm>
#include <cstdarg>
#include <cstdio>
#include <string>
#include <vector>
#include "safetyhook.hpp"
#include "swofty_api.h"

std::string core_code_owner(uintptr_t addr);

namespace {

constexpr LONG MAX_REPORTS = 16;
constexpr int MAX_FRAMES = 32;
constexpr LONG MAX_SEEN = 256;

struct Symbol {
    uint32_t rva;
    std::string name;
};

SwoftyApi *api;
HMODULE exe, self;
DWORD start_tick;
std::vector<Symbol> symbols;
LPTOP_LEVEL_EXCEPTION_FILTER previous;
safetyhook::InlineHook exit_hook, terminate_hook;
uintptr_t seen[MAX_SEEN];
volatile LONG seen_count, reports, busy, exited;
bool ea_build;
std::wstring diag_path;

std::string fmt(const char *f, ...)
{
    char b[512];
    va_list ap;
    va_start(ap, f);
    vsnprintf(b, sizeof b, f, ap);
    va_end(ap);
    return b;
}

void diag_line(const std::string &s)
{
    if (!ea_build || diag_path.empty())
        return;
    HANDLE f = CreateFileW(diag_path.c_str(), FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_ALWAYS,
                           FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return;
    SYSTEMTIME t;
    GetLocalTime(&t);
    char line[1200];
    int n = snprintf(line, sizeof line, "%04u-%02u-%02u %02u:%02u:%02u.%03u [%5lu] %s\r\n", t.wYear, t.wMonth, t.wDay,
                     t.wHour, t.wMinute, t.wSecond, t.wMilliseconds, GetCurrentThreadId(), s.c_str());
    if (n > 0) {
        DWORD w;
        WriteFile(f, line, (DWORD)(n > (int)sizeof line ? (int)sizeof line : n), &w, nullptr);
    }
    CloseHandle(f);
}

/* Like api->log, but on a non-matching (EA/other) build also mirrors the line into the dedicated
   swofty\ea-diagnostics.log, so the friend's single file carries the symbolised crash report. */
void cl(const char *f, ...)
{
    char b[1024];
    va_list ap;
    va_start(ap, f);
    vsnprintf(b, sizeof b, f, ap);
    va_end(ap);
    api->log("%s", b);
    diag_line(b);
}

bool readable(uintptr_t a, size_t n)
{
    for (uintptr_t p = a & ~0xfffull; p < a + n; p += 0x1000) {
        MEMORY_BASIC_INFORMATION m;
        if (!VirtualQuery(reinterpret_cast<void *>(p), &m, sizeof m) || m.State != MEM_COMMIT ||
            (m.Protect & (PAGE_GUARD | PAGE_NOACCESS)) || !(m.Protect & 0xEE))
            return false;
    }
    return true;
}

const char *code_name(DWORD c)
{
    switch (c) {
    case EXCEPTION_ACCESS_VIOLATION: return "ACCESS_VIOLATION";
    case EXCEPTION_IN_PAGE_ERROR: return "IN_PAGE_ERROR";
    case EXCEPTION_ILLEGAL_INSTRUCTION: return "ILLEGAL_INSTRUCTION";
    case EXCEPTION_PRIV_INSTRUCTION: return "PRIV_INSTRUCTION";
    case EXCEPTION_INT_DIVIDE_BY_ZERO: return "INT_DIVIDE_BY_ZERO";
    case EXCEPTION_INT_OVERFLOW: return "INT_OVERFLOW";
    case EXCEPTION_STACK_OVERFLOW: return "STACK_OVERFLOW";
    case EXCEPTION_ARRAY_BOUNDS_EXCEEDED: return "ARRAY_BOUNDS_EXCEEDED";
    case EXCEPTION_NONCONTINUABLE_EXCEPTION: return "NONCONTINUABLE_EXCEPTION";
    case EXCEPTION_DATATYPE_MISALIGNMENT: return "DATATYPE_MISALIGNMENT";
    case 0xC0000374: return "HEAP_CORRUPTION";
    case 0xC0000409: return "STACK_BUFFER_OVERRUN";
    case 0xE06D7363: return "C++ exception";
    }
    return "exception";
}

bool fatal(DWORD c)
{
    return c == EXCEPTION_ACCESS_VIOLATION || c == EXCEPTION_IN_PAGE_ERROR || c == EXCEPTION_ILLEGAL_INSTRUCTION ||
           c == EXCEPTION_PRIV_INSTRUCTION || c == EXCEPTION_INT_DIVIDE_BY_ZERO || c == EXCEPTION_INT_OVERFLOW ||
           c == EXCEPTION_STACK_OVERFLOW || c == EXCEPTION_ARRAY_BOUNDS_EXCEEDED ||
           c == EXCEPTION_NONCONTINUABLE_EXCEPTION || c == EXCEPTION_DATATYPE_MISALIGNMENT || c == 0xC0000374 ||
           c == 0xC0000409;
}

std::string module_name(HMODULE m)
{
    char path[MAX_PATH] = "?";
    GetModuleFileNameA(m, path, MAX_PATH);
    const char *slash = strrchr(path, '\\');
    return slash ? slash + 1 : path;
}

std::string section_name(uintptr_t a)
{
    auto base = reinterpret_cast<uintptr_t>(exe);
    auto nt = reinterpret_cast<const IMAGE_NT_HEADERS *>(base + reinterpret_cast<const IMAGE_DOS_HEADER *>(base)->e_lfanew);
    auto s = IMAGE_FIRST_SECTION(nt);
    for (int i = 0; i < nt->FileHeader.NumberOfSections; i++, s++)
        if (a - (base + s->VirtualAddress) < (s->Misc.VirtualSize ? s->Misc.VirtualSize : s->SizeOfRawData))
            return std::string(reinterpret_cast<const char *>(s->Name), strnlen(reinterpret_cast<const char *>(s->Name), 8));
    return "?";
}

PRUNTIME_FUNCTION primary(PRUNTIME_FUNCTION f, DWORD64 base)
{
    for (int i = 0; i < 8; i++) {
        auto ui = reinterpret_cast<const uint8_t *>(base + f->UnwindData);
        if (!readable(reinterpret_cast<uintptr_t>(ui), 4) || !((ui[0] >> 3) & UNW_FLAG_CHAININFO))
            break;
        f = reinterpret_cast<PRUNTIME_FUNCTION>(const_cast<uint8_t *>(ui) + 4 + 2 * ((ui[2] + 1) & ~1));
    }
    return f;
}

std::string where(uintptr_t a)
{
    HMODULE m = nullptr;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                            reinterpret_cast<LPCWSTR>(a), &m) || !m) {
        std::string owner = core_code_owner(a);
        return fmt("0x%llx", (unsigned long long)a) + (owner.empty() ? " (no module)" : " (" + owner + ")");
    }
    auto mb = reinterpret_cast<uintptr_t>(m);
    std::string s = fmt("%s+0x%llx", module_name(m).c_str(), (unsigned long long)(a - mb));
    if (m == exe)
        s += " " + section_name(a);
    DWORD64 ib = 0;
    auto f = RtlLookupFunctionEntry(a, &ib, nullptr);
    if (!f)
        return s;
    f = primary(f, ib);
    uint32_t start = f->BeginAddress, off = uint32_t(a - ib - start);
    if (m != exe) {
        std::string owner = core_code_owner(ib + start);
        return owner.empty() ? s : s + " (" + owner + fmt("+0x%x)", off);
    }
    auto it = std::lower_bound(symbols.begin(), symbols.end(), start, [](const Symbol &x, uint32_t r) { return x.rva < r; });
    if (it != symbols.end() && it->rva == start)
        return s + fmt(" %s+0x%x", it->name.c_str(), off);
    return s + fmt(" fn base+0x%x+0x%x", start, off);
}

void backtrace(CONTEXT ctx, bool skip_self)
{
    auto tib = reinterpret_cast<NT_TIB *>(NtCurrentTeb());
    auto lo = reinterpret_cast<uintptr_t>(tib->StackLimit), hi = reinterpret_cast<uintptr_t>(tib->StackBase);
    for (int i = 0, shown = 0; i < MAX_FRAMES && ctx.Rip; i++) {
        HMODULE m = nullptr;
        GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                           reinterpret_cast<LPCWSTR>(ctx.Rip), &m);
        if (!skip_self || m != self)
            cl("[crash]   #%-2d %s", shown++, where(ctx.Rip).c_str());
        if (ctx.Rsp < lo || ctx.Rsp >= hi)
            break;
        DWORD64 ib = 0;
        auto f = RtlLookupFunctionEntry(ctx.Rip, &ib, nullptr);
        if (f) {
            void *hd;
            DWORD64 ef;
            RtlVirtualUnwind(UNW_FLAG_NHANDLER, ib, ctx.Rip, f, &ctx, &hd, &ef, nullptr);
        } else {
            if (!readable(ctx.Rsp, 8))
                break;
            ctx.Rip = *reinterpret_cast<DWORD64 *>(ctx.Rsp);
            ctx.Rsp += 8;
        }
    }
}

bool first_time(uintptr_t ip)
{
    LONG n = seen_count < MAX_SEEN ? seen_count : MAX_SEEN;
    for (LONG i = 0; i < n; i++)
        if (seen[i] == ip)
            return false;
    LONG i = InterlockedIncrement(&seen_count) - 1;
    if (i < MAX_SEEN)
        seen[i] = ip;
    else if (i == MAX_SEEN)
        cl("[crash] %ld distinct fault addresses seen; no further first-chance exceptions are logged", MAX_SEEN);
    return i < MAX_SEEN;
}

void report(const char *when, EXCEPTION_POINTERS *e)
{
    auto r = e->ExceptionRecord;
    auto c = e->ContextRecord;
    LONG n = InterlockedIncrement(&reports);
    cl("[crash] #%ld %s %s (0x%08lx) on thread %lu, %lu s after load, at %s", n, when, code_name(r->ExceptionCode),
             r->ExceptionCode, GetCurrentThreadId(), (GetTickCount() - start_tick) / 1000, where(c->Rip).c_str());
    if ((r->ExceptionCode == EXCEPTION_ACCESS_VIOLATION || r->ExceptionCode == EXCEPTION_IN_PAGE_ERROR) &&
        r->NumberParameters >= 2) {
        auto op = r->ExceptionInformation[0];
        cl("[crash]   %s 0x%llx", op == 0 ? "reading" : op == 1 ? "writing" : op == 8 ? "executing (DEP)" : "accessing",
                 (unsigned long long)r->ExceptionInformation[1]);
    }
    if (r->ExceptionCode == EXCEPTION_STACK_OVERFLOW)
        return;
    cl("[crash]   rax=%016llx rbx=%016llx rcx=%016llx rdx=%016llx", c->Rax, c->Rbx, c->Rcx, c->Rdx);
    cl("[crash]   rsi=%016llx rdi=%016llx rbp=%016llx rsp=%016llx", c->Rsi, c->Rdi, c->Rbp, c->Rsp);
    cl("[crash]   r8 =%016llx r9 =%016llx r10=%016llx r11=%016llx", c->R8, c->R9, c->R10, c->R11);
    cl("[crash]   r12=%016llx r13=%016llx r14=%016llx r15=%016llx", c->R12, c->R13, c->R14, c->R15);
    if (readable(c->Rip, 16)) {
        auto b = reinterpret_cast<const uint8_t *>(c->Rip);
        cl("[crash]   code: %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X", b[0],
                 b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]);
    }
    cl("[crash]   stack:");
    backtrace(*c, false);
}

bool enter()
{
    return InterlockedCompareExchange(&busy, (LONG)GetCurrentThreadId(), 0) == 0;
}

void leave()
{
    InterlockedExchange(&busy, 0);
}

LONG CALLBACK veh(EXCEPTION_POINTERS *e)
{
    DWORD code = e->ExceptionRecord->ExceptionCode;
    if (!fatal(code) || !enter())
        return EXCEPTION_CONTINUE_SEARCH;
    uintptr_t ip = e->ContextRecord->Rip;
    if (first_time(ip)) {
        if (reports < MAX_REPORTS)
            report("first-chance (the game may still handle it)", e);
        else
            cl("[crash] first-chance %s (0x%08lx) at %s; report cap reached, no details", code_name(code), code,
                     where(ip).c_str());
    }
    leave();
    return EXCEPTION_CONTINUE_SEARCH;
}

LONG WINAPI unhandled(EXCEPTION_POINTERS *e)
{
    if (enter()) {
        DWORD code = e->ExceptionRecord->ExceptionCode;
        cl("[crash] unhandled %s (0x%08lx) at %s reached the top-level filter; %s", code_name(code), code,
                 where(e->ContextRecord->Rip).c_str(),
                 previous ? "passing it to the game's filter" : "the process ends here");
        if (!fatal(code) || first_time(e->ContextRecord->Rip))
            report("unhandled", e);
        leave();
    }
    return previous ? previous(e) : EXCEPTION_CONTINUE_SEARCH;
}

void on_exit(const char *fn, UINT code)
{
    if (InterlockedExchange(&exited, 1) || !enter())
        return;
    cl("[crash] %s(%u) on thread %lu, %lu s after load%s", fn, code, GetCurrentThreadId(),
             (GetTickCount() - start_tick) / 1000, code ? "; caller stack:" : "");
    if (code) {
        CONTEXT ctx;
        RtlCaptureContext(&ctx);
        backtrace(ctx, true);
    }
    leave();
}

void WINAPI hooked_exit(UINT code)
{
    on_exit("ExitProcess", code);
    exit_hook.unsafe_call<void>(code);
}

BOOL WINAPI hooked_terminate(HANDLE process, UINT code)
{
    if (GetProcessId(process) == GetCurrentProcessId())
        on_exit("TerminateProcess", code);
    return terminate_hook.unsafe_call<BOOL>(process, code);
}

void load_symbols(const std::wstring &path, DWORD stamp)
{
    FILE *f = _wfopen(path.c_str(), L"rb");
    if (!f)
        return cl("[crash] symbols.tsv missing; exe frames show rva only");
    char line[512];
    bool match = false;
    while (fgets(line, sizeof line, f)) {
        char name[256];
        unsigned rva;
        unsigned long ts;
        if (line[0] == '#') {
            match = sscanf(line, "# %*s %*s %lu", &ts) == 1 && ts == stamp;
        } else if (match && sscanf(line, "%x\t%255[^\t\r\n]", &rva, name) == 2) {
            symbols.push_back({rva, name});
        }
    }
    fclose(f);
    std::sort(symbols.begin(), symbols.end(), [](const Symbol &a, const Symbol &b) { return a.rva < b.rva; });
    cl(match ? "[crash] symbols.tsv: %zu KB function name(s) for this exe"
                   : "[crash] symbols.tsv is for another build (%zu names loaded); exe frames show rva only",
             symbols.size());
}

}

void crash_attach(SwoftyApi *a, const std::wstring &dir)
{
    api = a;
    start_tick = GetTickCount();
    exe = GetModuleHandleW(nullptr);
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                       reinterpret_cast<LPCWSTR>(&crash_attach), &self);
    auto base = reinterpret_cast<uintptr_t>(exe);
    auto nt = reinterpret_cast<const IMAGE_NT_HEADERS *>(base + reinterpret_cast<const IMAGE_DOS_HEADER *>(base)->e_lfanew);
    char v[8] = {0};
    ea_build = GetEnvironmentVariableA("SWOFTY_EA_DIAG", v, sizeof v) && v[0] == '1';
    diag_path = ea_build ? dir + L"..\\..\\ea-diagnostics.log" : std::wstring();
    load_symbols(dir + L"symbols.tsv", nt->FileHeader.TimeDateStamp);
    AddVectoredExceptionHandler(1, veh);
    previous = SetUnhandledExceptionFilter(unhandled);
    HMODULE k32 = GetModuleHandleW(L"kernel32.dll");
    auto e = safetyhook::InlineHook::create(reinterpret_cast<void *>(GetProcAddress(k32, "ExitProcess")), hooked_exit);
    auto t = safetyhook::InlineHook::create(reinterpret_cast<void *>(GetProcAddress(k32, "TerminateProcess")), hooked_terminate);
    if (e)
        exit_hook = std::move(*e);
    if (t)
        terminate_hook = std::move(*t);
    cl("[crash] crash log armed: fatal exceptions (first %ld in full, deduplicated by address), the top-level "
             "filter%s, ExitProcess %s, TerminateProcess %s",
             MAX_REPORTS, previous ? " (chained to the game's)" : "", e ? "hooked" : "NOT hooked",
             t ? "hooked" : "NOT hooked");
}
