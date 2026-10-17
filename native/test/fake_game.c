#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct NeStr NeStr;
typedef struct { int32_t type, pad; NeStr *str; } Action;
typedef struct { uint8_t *begin, *end, *cap; } Vec;
typedef struct { void *vt; char title[64], description[64]; int title_localised, description_localised; } VmNode;
typedef struct { const char *name; uint32_t index; } AdderCall;

void *g_registry = (void *)0x5157;
uint64_t out_r12, out_r15;
static uint8_t list[0x100];
static AdderCall calls[32];
static int ncalls, fails;
static void *career_vt_storage[40], *other_vt_storage[40];
#define CAREER_VT (career_vt_storage + 1)
#define OTHER_VT (other_vt_storage + 1)

void fake_builder(void *list, const uint8_t *flags, uint32_t start);
void fake_base_populate(uint8_t *item, void *node, uint32_t a7);
uint32_t fake_nestr_from(NeStr **out, const char *s, uint32_t flags, uintptr_t len);

static const char *chars(const NeStr *s) { return s ? (const char *)s + 9 : ""; }

uint32_t nestr_impl(NeStr **out, const char *s)
{
    size_t n = strlen(s);
    uint8_t *p = calloc(1, 16 + n);
    uint32_t h = 5381;
    for (size_t i = 0; i < n; i++)
        h = h * 33 + (uint8_t)s[i];
    memcpy(p, &h, 4);
    *(uint16_t *)(p + 4) = 1;
    memcpy(p + 9, s, n);
    *out = (NeStr *)p;
    return 1;
}

static VmNode *node_of(void *parent, void *reg)
{
    if (reg != g_registry) {
        printf("FAIL: registry %p != %p\n", reg, g_registry);
        fails++;
    }
    return (VmNode *)((uint8_t *)parent - 8);
}

void *fake_vm_set_string(void *reg, void *parent, const char *name, NeStr **value, int overwrite)
{
    VmNode *n = node_of(parent, reg);
    if (!strcmp(name, "title"))
        snprintf(n->title, sizeof n->title, "%s", chars(*value));
    else if (!strcmp(name, "description"))
        snprintf(n->description, sizeof n->description, "%s", chars(*value));
    (void)overwrite;
    return &g_registry;
}

void *fake_vm_set_bool(void *reg, void *parent, const char *name, uint8_t *value, int overwrite)
{
    VmNode *n = node_of(parent, reg);
    if (!strcmp(name, "title_localised"))
        n->title_localised = *value;
    else if (!strcmp(name, "description_localised"))
        n->description_localised = *value;
    (void)overwrite;
    return &g_registry;
}

void populate_impl(uint8_t *item, void *node, uint32_t a7)
{
    VmNode *n = calloc(1, sizeof *n);
    n->title_localised = n->description_localised = 1;
    *(VmNode **)(item + 0x20) = n;
    (void)node, (void)a7;
}

static void career_populate(uint8_t *item, void *node, uint32_t a7)
{
    fake_base_populate(item, node, a7);
    NeStr *t = NULL;
    fake_nestr_from(&t, "lng_mode_career", 0, 0);
    fake_vm_set_string(g_registry, *(uint8_t **)(item + 0x20) + 8, "title", &t, 1);
}

static Action *id_action(uint8_t *item, Action *out)
{
    out->type = 1;
    out->str = *(NeStr **)(item + 0x10);
    return out;
}

static uintptr_t stub_zero(void) { return 0; }

static void add(void *l, uint32_t index, const char *name, void **vt)
{
    Vec *v = (Vec *)((uint8_t *)l + 0x78);
    uint8_t *tile = calloc(1, 0xd8), *item = calloc(1, 0x148);
    *(void ***)item = vt;
    fake_nestr_from((NeStr **)(item + 0x10), name, 0, 0);
    *(uint8_t **)(tile + 0xd0) = item;
    ((void **)v->end)[0] = tile + 8;
    ((void **)v->end)[1] = tile;
    v->end += 16;
    calls[ncalls++] = (AdderCall){name, index & 0xff};
}

