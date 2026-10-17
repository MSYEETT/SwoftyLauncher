#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <bcrypt.h>
#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <utility>
#include "Zydis.h"
#include "safetyhook.hpp"
#include "swofty_api.h"

extern "C" int trace_guarded_copy(void *dst, const void *src, size_t n);
extern "C" char trace_guarded_copy_fault[], trace_guarded_copy_end[];

__asm__(".text\n"
        ".globl trace_guarded_copy\n"
        "trace_guarded_copy:\n"
        "  test %r8, %r8\n"
        "  jz 2f\n"
        "1:\n"
        "  movb (%rdx), %al\n"
        "  movb %al, (%rcx)\n"
        "  inc %rcx\n"
        "  inc %rdx\n"
        "  dec %r8\n"
        "  jnz 1b\n"
        "2:\n"
        "  mov $1, %eax\n"
        "  ret\n"
        ".globl trace_guarded_copy_fault\n"
        "trace_guarded_copy_fault:\n"
        "  xor %eax, %eax\n"
        "  ret\n"
        ".globl trace_guarded_copy_end\n"
        "trace_guarded_copy_end:\n"
        "  int3\n");

#ifndef UNW_FLAG_CHAININFO
#define UNW_FLAG_CHAININFO 0x4
#endif

namespace {

constexpr int PLUGIN_VERSION = 1;
constexpr int MAX_HOOKS = 64;
constexpr int MAX_FIELDS = 24;
constexpr int MAX_CONDS = 4;
constexpr int MAX_REGS = 24;
constexpr int MAX_ARGS = 8;
constexpr int MAX_BT = 32;
constexpr int HARD_MAX_READ = 1024;
constexpr int LINE_MAX = 2048;
constexpr int RING_SLOTS = 4096;
constexpr uint32_t PAGE_CACHE = 1024;

struct J {
    enum Kind : uint8_t { NUL, BOOL, NUM, STR, ARR, OBJ } k = NUL;
    bool b = false;
    double num = 0;
    char *str = nullptr;
    int n = 0;
    J *items = nullptr;
    char **keys = nullptr;
    const J *get(const char *key) const
    {
        if (k != OBJ)
            return nullptr;
        for (int i = 0; i < n; i++)
            if (!strcmp(keys[i], key))
                return &items[i];
        return nullptr;
    }
};

struct JParser {
    const char *p, *start;
    const char *err = nullptr;
    void ws()
    {
        while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n')
            p++;
    }
    bool fail(const char *e)
    {
        if (!err)
            err = e;
        return false;
    }
    bool str(char **out)
    {
        if (*p != '"')
            return fail("expected string");
        p++;
        size_t cap = 32, n = 0;
        char *s = (char *)malloc(cap);
        while (*p && *p != '"') {
            char c = *p++;
            if (c == '\\') {
                if (!*p) {
                    free(s);
                    return fail("unterminated escape");
                }
                char e = *p++;
                switch (e) {
                case 'n': c = '\n'; break;
                case 't': c = '\t'; break;
                case 'r': c = '\r'; break;
                case 'b': c = '\b'; break;
                case 'f': c = '\f'; break;
                case 'u': {
                    if (strnlen(p, 4) < 4) {
                        free(s);
                        return fail("short \\u escape");
                    }
                    unsigned v = 0;
                    for (int i = 0; i < 4; i++) {
                        char h = *p++;
                        v = v * 16 + (h >= '0' && h <= '9' ? h - '0' : (h | 32) - 'a' + 10);
                    }
                    c = v < 0x80 ? (char)v : '?';
                    break;
                }
                default: c = e;
                }
            }
            if (n + 2 > cap)
                s = (char *)realloc(s, cap *= 2);
            s[n++] = c;
        }
        if (*p != '"')
            return fail("unterminated string");
        p++;
        s[n] = 0;
        *out = s;
        return true;
    }
    bool val(J &v)
    {
        ws();
        if (*p == '{') {
            p++;
            v.k = J::OBJ;
            int cap = 0;
            ws();
            if (*p == '}') {
                p++;
                return true;
            }
            for (;;) {
                ws();
                char *key;
                if (!str(&key))
                    return false;
                ws();
                if (*p++ != ':')
                    return fail("expected ':'");
                J child;
                if (!val(child))
                    return false;
                if (v.n == cap) {
                    cap = cap ? cap * 2 : 8;
                    v.items = (J *)realloc((void *)v.items, cap * sizeof(J));
                    v.keys = (char **)realloc(v.keys, cap * sizeof(char *));
                }
                v.items[v.n] = child;
                v.keys[v.n++] = key;
                ws();
                if (*p == ',') {
                    p++;
                    continue;
                }
                if (*p == '}') {
                    p++;
                    return true;
                }
                return fail("expected ',' or '}'");
            }
        }
        if (*p == '[') {
            p++;
            v.k = J::ARR;
            int cap = 0;
            ws();
            if (*p == ']') {
                p++;
                return true;
            }
            for (;;) {
                J child;
                if (!val(child))
                    return false;
                if (v.n == cap) {
                    cap = cap ? cap * 2 : 8;
                    v.items = (J *)realloc((void *)v.items, cap * sizeof(J));
                }
                v.items[v.n++] = child;
                ws();
                if (*p == ',') {
                    p++;
                    continue;
                }
                if (*p == ']') {
                    p++;
                    return true;
                }
                return fail("expected ',' or ']'");
            }
        }
        if (*p == '"') {
            v.k = J::STR;
            return str(&v.str);
        }
        if (!strncmp(p, "true", 4)) {
            v.k = J::BOOL;
            v.b = true;
            p += 4;
            return true;
        }
        if (!strncmp(p, "false", 5)) {
            v.k = J::BOOL;
            p += 5;
            return true;
        }
        if (!strncmp(p, "null", 4)) {
            p += 4;
            return true;
        }
        char *end;
        v.num = strtod(p, &end);
        if (end == p)
            return fail("unexpected character");
        v.k = J::NUM;
        p = end;
        return true;
    }
};

const SwoftyApi *api;
uintptr_t mod_base, pref_base, mod_end;
wchar_t game_dir[MAX_PATH], exe_path[MAX_PATH];
HANDLE log_file = INVALID_HANDLE_VALUE;
CRITICAL_SECTION file_lock;
LARGE_INTEGER qpc0, qpf;
uint64_t log_bytes, max_log_bytes = 64ull << 20;
bool rotate_on_full;
std::atomic<int> stopped, quitting;
std::atomic<uint64_t> logged_total, total_capped, lines_dropped_full;
uint64_t max_events_total = 200000;
uint32_t flush_ms = 100, summary_every_s = 60;
int default_max_read = 64;

struct Sec {
    char name[9];
    uintptr_t start, end;
    DWORD flags;
};
Sec secs[96];
int nsecs;
char allowed_secs[8][9];
int nallowed;

enum FType : uint8_t { U8, U16, U32, U64, I8, I16, I32, I64, F32, F64, PTR, BOOLT, HEX, CSTR, WSTR, NESTR, NT };
const char *const ftype_names[] = {"u8",  "u16", "u32", "u64", "i8",  "i16",  "i32",  "i64",
                                   "f32", "f64", "ptr", "bool", "hex", "cstr", "wstr", "nestr"};
const uint8_t ftype_size[] = {1, 2, 4, 8, 1, 2, 4, 8, 4, 8, 8, 1, 0, 0, 0, 0};
bool is_scalar(FType t) { return t <= BOOLT; }

const char *const reg_names[] = {"rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8",
                                 "r9",  "r10", "r11", "r12", "r13", "r14", "r15", "rflags"};
constexpr int NGPR = 17;
const size_t reg_off[] = {offsetof(safetyhook::Context, rax), offsetof(safetyhook::Context, rbx),
                          offsetof(safetyhook::Context, rcx), offsetof(safetyhook::Context, rdx),
                          offsetof(safetyhook::Context, rsi), offsetof(safetyhook::Context, rdi),
                          offsetof(safetyhook::Context, rbp), offsetof(safetyhook::Context, rsp),
                          offsetof(safetyhook::Context, r8),  offsetof(safetyhook::Context, r9),
                          offsetof(safetyhook::Context, r10), offsetof(safetyhook::Context, r11),
                          offsetof(safetyhook::Context, r12), offsetof(safetyhook::Context, r13),
                          offsetof(safetyhook::Context, r14), offsetof(safetyhook::Context, r15),
                          offsetof(safetyhook::Context, rflags)};
constexpr int REG_XMM0 = 32;

int reg_id(const char *s, size_t n)
{
    for (int i = 0; i < NGPR; i++)
        if (strlen(reg_names[i]) == n && !strncmp(reg_names[i], s, n))
            return i;
    if (n >= 4 && n <= 5 && !strncmp(s, "xmm", 3)) {
        int v = atoi(s + 3);
        if (v >= 0 && v < 16)
            return REG_XMM0 + v;
    }
    return -1;
}

const char *reg_name(int r)
{
    static const char *const x[] = {"xmm0", "xmm1", "xmm2",  "xmm3",  "xmm4",  "xmm5",  "xmm6",  "xmm7",
                                    "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15"};
    return r >= REG_XMM0 ? x[r - REG_XMM0] : reg_names[r];
}

uint64_t reg_val(const safetyhook::Context &c, int r)
{
    if (r >= REG_XMM0)
        return (&c.xmm0)[r - REG_XMM0].u64[0];
    return *reinterpret_cast<const uint64_t *>(reinterpret_cast<const uint8_t *>(&c) + reg_off[r]);
}

std::atomic<uint64_t> page_cache[PAGE_CACHE];

uint64_t now_ms()
{
    LARGE_INTEGER t;
    QueryPerformanceCounter(&t);
    uint64_t d = (uint64_t)(t.QuadPart - qpc0.QuadPart), f = (uint64_t)qpf.QuadPart;
    return d / f * 1000 + (d % f) * 1000 / f;
}

uint64_t now_us()
{
    LARGE_INTEGER t;
    QueryPerformanceCounter(&t);
    uint64_t d = (uint64_t)(t.QuadPart - qpc0.QuadPart), f = (uint64_t)qpf.QuadPart;
    return d / f * 1000000 + (d % f) * 1000000 / f;
}

bool page_readable(uintptr_t page)
{
    uint64_t pn = page >> 12;
    if (pn == 0 || pn >= (1ull << 35))
        return false;
    uint64_t tick = (now_ms() >> 4) & ((1ull << 28) - 1);
    auto &slot = page_cache[(pn * 0x9E3779B1u) % PAGE_CACHE];
    uint64_t e = slot.load(std::memory_order_relaxed);
    if ((e & ((1ull << 35) - 1)) == pn && ((tick - (e >> 36)) & ((1ull << 28) - 1)) < 32)
        return (e >> 35) & 1;
    MEMORY_BASIC_INFORMATION m;
    bool ok = VirtualQuery((void *)page, &m, sizeof m) && m.State == MEM_COMMIT &&
              (m.Protect & (PAGE_READONLY | PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READ |
                            PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY)) &&
              !(m.Protect & (PAGE_GUARD | PAGE_NOACCESS));
    slot.store(pn | ((uint64_t)ok << 35) | (tick << 36), std::memory_order_relaxed);
    return ok;
}

bool safe_read(void *dst, uintptr_t src, size_t n)
{
    if (!n)
        return true;
    if (src + n < src)
        return false;
    for (uintptr_t pg = src & ~(uintptr_t)0xfff; pg < src + n; pg += 0x1000)
        if (!page_readable(pg))
            return false;
    return trace_guarded_copy(dst, (const void *)src, n) != 0;
}

LONG CALLBACK veh(EXCEPTION_POINTERS *e)
{
    DWORD code = e->ExceptionRecord->ExceptionCode;
    uintptr_t ip = e->ContextRecord->Rip;
    if ((code == EXCEPTION_ACCESS_VIOLATION || code == EXCEPTION_IN_PAGE_ERROR || code == STATUS_GUARD_PAGE_VIOLATION) &&
        ip >= (uintptr_t)trace_guarded_copy && ip < (uintptr_t)trace_guarded_copy_fault) {
        e->ContextRecord->Rip = (uintptr_t)trace_guarded_copy_fault;
        return EXCEPTION_CONTINUE_EXECUTION;
    }
    return EXCEPTION_CONTINUE_SEARCH;
}

struct Out {
    char *p;
    int n, cap;
    bool trunc;
    void c(char ch)
    {
        if (n < cap)
            p[n++] = ch;
        else
            trunc = true;
    }
    void s(const char *x)
    {
        while (*x)
            c(*x++);
    }
    void hex(uint64_t v)
    {
        char b[17];
        int i = 16;
        b[i] = 0;
        do {
            b[--i] = "0123456789abcdef"[v & 15];
            v >>= 4;
        } while (v);
        s("0x");
        s(b + i);
    }
    void u(uint64_t v)
    {
        char b[21];
        int i = 20;
        b[i] = 0;
        do {
            b[--i] = (char)('0' + v % 10);
            v /= 10;
        } while (v);
        s(b + i);
    }
    void i(int64_t v)
    {
        if (v < 0) {
            c('-');
            u((uint64_t)0 - (uint64_t)v);
        } else
            u((uint64_t)v);
    }
    void ms(uint64_t us)
    {
        u(us / 1000);
        c('.');
        char b[4] = {(char)('0' + us / 100 % 10), (char)('0' + us / 10 % 10), (char)('0' + us % 10), 0};
        s(b);
    }
    void f(double d)
    {
        if (d != d) {
            s("nan");
            return;
        }
        if (d < 0) {
            c('-');
            d = -d;
        }
        if (d > 1.7e308) {
            s("inf");
            return;
        }
        int e = 0;
        if (d != 0 && (d >= 1e15 || d < 1e-4)) {
            while (d >= 10) {
                d /= 10;
                e++;
            }
            while (d < 1) {
                d *= 10;
                e--;
            }
        }
        uint64_t ip = (uint64_t)d;
        double fr = d - (double)ip;
        uint64_t fv = (uint64_t)(fr * 1e6 + 0.5);
        if (fv >= 1000000) {
            ip++;
            fv -= 1000000;
        }
        u(ip);
        if (fv) {
            c('.');
            char b[7];
            for (int k = 5; k >= 0; k--) {
                b[k] = (char)('0' + fv % 10);
                fv /= 10;
            }
            int len = 6;
            while (len > 1 && b[len - 1] == '0')
                len--;
            for (int k = 0; k < len; k++)
                c(b[k]);
        }
        if (e) {
            c('e');
            i(e);
        }
    }
    void q(const char *x, int len)
    {
        c('"');
        for (int k = 0; k < len; k++) {
            unsigned char ch = (unsigned char)x[k];
            if (ch == '"' || ch == '\\') {
                c('\\');
                c((char)ch);
            } else if (ch < 0x20 || ch >= 0x7f) {
                s("\\x");
                c("0123456789abcdef"[ch >> 4]);
                c("0123456789abcdef"[ch & 15]);
            } else
                c((char)ch);
        }
        c('"');
    }
    void code_addr(uint64_t a)
    {
        if (a >= mod_base && a < mod_end) {
            hex(a - mod_base + pref_base);
        } else {
            c('@');
            hex(a);
        }
    }
};

enum Op : uint8_t { OP_REG, OP_IMM, OP_ADD, OP_DEREF };
struct Ins {
    Op op;
    int8_t reg;
    int64_t imm;
};
struct Expr {
    Ins ins[16];
    uint8_t n;
    bool top_deref;
};

struct ExprParser {
    const char *p;
    Expr *e;
    bool emit(Op op, int reg, int64_t imm)
    {
        if (e->n >= 16)
            return false;
        e->ins[e->n++] = {op, (int8_t)reg, imm};
        return true;
    }
    void ws()
    {
        while (*p == ' ')
            p++;
    }
    bool number(int64_t *v)
    {
        ws();
        char *end;
        unsigned long long x = strtoull(p, &end, 0);
        if (end == p)
            return false;
        *v = (int64_t)x;
        p = end;
        return true;
    }
    bool term()
    {
        ws();
        if (*p == '[') {
            p++;
            if (!expr())
                return false;
            ws();
            if (*p++ != ']')
                return false;
            return emit(OP_DEREF, 0, 0);
        }
        if ((*p >= 'a' && *p <= 'z')) {
            const char *s = p;
            while ((*p >= 'a' && *p <= 'z') || (*p >= '0' && *p <= '9'))
                p++;
            int r = reg_id(s, p - s);
            if (r >= 0)
                return emit(OP_REG, r, 0);
            if (p - s >= 2 && s[0] == 'a' && s[1] >= '0' && s[1] <= '9') {
                int k = atoi(s + 1);
                static const int argreg[] = {2, 3, 8, 9};
                if (k < 4)
                    return emit(OP_REG, argreg[k], 0);
                return emit(OP_REG, 7, 0) && emit(OP_IMM, 0, 0x28 + 8 * (k - 4)) && emit(OP_ADD, 0, 0) &&
                       emit(OP_DEREF, 0, 0);
            }
            return false;
        }
        int64_t v;
        if (!number(&v))
            return false;
        return emit(OP_IMM, 0, v);
    }
    bool expr()
    {
        if (!term())
            return false;
        for (;;) {
            ws();
            if (*p != '+' && *p != '-')
                return true;
            char sign = *p++;
            int64_t v;
            if (!number(&v))
                return false;
            if (!emit(OP_IMM, 0, sign == '-' ? -v : v) || !emit(OP_ADD, 0, 0))
                return false;
        }
    }
};

bool parse_expr(const char *s, Expr *e)
{
    memset(e, 0, sizeof *e);
    ExprParser ep{s, e};
    if (!ep.expr())
        return false;
    ep.ws();
    if (*ep.p)
        return false;
    const char *a = s;
    while (*a == ' ')
        a++;
    if (*a == '[') {
        int depth = 0;
        const char *q = a;
        for (; *q; q++) {
            if (*q == '[')
                depth++;
            else if (*q == ']' && --depth == 0)
                break;
        }
        const char *rest = q + (*q ? 1 : 0);
        while (*rest == ' ')
            rest++;
        e->top_deref = !*rest;
    }
    return true;
}

bool eval(const Expr &e, const safetyhook::Context &c, int upto, uint64_t *out, uint64_t *fault)
{
    uint64_t st[16];
    int sp = 0;
    for (int k = 0; k < upto; k++) {
        const Ins &in = e.ins[k];
        switch (in.op) {
        case OP_REG: st[sp++] = reg_val(c, in.reg); break;
        case OP_IMM: st[sp++] = (uint64_t)in.imm; break;
        case OP_ADD:
            sp--;
            st[sp - 1] += st[sp];
            break;
        case OP_DEREF: {
            uint64_t a = st[sp - 1], v;
            if (!safe_read(&v, a, 8)) {
                *fault = a;
                return false;
            }
            st[sp - 1] = v;
            break;
        }
        }
    }
    *out = st[0];
    return true;
}

struct Field {
    char name[32];
    FType t;
    uint16_t len;
    Expr e;
};
struct Cond {
    FType t;
    Expr e;
    uint8_t op;
    uint64_t v;
};
enum Kind { K_MID, K_FUNC };

struct Hook {
    char id[64], test[16], sig[64];
    int kind;
    int8_t regs[MAX_REGS];
    int nregs, nargs, bt;
    bool ret;
    Field fields[MAX_FIELDS];
    int nfields;
    Cond conds[MAX_CONDS];
    int nconds;
    uint64_t max_per_sec, max_events;
    uintptr_t target;
    bool installed;
    safetyhook::MidHook mh;
    std::atomic<uint64_t> calls, logged, filtered, rate_dropped, capped, ring_dropped, read_errors, win;
    uint64_t last_summary_calls;
};
Hook *hooks;
int nhooks;

struct Slot {
    std::atomic<uint64_t> seq;
    uint32_t len;
    char buf[LINE_MAX];
};
Slot *ring;
std::atomic<uint64_t> enq_pos;
uint64_t deq_pos;

Slot *ring_reserve()
{
    uint64_t pos = enq_pos.load(std::memory_order_relaxed);
    for (;;) {
        Slot *s = &ring[pos % RING_SLOTS];
        uint64_t seq = s->seq.load(std::memory_order_acquire);
        int64_t diff = (int64_t)seq - (int64_t)pos;
        if (diff == 0) {
            if (enq_pos.compare_exchange_weak(pos, pos + 1, std::memory_order_relaxed))
                return s;
        } else if (diff < 0) {
            return nullptr;
        } else {
            pos = enq_pos.load(std::memory_order_relaxed);
        }
    }
}

void ring_commit(Slot *s)
{
    uint64_t pos = s->seq.load(std::memory_order_relaxed);
    s->seq.store(pos + 1, std::memory_order_release);
}

void file_write_raw(const char *b, DWORD n)
{
    DWORD w;
    if (log_file != INVALID_HANDLE_VALUE)
        WriteFile(log_file, b, n, &w, nullptr);
    log_bytes += n;
}

void open_log(bool rotate_prev)
{
    wchar_t path[MAX_PATH * 2], prev[MAX_PATH * 2];
    wsprintfW(path, L"%s\\swofty\\trace.log", game_dir);
    wsprintfW(prev, rotate_prev ? L"%s\\swofty\\trace.prev.log" : L"%s\\swofty\\trace.log.1", game_dir);
    MoveFileExW(path, prev, MOVEFILE_REPLACE_EXISTING);
    log_file = CreateFileW(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, CREATE_ALWAYS,
                           FILE_ATTRIBUTE_NORMAL, nullptr);
    log_bytes = 0;
}

void meta_line(const char *b, int n)
{
    EnterCriticalSection(&file_lock);
    file_write_raw(b, n);
    LeaveCriticalSection(&file_lock);
}

struct LineBuf {
    char b[LINE_MAX];
    Out o{b, 0, LINE_MAX - 1, false};
    void begin(char kind)
    {
        o.c(kind);
        o.c(' ');
        o.ms(now_us());
    }
    void end()
    {
        if (o.trunc) {
            o.n = o.n > LINE_MAX - 10 ? LINE_MAX - 10 : o.n;
            o.trunc = false;
            o.s(" ~trunc");
        }
        b[o.n++] = '\n';
        meta_line(b, o.n);
    }
};

void events_write(const char *b, DWORD n, uint64_t lines)
{
    EnterCriticalSection(&file_lock);
    if (stopped.load()) {
        lines_dropped_full += lines;
    } else if (log_bytes + n > max_log_bytes) {
        if (rotate_on_full) {
            CloseHandle(log_file);
            open_log(false);
            LeaveCriticalSection(&file_lock);
            LineBuf l;
            l.begin('H');
            l.o.s(" rotated=1 previous=trace.log.1");
            l.end();
            EnterCriticalSection(&file_lock);
            file_write_raw(b, n);
        } else {
            stopped = 1;
            lines_dropped_full += lines;
            LeaveCriticalSection(&file_lock);
            LineBuf l;
            l.begin('W');
            l.o.s(" log_full=1 max_bytes=");
            l.o.u(max_log_bytes);
            l.o.s(" events_stopped=1 counters_continue=1");
            l.end();
            return;
        }
    } else {
        file_write_raw(b, n);
    }
    LeaveCriticalSection(&file_lock);
}

void drain()
{
    static char buf[1 << 16];
    int n = 0;
    uint64_t lines = 0;
    for (;;) {
        Slot *s = &ring[deq_pos % RING_SLOTS];
        uint64_t seq = s->seq.load(std::memory_order_acquire);
        if (seq != deq_pos + 1)
            break;
        if (n + (int)s->len > (int)sizeof buf) {
            events_write(buf, n, lines);
            n = 0;
            lines = 0;
        }
        memcpy(buf + n, s->buf, s->len);
        n += s->len;
        lines++;
        s->seq.store(deq_pos + RING_SLOTS, std::memory_order_release);
        deq_pos++;
    }
    if (n)
        events_write(buf, n, lines);
}

void summary(bool final)
{
    for (int i = 0; i < nhooks; i++) {
        Hook &h = hooks[i];
        uint64_t calls = h.calls.load();
        if (!final && calls == h.last_summary_calls)
            continue;
        h.last_summary_calls = calls;
        LineBuf l;
        l.begin('S');
        l.o.s(" hook=");
        l.o.s(h.id);
        l.o.s(" installed=");
        l.o.u(h.installed);
        l.o.s(" calls=");
        l.o.u(calls);
        l.o.s(" logged=");
        l.o.u(h.logged.load());
        l.o.s(" filtered=");
        l.o.u(h.filtered.load());
        l.o.s(" rate_dropped=");
        l.o.u(h.rate_dropped.load());
        l.o.s(" capped=");
        l.o.u(h.capped.load());
        l.o.s(" ring_dropped=");
        l.o.u(h.ring_dropped.load());
        l.o.s(" read_errors=");
        l.o.u(h.read_errors.load());
        if (final)
            l.o.s(" final=1");
        l.end();
    }
    LineBuf l;
    l.begin('S');
    l.o.s(" total logged=");
    l.o.u(logged_total.load());
    l.o.s(" capped=");
    l.o.u(total_capped.load());
    l.o.s(" dropped_log_full=");
    l.o.u(lines_dropped_full.load());
    l.o.s(" stopped=");
    l.o.u(stopped.load());
    if (final)
        l.o.s(" final=1");
    l.end();
}

DWORD WINAPI writer(LPVOID)
{
    uint64_t last = now_ms();
    while (!quitting.load()) {
        Sleep(flush_ms);
        drain();
        if (summary_every_s && now_ms() - last >= summary_every_s * 1000ull) {
            last = now_ms();
            summary(false);
        }
    }
    return 0;
}

bool rate_ok(Hook &h, uint64_t now_s)
{
    if (!h.max_per_sec)
        return true;
    uint64_t w = h.win.load(std::memory_order_relaxed);
    for (;;) {
        uint64_t s = w >> 20, c = w & 0xFFFFF, nw;
        if (s != now_s)
            nw = (now_s << 20) | 1;
        else if (c >= h.max_per_sec)
            return false;
        else
            nw = w + 1;
        if (h.win.compare_exchange_weak(w, nw, std::memory_order_relaxed))
            return true;
    }
}

bool scalar_value(const Expr &e, FType t, const safetyhook::Context &c, uint64_t *out, uint64_t *fault)
{
    uint64_t v = 0;
    if (e.top_deref && is_scalar(t)) {
        uint64_t addr;
        if (!eval(e, c, e.n - 1, &addr, fault))
            return false;
        if (!safe_read(&v, addr, ftype_size[t])) {
            *fault = addr;
            return false;
        }
    } else {
        if (!eval(e, c, e.n, &v, fault))
            return false;
        if (is_scalar(t) && ftype_size[t] < 8)
            v &= (1ull << (8 * ftype_size[t])) - 1;
    }
    *out = v;
    return true;
}

int64_t sext(uint64_t v, int bytes)
{
    int sh = 64 - 8 * bytes;
    return (int64_t)(v << sh) >> sh;
}

void put_scalar(Out &o, FType t, uint64_t v)
{
    switch (t) {
    case I8: case I16: case I32: case I64: o.i(sext(v, ftype_size[t])); break;
    case F32: {
        float f;
        uint32_t x = (uint32_t)v;
        memcpy(&f, &x, 4);
        o.f(f);
        break;
    }
    case F64: {
        double d;
        memcpy(&d, &v, 8);
        o.f(d);
        break;
    }
    case BOOLT: o.s(v ? "true" : "false"); break;
    default: o.hex(v);
    }
}

void put_field(Out &o, Hook &h, const Field &f, const safetyhook::Context &c)
{
    o.c(' ');
    o.s(f.name);
    o.c('=');
    uint64_t v = 0, fault = 0;
    if (!scalar_value(f.e, is_scalar(f.t) ? f.t : U64, c, &v, &fault)) {
        h.read_errors++;
        o.s("!unreadable@");
        o.hex(fault);
        return;
    }
    if (is_scalar(f.t)) {
        put_scalar(o, f.t, v);
        return;
    }
    char tmp[HARD_MAX_READ + 16];
    int len = f.len;
    if (f.t == HEX) {
        if (!safe_read(tmp, v, len)) {
            h.read_errors++;
            o.s("!unreadable@");
            o.hex(v);
            return;
        }
        o.s("h:");
        for (int k = 0; k < len; k++) {
            o.c("0123456789abcdef"[(uint8_t)tmp[k] >> 4]);
            o.c("0123456789abcdef"[tmp[k] & 15]);
        }
        return;
    }
    if (f.t == NESTR) {
        if (!v) {
            o.s("null:\"\"");
            return;
        }
        uint32_t hash;
        if (!safe_read(&hash, v, 4)) {
            h.read_errors++;
            o.s("!unreadable@");
            o.hex(v);
            return;
        }
        o.hex(hash);
        o.c(':');
        v += 9;
    }
    int got = 0;
    bool ok = true;
    if (f.t == WSTR) {
        while (got < len) {
            uint16_t ch;
            if (!safe_read(&ch, v + got * 2, 2)) {
                ok = false;
                break;
            }
            if (!ch)
                break;
            tmp[got++] = ch < 0x80 ? (char)ch : '?';
        }
    } else {
        while (got < len) {
            char ch;
            if (!safe_read(&ch, v + got, 1)) {
                ok = false;
                break;
            }
            if (!ch)
                break;
            tmp[got++] = ch;
        }
    }
    o.q(tmp, got);
    if (!ok) {
        h.read_errors++;
        o.s("~unreadable");
    } else if (got == len)
        o.s("~cut");
}

void put_backtrace(Out &o, const safetyhook::Context &c, uintptr_t site, int depth)
{
    CONTEXT ctx;
    memset(&ctx, 0, sizeof ctx);
    ctx.Rip = site;
    ctx.Rsp = c.rsp;
    ctx.Rax = c.rax, ctx.Rbx = c.rbx, ctx.Rcx = c.rcx, ctx.Rdx = c.rdx, ctx.Rsi = c.rsi, ctx.Rdi = c.rdi;
    ctx.Rbp = c.rbp, ctx.R8 = c.r8, ctx.R9 = c.r9, ctx.R10 = c.r10, ctx.R11 = c.r11, ctx.R12 = c.r12;
    ctx.R13 = c.r13, ctx.R14 = c.r14, ctx.R15 = c.r15;
    o.s(" bt=");
    for (int k = 0; k < depth; k++) {
        DWORD64 ib = 0;
        PRUNTIME_FUNCTION fe = RtlLookupFunctionEntry(ctx.Rip, &ib, nullptr);
        if (fe) {
            void *hd;
            DWORD64 est;
            RtlVirtualUnwind(UNW_FLAG_NHANDLER, ib, ctx.Rip, fe, &ctx, &hd, &est, nullptr);
        } else {
            uint64_t r;
            if (!safe_read(&r, ctx.Rsp, 8))
                break;
            ctx.Rip = r;
            ctx.Rsp += 8;
        }
        if (!ctx.Rip || !page_readable(ctx.Rip & ~0xfffull))
            break;
        if (k)
            o.c(',');
        o.code_addr(ctx.Rip);
    }
}

void on_hit(int idx, safetyhook::Context &c)
{
    Hook &h = hooks[idx];
    uint64_t n = h.calls.fetch_add(1, std::memory_order_relaxed) + 1;
    if (stopped.load(std::memory_order_relaxed))
        return;
    for (int k = 0; k < h.nconds; k++) {
        const Cond &cd = h.conds[k];
        uint64_t v, fault;
        bool pass = false;
        if (scalar_value(cd.e, cd.t, c, &v, &fault)) {
            bool sgn = cd.t >= I8 && cd.t <= I64;
            int64_t sv = sext(v, ftype_size[cd.t]), sw = (int64_t)cd.v;
            switch (cd.op) {
            case 0: pass = v == cd.v; break;
            case 1: pass = v != cd.v; break;
            case 2: pass = sgn ? sv < sw : v < cd.v; break;
            case 3: pass = sgn ? sv > sw : v > cd.v; break;
            case 4: pass = sgn ? sv <= sw : v <= cd.v; break;
            case 5: pass = sgn ? sv >= sw : v >= cd.v; break;
            case 6: pass = (v & cd.v) != 0; break;
            }
        }
        if (!pass) {
            h.filtered.fetch_add(1, std::memory_order_relaxed);
            return;
        }
    }
    if (h.logged.load(std::memory_order_relaxed) >= h.max_events) {
        h.capped.fetch_add(1, std::memory_order_relaxed);
        return;
    }
    if (logged_total.load(std::memory_order_relaxed) >= max_events_total) {
        h.capped.fetch_add(1, std::memory_order_relaxed);
        total_capped.fetch_add(1, std::memory_order_relaxed);
        return;
    }
    uint64_t us = now_us();
    if (!rate_ok(h, us / 1000000)) {
        h.rate_dropped.fetch_add(1, std::memory_order_relaxed);
        return;
    }
    Slot *s = ring_reserve();
    if (!s) {
        h.ring_dropped.fetch_add(1, std::memory_order_relaxed);
        return;
    }
    uint64_t ev = h.logged.fetch_add(1, std::memory_order_relaxed) + 1;
    logged_total.fetch_add(1, std::memory_order_relaxed);
    Out o{s->buf, 0, LINE_MAX - 12, false};
    o.s("E ");
    o.ms(us);
    o.s(" tid=");
    o.u(GetCurrentThreadId());
    o.s(" hook=");
    o.s(h.id);
    o.s(" n=");
    o.u(n);
    o.s(" ev=");
    o.u(ev);
    for (int k = 0; k < h.nregs; k++) {
        o.c(' ');
        o.s(reg_name(h.regs[k]));
        o.c('=');
        o.hex(reg_val(c, h.regs[k]));
    }
    for (int k = 0; k < h.nargs; k++) {
        o.s(" a");
        o.u(k);
        o.c('=');
        static const int argreg[] = {2, 3, 8, 9};
        if (k < 4) {
            o.hex(reg_val(c, argreg[k]));
        } else {
            uint64_t v;
            if (safe_read(&v, c.rsp + 0x28 + 8 * (k - 4), 8))
                o.hex(v);
            else
                o.s("!unreadable");
        }
    }
    for (int k = 0; k < h.nfields; k++)
        put_field(o, h, h.fields[k], c);
    if (h.ret) {
        uint64_t r;
        o.s(" ret=");
        if (safe_read(&r, c.rsp, 8))
            o.code_addr(r);
        else
            o.s("!unreadable");
    }
    if (h.bt)
        put_backtrace(o, c, h.target, h.bt);
    if (o.trunc) {
        o.trunc = false;
        o.cap = LINE_MAX - 1;
        o.s(" ~trunc");
    }
    s->buf[o.n++] = '\n';
    s->len = o.n;
    ring_commit(s);
}

template <int I> void cb(safetyhook::Context &c) { on_hit(I, c); }
template <int... I> constexpr auto make_cbs(std::integer_sequence<int, I...>)
{
    return std::array<safetyhook::MidHookFn, sizeof...(I)>{&cb<I>...};
}
const auto callbacks = make_cbs(std::make_integer_sequence<int, MAX_HOOKS>{});

void hlog(char kind, const char *a, const char *b = nullptr, const char *c = nullptr, const char *d = nullptr)
{
    LineBuf l;
    l.begin(kind);
    l.o.c(' ');
    l.o.s(a);
    if (b)
        l.o.s(b);
    if (c)
        l.o.s(c);
    if (d)
        l.o.s(d);
    l.end();
}

bool parse_hex_bytes(const char *s, uint8_t *out, int *n, int max)
{
    *n = 0;
    while (*s) {
        while (*s == ' ')
            s++;
        if (!*s)
            break;
        char *end;
        long v = strtol(s, &end, 16);
        if (end - s != 2 || *n == max)
            return false;
        out[(*n)++] = (uint8_t)v;
        s = end;
    }
    return true;
}

uint64_t jnum(const J *v, uint64_t dflt)
{
    if (!v)
        return dflt;
    if (v->k == J::NUM)
        return (uint64_t)(int64_t)v->num;
    if (v->k == J::STR)
        return strtoull(v->str, nullptr, 0);
    if (v->k == J::BOOL)
        return v->b;
    return dflt;
}

const char *jstr(const J *v, const char *dflt) { return v && v->k == J::STR ? v->str : dflt; }
bool jbool(const J *v, bool dflt) { return v && v->k == J::BOOL ? v->b : dflt; }

int ftype_parse(const char *s, size_t n)
{
    for (int i = 0; i < NT; i++)
        if (strlen(ftype_names[i]) == n && !strncmp(ftype_names[i], s, n))
            return i;
    return -1;
}

bool parse_type_spec(const char *s, size_t n, FType *t, uint16_t *len, const char **err)
{
    const char *br = (const char *)memchr(s, '[', n);
    size_t tn = br ? (size_t)(br - s) : n;
    int ft = ftype_parse(s, tn);
    if (ft < 0) {
        *err = "unknown type";
        return false;
    }
    *t = (FType)ft;
    *len = (uint16_t)(ft == HEX ? 16 : default_max_read);
    if (br) {
        long v = strtol(br + 1, nullptr, 0);
        if (v <= 0 || v > HARD_MAX_READ) {
            *err = "length out of range (1..1024)";
            return false;
        }
        *len = (uint16_t)v;
    }
    return true;
}

bool load_field(const J *fj, Field *f, const char **err)
{
    const char *name, *type, *expr;
    char nbuf[32], tbuf[32];
    if (fj->k == J::STR) {
        const char *s = fj->str, *eq = strchr(s, '='), *colon = eq ? strchr(eq, ':') : nullptr;
        if (!eq || !colon || eq - s >= 32 || colon - eq - 1 >= 32) {
            *err = "field string must be name=type:expr";
            return false;
        }
        memcpy(nbuf, s, eq - s);
        nbuf[eq - s] = 0;
        memcpy(tbuf, eq + 1, colon - eq - 1);
        tbuf[colon - eq - 1] = 0;
        name = nbuf, type = tbuf, expr = colon + 1;
    } else {
        name = jstr(fj->get("name"), nullptr);
        type = jstr(fj->get("type"), "u64");
        expr = jstr(fj->get("expr"), nullptr);
        if (!name || !expr) {
            *err = "field needs name and expr";
            return false;
        }
    }
    if (strlen(name) >= sizeof f->name) {
        *err = "field name too long";
        return false;
    }
    strcpy(f->name, name);
    if (!parse_type_spec(type, strlen(type), &f->t, &f->len, err))
        return false;
    if (fj->k == J::OBJ && fj->get("len")) {
        uint64_t l = jnum(fj->get("len"), 0);
        if (!l || l > HARD_MAX_READ) {
            *err = "len out of range (1..1024)";
            return false;
        }
        f->len = (uint16_t)l;
    }
    if (!parse_expr(expr, &f->e)) {
        *err = "bad expression";
        return false;
    }
    return true;
}

bool load_cond(const char *s, Cond *cd, const char **err)
{
    char buf[160];
    if (strlen(s) >= sizeof buf) {
        *err = "condition too long";
        return false;
    }
    strcpy(buf, s);
    cd->t = U64;
    char *p = buf;
    char *colon = strchr(p, ':');
    if (colon && colon - p <= 4 && ftype_parse(p, colon - p) >= 0 && is_scalar((FType)ftype_parse(p, colon - p))) {
        cd->t = (FType)ftype_parse(p, colon - p);
        p = colon + 1;
    }
    static const char *const ops[] = {"==", "!=", "<=", ">=", "<", ">", "&"};
    static const uint8_t opc[] = {0, 1, 4, 5, 2, 3, 6};
    for (int i = 0; i < 7; i++) {
        char *o = strstr(p, ops[i]);
        if (!o)
            continue;
        *o = 0;
        cd->op = opc[i];
        cd->v = strtoull(o + strlen(ops[i]), nullptr, 0);
        const char *vs = o + strlen(ops[i]);
        while (*vs == ' ')
            vs++;
        if (*vs == '-')
            cd->v = (uint64_t)strtoll(vs, nullptr, 0);
        if (!parse_expr(p, &cd->e)) {
            *err = "bad condition expression";
            return false;
        }
        return true;
    }
    *err = "condition needs an operator (== != < > <= >= &)";
    return false;
}

const Sec *sec_of(uintptr_t a)
{
    for (int i = 0; i < nsecs; i++)
        if (a >= secs[i].start && a < secs[i].end)
            return &secs[i];
    return nullptr;
}

bool sec_allowed(const char *name)
{
    for (int i = 0; i < nallowed; i++)
        if (!strcmp(allowed_secs[i], name))
            return true;
    return false;
}

const char *boundary_check(uintptr_t target, int kind)
{
    DWORD64 ib = 0;
    PRUNTIME_FUNCTION fe = RtlLookupFunctionEntry(target, &ib, nullptr);
    if (!fe)
        return "no_pdata";
    uintptr_t fs = ib + fe->BeginAddress, fend = ib + fe->EndAddress;
    if (kind == K_FUNC) {
        uint8_t ui;
        if (!safe_read(&ui, ib + fe->UnwindData, 1))
            return "unwind_unreadable";
        if (target != fs || (ui >> 3) & UNW_FLAG_CHAININFO)
            return "not_function_start";
        return nullptr;
    }
    ZydisDecoder dec;
    ZydisDecoderInit(&dec, ZYDIS_MACHINE_MODE_LONG_64, ZYDIS_STACK_WIDTH_64);
    uintptr_t p = fs;
    if (fend - fs > (1u << 20))
        return "function_too_large";
    while (p < target) {
        uint8_t b[16];
        size_t avail = fend - p < 16 ? fend - p : 16;
        if (!safe_read(b, p, avail))
            return "code_unreadable";
        ZydisDecoderContext ctx;
        ZydisDecodedInstruction in;
        if (ZYAN_FAILED(ZydisDecoderDecodeInstruction(&dec, &ctx, b, avail, &in)))
            return "decode_failed";
        p += in.length;
    }
    return p == target ? nullptr : "not_instruction_boundary";
}

bool sha256_file(const wchar_t *path, char out[65], uint64_t *ms)
{
    uint64_t t0 = now_ms();
    BCRYPT_ALG_HANDLE alg = nullptr;
    BCRYPT_HASH_HANDLE h = nullptr;
    HANDLE f = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
                           OPEN_EXISTING, FILE_FLAG_SEQUENTIAL_SCAN, nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return false;
    bool ok = BCryptOpenAlgorithmProvider(&alg, BCRYPT_SHA256_ALGORITHM, nullptr, 0) >= 0 &&
              BCryptCreateHash(alg, &h, nullptr, 0, nullptr, 0, 0) >= 0;
    const DWORD CH = 4 << 20;
    uint8_t *buf = ok ? (uint8_t *)VirtualAlloc(nullptr, CH, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) : nullptr;
    DWORD got;
    while (ok && buf && ReadFile(f, buf, CH, &got, nullptr) && got)
        ok = BCryptHashData(h, buf, got, 0) >= 0;
    uint8_t dig[32];
    ok = ok && buf && BCryptFinishHash(h, dig, 32, 0) >= 0;
    for (int i = 0; ok && i < 32; i++) {
        out[2 * i] = "0123456789abcdef"[dig[i] >> 4];
        out[2 * i + 1] = "0123456789abcdef"[dig[i] & 15];
    }
    out[64] = 0;
    if (buf)
        VirtualFree(buf, 0, MEM_RELEASE);
    if (h)
        BCryptDestroyHash(h);
    if (alg)
        BCryptCloseAlgorithmProvider(alg, 0);
    CloseHandle(f);
    *ms = now_ms() - t0;
    return ok;
}

void capture_sections()
{
    auto dos = (const IMAGE_DOS_HEADER *)mod_base;
    auto nt = (const IMAGE_NT_HEADERS *)(mod_base + dos->e_lfanew);
    pref_base = nt->OptionalHeader.ImageBase;
    mod_end = mod_base + nt->OptionalHeader.SizeOfImage;
    auto s = IMAGE_FIRST_SECTION(nt);
    for (int i = 0; i < nt->FileHeader.NumberOfSections && nsecs < 96; i++, s++) {
        Sec &d = secs[nsecs++];
        memcpy(d.name, s->Name, 8);
        d.name[8] = 0;
        d.start = mod_base + s->VirtualAddress;
        d.end = d.start + (s->Misc.VirtualSize ? s->Misc.VirtualSize : s->SizeOfRawData);
        d.flags = s->Characteristics;
    }
}

char *read_file(const wchar_t *path, DWORD *len)
{
    HANDLE f = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, 0, nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return nullptr;
    DWORD n = GetFileSize(f, nullptr), got = 0;
    char *b = n < (16u << 20) ? (char *)malloc(n + 1) : nullptr;
    if (b && (!ReadFile(f, b, n, &got, nullptr) || got != n)) {
        free(b);
        b = nullptr;
    }
    CloseHandle(f);
    if (b)
        b[n] = 0;
    *len = n;
    return b;
}

void reject(Hook &h, const char *sig, const char *why, const char *detail = nullptr)
{
    LineBuf l;
    l.begin('R');
    l.o.s(" hook=");
    l.o.s(h.id[0] ? h.id : "?");
    l.o.s(" sig=");
    l.o.s(sig ? sig : "?");
    l.o.s(" status=rejected reason=");
    l.o.s(why);
    if (detail) {
        l.o.c(' ');
        l.o.s(detail);
    }
    l.end();
    api->log("[trace] hook %s rejected: %s %s", h.id, why, detail ? detail : "");
}

void setup_hook(Hook &h, const J *hj, int index, bool build_ok)
{
    const char *err = nullptr;
    const char *id = jstr(hj->get("id"), nullptr);
    if (!id || strlen(id) >= sizeof h.id) {
        reject(h, nullptr, "bad_id");
        return;
    }
    strcpy(h.id, id);
    strncpy(h.test, jstr(hj->get("test"), "-"), sizeof h.test - 1);
    const J *sj = hj->get("sig");
    const char *sig_id = sj ? jstr(sj->get("id"), "?") : "?";
    strncpy(h.sig, sig_id, sizeof h.sig - 1);
    if (!jbool(hj->get("enabled"), true)) {
        reject(h, sig_id, "disabled");
        return;
    }
    if (!sj || sj->k != J::OBJ) {
        reject(h, sig_id, "no_signature");
        return;
    }
    const char *kind = jstr(hj->get("kind"), jstr(sj->get("kind"), "mid"));
    h.kind = !strcmp(kind, "func_start") ? K_FUNC : K_MID;
    if (strcmp(kind, "mid") && strcmp(kind, "func_start")) {
        reject(h, sig_id, "bad_kind", kind);
        return;
    }
    const J *regs = hj->get("regs");
    for (int k = 0; regs && regs->k == J::ARR && k < regs->n; k++) {
        const char *r = jstr(&regs->items[k], "");
        int rid = reg_id(r, strlen(r));
        if (rid < 0 || h.nregs == MAX_REGS) {
            reject(h, sig_id, "bad_register", r);
            return;
        }
        h.regs[h.nregs++] = (int8_t)rid;
    }
    h.nargs = (int)jnum(hj->get("args"), 0);
    if (h.nargs < 0 || h.nargs > MAX_ARGS || (h.nargs && h.kind != K_FUNC)) {
        reject(h, sig_id, "bad_args", "args need kind func_start and 0..8");
        return;
    }
    const J *fields = hj->get("fields");
    for (int k = 0; fields && fields->k == J::ARR && k < fields->n; k++) {
        if (h.nfields == MAX_FIELDS) {
            reject(h, sig_id, "too_many_fields");
            return;
        }
        if (!load_field(&fields->items[k], &h.fields[h.nfields], &err)) {
            reject(h, sig_id, "bad_field", err);
            return;
        }
        h.nfields++;
    }
    const J *when = hj->get("when");
    for (int k = 0; when && when->k == J::ARR && k < when->n; k++) {
        if (h.nconds == MAX_CONDS) {
            reject(h, sig_id, "too_many_conditions");
            return;
        }
        if (!load_cond(jstr(&when->items[k], ""), &h.conds[h.nconds], &err)) {
            reject(h, sig_id, "bad_when", err);
            return;
        }
        h.nconds++;
    }
    h.ret = jbool(hj->get("ret"), false);
    h.bt = (int)jnum(hj->get("bt"), 0);
    if (h.bt < 0 || h.bt > MAX_BT) {
        reject(h, sig_id, "bad_bt", "0..32");
        return;
    }
    if (h.ret && h.kind != K_FUNC) {
        reject(h, sig_id, "bad_ret", "ret needs kind func_start");
        return;
    }
    h.max_per_sec = jnum(hj->get("max_per_sec"), 50);
    h.max_events = jnum(hj->get("max_events"), 2000);

    const char *section = jstr(sj->get("section"), ".xtls");
    const char *pattern = jstr(sj->get("pattern"), nullptr);
    if (!sec_allowed(section)) {
        reject(h, sig_id, "section_not_allowed", section);
        return;
    }
    if (!pattern) {
        reject(h, sig_id, "no_pattern");
        return;
    }
    uintptr_t match = 0;
    int hits = api->scan(section, pattern, &match);
    if (hits != 1) {
        char d[32];
        wsprintfA(d, "hits=%d", hits);
        reject(h, sig_id, hits ? "not_unique" : "no_match", d);
        return;
    }
    int64_t anchor = (int64_t)jnum(sj->get("anchor"), 0);
    const J *rj = sj->get("resolve");
    const char *rk = rj ? jstr(rj->get("kind"), "addr") : "addr";
    uintptr_t target = match + anchor;
    if (!strcmp(rk, "rel32")) {
        int32_t rel;
        if (!safe_read(&rel, target, 4)) {
            reject(h, sig_id, "unreadable_rel32");
            return;
        }
        target = target + 4 + rel;
    } else if (strcmp(rk, "addr")) {
        reject(h, sig_id, "resolve_not_code", rk);
        return;
    }
    const Sec *ts = sec_of(target);
    if (!ts || !sec_allowed(ts->name) || !(ts->flags & IMAGE_SCN_MEM_EXECUTE)) {
        reject(h, sig_id, "target_outside_allowed_code", ts ? ts->name : "(no section)");
        return;
    }
    char where[96];
    {
        Out w{where, 0, (int)sizeof where - 1, false};
        w.s("match=");
        w.hex(match - mod_base + pref_base);
        w.s(" target=");
        w.hex(target - mod_base + pref_base);
        where[w.n] = 0;
    }
    const char *exp = jstr(sj->get("expect"), nullptr);
    if (exp) {
        uint8_t want[32], have[32];
        int n;
        if (!parse_hex_bytes(exp, want, &n, 32) || !safe_read(have, target, n)) {
            reject(h, sig_id, "bad_expect", where);
            return;
        }
        if (memcmp(want, have, n)) {
            reject(h, sig_id, have[0] == 0xE9 || (have[0] == 0xFF && have[1] == 0x25) ? "already_patched" : "expect_mismatch",
                   where);
            return;
        }
    }
    const J *rva = sj->get("rva");
    if (rva && build_ok && (uint64_t)(target - mod_base) != jnum(rva, 0)) {
        reject(h, sig_id, "rva_mismatch", where);
        return;
    }
    if (!jbool(hj->get("skip_boundary_check"), false)) {
        if (const char *why = boundary_check(target, h.kind)) {
            reject(h, sig_id, why, where);
            return;
        }
    }
    h.target = target;
    auto created = safetyhook::MidHook::create((void *)target, callbacks[index]);
    if (!created) {
        char d[64];
        wsprintfA(d, "safetyhook_error=%d", (int)created.error().type);
        reject(h, sig_id, "install_failed", d);
        return;
    }
    h.mh = std::move(*created);
    h.installed = true;
    LineBuf l;
    l.begin('R');
    l.o.s(" hook=");
    l.o.s(h.id);
    l.o.s(" test=");
    l.o.s(h.test);
    l.o.s(" sig=");
    l.o.s(sig_id);
    l.o.s(" status=installed kind=");
    l.o.s(kind);
    l.o.c(' ');
    l.o.s(where);
    l.o.s(" rva=");
    l.o.hex(target - mod_base);
    l.o.s(" fields=");
    l.o.u(h.nfields);
    l.o.s(" when=");
    l.o.u(h.nconds);
    l.o.s(" max_per_sec=");
    l.o.u(h.max_per_sec);
    l.o.s(" max_events=");
    l.o.u(h.max_events);
    l.end();
    api->log("[trace] hook %s installed at %s", h.id, where);
}

int init()
{
    J root;
    wchar_t cfg_path[MAX_PATH * 2];
    wsprintfW(cfg_path, L"%s\\swofty\\trace.json", game_dir);
    DWORD clen = 0;
    char *cfg = read_file(cfg_path, &clen);
    open_log(true);
    {
        LineBuf l;
        l.begin('H');
        l.o.s(" plugin=swofty_trace version=");
        l.o.u(PLUGIN_VERSION);
        l.o.s(" pid=");
        l.o.u(GetCurrentProcessId());
        SYSTEMTIME t;
        GetLocalTime(&t);
        char ts[40];
        wsprintfA(ts, " local=%04u-%02u-%02uT%02u:%02u:%02u", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond);
        l.o.s(ts);
        l.o.s(" image_base=");
        l.o.hex(mod_base);
        l.o.s(" preferred_base=");
        l.o.hex(pref_base);
        l.end();
    }
    if (!cfg) {
        hlog('H', "config=missing status=idle");
        api->log("[trace] no swofty/trace.json, tracing idle");
        return 1;
    }
    JParser jp{cfg, cfg};
    if (!jp.val(root) || root.k != J::OBJ) {
        char d[96];
        wsprintfA(d, "config=invalid error=\"%s\" at_byte=%d status=idle", jp.err ? jp.err : "not an object",
                  (int)(jp.p - cfg));
        hlog('H', d);
        api->log("[trace] trace.json invalid: %s", d);
        return 0;
    }
    if (jnum(root.get("schema"), 0) != 1) {
        hlog('H', "config=unsupported_schema status=idle");
        return 0;
    }
    const J *lg = root.get("log"), *lim = root.get("limits");
    if (lg) {
        max_log_bytes = jnum(lg->get("max_mb"), 64) << 20;
        if (max_log_bytes > (1024ull << 20))
            max_log_bytes = 1024ull << 20;
        rotate_on_full = !strcmp(jstr(lg->get("on_full"), "stop"), "rotate");
        flush_ms = (uint32_t)jnum(lg->get("flush_ms"), 100);
        if (flush_ms < 10)
            flush_ms = 10;
        summary_every_s = (uint32_t)jnum(lg->get("summary_every_s"), 60);
    }
    if (lim) {
        max_events_total = jnum(lim->get("max_events_total"), 200000);
        default_max_read = (int)jnum(lim->get("max_read"), 64);
        if (default_max_read < 1 || default_max_read > HARD_MAX_READ)
            default_max_read = 64;
    }
    const J *as = root.get("allowed_sections");
    if (as && as->k == J::ARR) {
        for (int i = 0; i < as->n && nallowed < 8; i++)
            strncpy(allowed_secs[nallowed++], jstr(&as->items[i], ""), 8);
    } else {
        strcpy(allowed_secs[nallowed++], ".xtls");
    }

    const J *b = root.get("build");
    WIN32_FILE_ATTRIBUTE_DATA fa;
    uint64_t size = 0;
    if (GetFileAttributesExW(exe_path, GetFileExInfoStandard, &fa))
        size = ((uint64_t)fa.nFileSizeHigh << 32) | fa.nFileSizeLow;
    auto nt = (const IMAGE_NT_HEADERS *)(mod_base + ((const IMAGE_DOS_HEADER *)mod_base)->e_lfanew);
    uint64_t stamp = nt->FileHeader.TimeDateStamp;
    uint64_t want_size = b ? jnum(b->get("size"), 0) : 0, want_stamp = b ? jnum(b->get("pe_timestamp"), 0) : 0;
    const char *want_sha = b ? jstr(b->get("sha256"), "") : "";
    bool build_ok = b && size == want_size && stamp == want_stamp;
    char sha[65] = "skipped";
    uint64_t sha_ms = 0;
    bool check_sha = jbool(root.get("check_sha256"), true) && *want_sha;
    if (check_sha) {
        if (!sha256_file(exe_path, sha, &sha_ms))
            strcpy(sha, "error");
        build_ok = build_ok && !_strnicmp(sha, want_sha, strlen(want_sha));
    }
    bool allow_mismatch = jbool(root.get("allow_build_mismatch"), false);
    {
        LineBuf l;
        l.begin('H');
        l.o.s(" build size=");
        l.o.u(size);
        l.o.s(" want_size=");
        l.o.u(want_size);
        l.o.s(" pe_timestamp=");
        l.o.u(stamp);
        l.o.s(" want_pe_timestamp=");
        l.o.u(want_stamp);
        l.o.s(" sha256=");
        l.o.s(sha);
        l.o.s(" want_sha256=");
        l.o.s(*want_sha ? want_sha : "-");
        l.o.s(" sha_ms=");
        l.o.u(sha_ms);
        l.o.s(build_ok ? " result=match" : allow_mismatch ? " result=MISMATCH allowed=1" : " result=MISMATCH allowed=0");
        l.end();
    }
    api->log("[trace] build check: %s", build_ok ? "match" : "MISMATCH");
    if (!build_ok && !allow_mismatch) {
        hlog('H', "status=idle reason=build_mismatch installed=0");
        return 0;
    }

    const J *hj = root.get("hooks");
    int n = hj && hj->k == J::ARR ? hj->n : 0;
    if (n > MAX_HOOKS) {
        char d[64];
        wsprintfA(d, "too_many_hooks=%d max=%d, extra ignored", n, MAX_HOOKS);
        hlog('W', d);
        n = MAX_HOOKS;
    }
    ring = (Slot *)VirtualAlloc(nullptr, sizeof(Slot) * RING_SLOTS, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    for (int i = 0; i < RING_SLOTS; i++)
        ring[i].seq.store(i);
    hooks = new Hook[MAX_HOOKS]();
    AddVectoredExceptionHandler(1, veh);
    int installed = 0;
    for (int i = 0; i < n; i++) {
        setup_hook(hooks[i], &hj->items[i], i, build_ok);
        nhooks++;
        installed += hooks[i].installed;
    }
    char d[96];
    wsprintfA(d, "status=active hooks=%d installed=%d rejected=%d", n, installed, n - installed);
    hlog('H', d);
    api->log("[trace] %s", d);
    HANDLE t = CreateThread(nullptr, 0, writer, nullptr, 0, nullptr);
    if (t)
        CloseHandle(t);
    return 1;
}

} // namespace

extern "C" __declspec(dllexport) int SwoftyModInit(const SwoftyApi *a)
{
    api = a;
    if (!a || a->version < 1)
        return 0;
    QueryPerformanceFrequency(&qpf);
    QueryPerformanceCounter(&qpc0);
    InitializeCriticalSection(&file_lock);
    mod_base = api->module_base();
    capture_sections();
    GetModuleFileNameW(nullptr, exe_path, MAX_PATH);
    wcscpy(game_dir, exe_path);
    wchar_t *slash = wcsrchr(game_dir, L'\\');
    if (slash)
        *slash = 0;
    return init();
}

BOOL WINAPI DllMain(HINSTANCE, DWORD reason, LPVOID)
{
    if (reason == DLL_PROCESS_DETACH && ring && log_file != INVALID_HANDLE_VALUE) {
        quitting = 1;
        if (TryEnterCriticalSection(&file_lock)) {
            LeaveCriticalSection(&file_lock);
        } else {
            InitializeCriticalSection(&file_lock);
        }
        drain();
        summary(true);
        hlog('H', "end=1");
        FlushFileBuffers(log_file);
    }
    return TRUE;
}