void career_adder(void *l, uint32_t index) { add(l, index, "career", CAREER_VT); }
void movie_adder(void *l, uint32_t index) { add(l, index, "zenith_button", OTHER_VT); }
void world_adder(void *l, uint32_t index) { add(l, index, "f1_world", OTHER_VT); }

__asm__(
    ".section .xtls,\"xr\"\n"
    ".globl fake_builder\nfake_builder:\n"
    ".byte 0x41,0x57,0x41,0x56,0x41,0x55,0x41,0x54,0x56,0x57,0x55,0x53\n"
    ".byte 0x48,0x81,0xEC,0x88,0x02,0x00,0x00\n"
    ".byte 0xC5,0xF8,0x29,0xB4,0x24,0x70,0x02,0x00,0x00\n"
    ".byte 0x4C,0x89,0x8C,0x24,0xE8,0x02,0x00,0x00\n"
    "mov %rcx, %r14\nmov %rdx, %rsi\nmov %r8d, %r15d\n"
    ".byte 0x45,0x8D,0x67,0x01,0x4C,0x89,0xF1,0x44,0x89,0xE2,0xE8\n.long career_adder - (. + 4)\n"
    ".byte 0xF6,0x46,0x05,0x04,0x74,0x12\n"
    ".byte 0x41,0x80,0xC7,0x02,0x4C,0x89,0xF1,0x44,0x89,0xFA,0xE8\n.long movie_adder - (. + 4)\n"
    ".byte 0x45,0x89,0xFC\n"
    ".byte 0x45,0x8D,0x7C,0x24,0x01,0x4C,0x89,0xF1,0x44,0x89,0xFA,0xE8\n.long world_adder - (. + 4)\n"
    "mov %r12, out_r12(%rip)\nmov %r15, out_r15(%rip)\n"
    "vmovaps 0x270(%rsp), %xmm6\nadd $0x288, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r12\npop %r13\npop %r14\npop %r15\nret\n"

    ".globl fake_base_populate\nfake_base_populate:\n"
    ".byte 0x41,0x57,0x41,0x56,0x41,0x55,0x41,0x54,0x56,0x57,0x55,0x53\n"
    ".byte 0x48,0x81,0xEC,0x88,0x00,0x00,0x00\n"
    ".byte 0xC5,0xF8,0x29,0x74,0x24,0x70,0x44,0x89,0x44,0x24,0x6C\n"
    "call populate_impl\njmp 1f\n"
    ".org fake_base_populate+0x53, 0xCC\nmov g_registry(%rip), %rcx\n"
    ".org fake_base_populate+0x69, 0xCC\ncall fake_vm_set_string\n"
    ".org fake_base_populate+0x28e, 0xCC\ncall fake_vm_set_bool\n"
    "1:\nvmovaps 0x70(%rsp), %xmm6\nadd $0x88, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r12\npop %r13\npop %r14\npop %r15\nret\n"

    ".globl fake_nestr_from\nfake_nestr_from:\n"
    ".byte 0x41,0x57,0x41,0x56,0x41,0x54,0x56,0x57,0x55,0x53,0x48,0x83,0xEC,0x50,0x44,0x89,0xC6,0x48,0x89,0xCF\n"
    "call nestr_impl\nadd $0x50, %rsp\n"
    "pop %rbx\npop %rbp\npop %rdi\npop %rsi\npop %r12\npop %r14\npop %r15\nret\n"
    ".section .data1,\"dwx\"\n"
    ".globl vm_thunk_register\nvm_thunk_register:\n"
    "sub $0x28, %rsp\ncall *__imp_RegisterClassExW(%rip)\nadd $0x28, %rsp\nret\n"
    ".text\n");
ATOM vm_thunk_register(const WNDCLASSEXW *wc);

static void expect(int ok, const char *what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    fails += !ok;
}

static void run_build(int movie, uint32_t start)
{
    uint8_t flags[16] = {0};
    flags[5] = movie ? 4 : 0;
    Vec *v = (Vec *)(list + 0x78);
    if (!v->begin)
        v->begin = v->end = calloc(64, 16);
    v->end = v->begin;
    ncalls = 0;
    fake_builder(list, flags, start);
    printf("build movie=%d start=%u -> r12=%llu r15=%llu, %d adder calls:", movie, start,
           (unsigned long long)out_r12, (unsigned long long)out_r15, ncalls);
    for (int i = 0; i < ncalls; i++)
        printf(" %s=%u", calls[i].name, calls[i].index);
    printf("\n");
    for (uint8_t *e = v->begin; e < v->end; e += 16) {
        uint8_t *tile = ((uint8_t **)e)[1], *item = *(uint8_t **)(tile + 0xd0);
        void **vt = *(void ***)item;
        ((void (*)(uint8_t *, void *, uint32_t))vt[4])(item, tile + 8, 0);
        Action a = {0};
        ((Action * (*)(uint8_t *, Action *)) vt[5])(item, &a);
        VmNode *n = *(VmNode **)(item + 0x20);
        printf("  item %-14s title='%s' loc=%d desc='%s' dloc=%d action={%d,'%s'}\n", chars(*(NeStr **)(item + 0x10)),
               n ? n->title : "", n ? n->title_localised : -1, n ? n->description : "", n ? n->description_localised : -1,
               a.type, chars(a.str));
    }
}

static LRESULT CALLBACK wndproc(HWND h, UINT m, WPARAM w, LPARAM l) { return DefWindowProcW(h, m, w, l); }

extern ATOM(WINAPI *__imp_RegisterClassExW)(const WNDCLASSEXW *);
extern HWND(WINAPI *__imp_CreateWindowExW)(DWORD, LPCWSTR, LPCWSTR, DWORD, int, int, int, int, HWND, HMENU, HINSTANCE,
                                           LPVOID);

int main(int argc, char **argv)
{
    int fallback = argc > 1 && !strcmp(argv[1], "fallback"), vm = argc > 1 && !strcmp(argv[1], "vm");
    setvbuf(stdout, NULL, _IONBF, 0);
    for (int i = 0; i < 40; i++)
        career_vt_storage[i] = other_vt_storage[i] = (void *)stub_zero;
    CAREER_VT[4] = (void *)career_populate;
    CAREER_VT[5] = (void *)id_action;
    OTHER_VT[4] = (void *)fake_base_populate;
    OTHER_VT[5] = (void *)id_action;

    DWORD dummy, size = GetFileVersionInfoSizeW(L"C:\\windows\\system32\\kernel32.dll", &dummy);
    void *buf = malloc(size ? size : 1);
    VS_FIXEDFILEINFO *fi = NULL;
    UINT len = 0;
    int vok = size && GetFileVersionInfoW(L"C:\\windows\\system32\\kernel32.dll", 0, size, buf) &&
              VerQueryValueW(buf, L"\\", (void **)&fi, &len) && fi;
    expect(vok, "version.dll forwarding (GetFileVersionInfoSizeW/GetFileVersionInfoW/VerQueryValueW on kernel32)");
    if (vok)
        printf("      kernel32 version %u.%u.%u.%u\n", HIWORD(fi->dwFileVersionMS), LOWORD(fi->dwFileVersionMS),
               HIWORD(fi->dwFileVersionLS), LOWORD(fi->dwFileVersionLS));

    run_build(1, 1);
    expect(ncalls == 3 && calls[1].index == 3, "before mods load: vanilla order career=2 movie=3 world=4");

    HMODULE u32 = GetModuleHandleW(L"user32.dll");
    void *real_rc = (void *)GetProcAddress(u32, "RegisterClassExW"), *real_cw = (void *)GetProcAddress(u32, "CreateWindowExW");
    expect(fallback || ((void *)__imp_RegisterClassExW != real_rc && (void *)__imp_CreateWindowExW != real_cw),
           "IAT trigger armed on USER32!RegisterClassExW and CreateWindowExW before the window exists");
    ATOM(WINAPI * rc)(const WNDCLASSEXW *) = fallback ? (void *)real_rc : (void *)__imp_RegisterClassExW;
    HWND(WINAPI * cw)(DWORD, LPCWSTR, LPCWSTR, DWORD, int, int, int, int, HWND, HMENU, HINSTANCE, LPVOID) =
        fallback ? (void *)real_cw : (void *)__imp_CreateWindowExW;
    WNDCLASSEXW wc = {.cbSize = sizeof wc, .lpfnWndProc = wndproc, .hInstance = GetModuleHandleW(NULL),
                      .lpszClassName = L"EGO_WindowClass"};
    if (vm) {
        WNDCLASSEXW vc = wc;
        vc.lpszClassName = L"VmThunkClass";
        vm_thunk_register(&vc);
        expect((void *)__imp_RegisterClassExW != real_rc, "vm mode: call from a writable+exec section does not fire the trigger");
        run_build(1, 1);
        expect(ncalls == 3, "vm mode: build after the skipped call is still vanilla");
    }
    rc(&wc);
    HWND w = cw(0, L"EGO_WindowClass", L"F1 25", WS_OVERLAPPEDWINDOW, 0, 0, 320, 200, NULL, NULL, wc.hInstance, NULL);
    expect((void *)__imp_RegisterClassExW == real_rc && (void *)__imp_CreateWindowExW == real_cw || fallback,
           "IAT slots restored after the trigger");

    run_build(1, 1);
    if (fallback)
        expect(ncalls == 3, "fallback mode: first build before the window is visible is vanilla");
    else
        expect(ncalls == 4 && calls[1].index == 3,
               "first home build right after window creation already has the mod tile (career=2, mod=3)");

    ShowWindow(w, SW_SHOW);
    for (DWORD t0 = GetTickCount(); GetTickCount() - t0 < 1500;) {
        MSG m;
        while (PeekMessageW(&m, NULL, 0, 0, PM_REMOVE))
            DispatchMessageW(&m);
        Sleep(20);
    }

    run_build(1, 1);
    expect(ncalls == 4, "hooked build with movie: 4 adder calls");
    expect(ncalls == 4 && !strcmp(calls[1].name, "career") && calls[0].index == 2 && calls[1].index == 3 &&
               calls[2].index == 4 && calls[3].index == 5,
           "hooked: career=2, mod tile=3, movie=4, world=5");
    expect(out_r12 == 4 && out_r15 == 5, "hooked: r12/r15 renumbered (4/5)");
    Vec *v = (Vec *)(list + 0x78);
    uint8_t *mod = *(uint8_t **)(((uint8_t **)v->begin)[3] + 0xd0);
    uint8_t *career = *(uint8_t **)(((uint8_t **)v->begin)[1] + 0xd0);
    VmNode *mn = *(VmNode **)(mod + 0x20), *cn = *(VmNode **)(career + 0x20);
    Action a = {0};
    ((Action * (*)(uint8_t *, Action *))(*(void ***)mod)[5])(mod, &a);
    expect(!strcmp(chars(*(NeStr **)(mod + 0x10)), "mod_menu"), "mod item id = mod_menu");
    expect(mn && !strcmp(mn->title, "MOD SETTINGS") && mn->title_localised == 0, "mod item title 'MOD SETTINGS', not localised");
    expect(a.type == 1 && !strcmp(chars(a.str), "mod_menu"), "mod item action {1, mod_menu}");
    expect(cn && !strcmp(cn->title, "lng_mode_career") && cn->title_localised == 1, "career item untouched");
    expect((*(void ***)mod)[0] == CAREER_VT[0] && (*(void ***)mod)[-1] == CAREER_VT[-1], "mod vtable keeps career slots 0 and -1");

    expect((void *)__imp_RegisterClassExW == real_rc && (void *)__imp_CreateWindowExW == real_cw,
           "IAT slots pristine after mods loaded");

    run_build(0, 0);
    expect(ncalls == 3 && calls[0].index == 1 && calls[1].index == 2 && calls[2].index == 3, "hooked without movie: career=1, mod=2, world=3");
    expect(out_r12 == 2 && out_r15 == 3, "hooked without movie: r12/r15 = 2/3");

    printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL PASSED", fails);
    DestroyWindow(w);
    return fails != 0;
}
