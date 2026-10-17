#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    uint8_t head[0x10];
    uint32_t counter;
    uint32_t pad;
    uint8_t *chain;
    uint8_t *name;
    float speed;
} Obj;

volatile uint64_t sink;

uint64_t fake_tick(Obj *o, uint32_t i, const char *label);
uint64_t fake_entry(uint64_t a, uint64_t b, uint64_t c, uint64_t d, uint64_t e);
void dup_a(void);
void dup_b(void);

void tick_impl(Obj *o, uint32_t i) { sink += o->counter + i; }

__asm__(".section .xtls,\"xr\"\n"
        ".globl fake_tick\n"
        ".def fake_tick; .scl 2; .type 32; .endef\n"
        ".seh_proc fake_tick\n"
        "fake_tick:\n"
        "  push %rbx\n"
        "  .seh_pushreg %rbx\n"
        "  push %rsi\n"
        "  .seh_pushreg %rsi\n"
        "  sub $0x28, %rsp\n"
        "  .seh_stackalloc 0x28\n"
        "  .seh_endprologue\n"
        "  .byte 0x48,0x89,0xCB\n"
        "  .byte 0x89,0xD6\n"
        "  .byte 0x8B,0x43,0x10\n"
        "  .byte 0x83,0xC0,0x01\n"
        "  .byte 0x89,0x43,0x10\n"
        "  .byte 0x0F,0xB7,0x43,0x08\n"
        "  mov %rbx, %rcx\n"
        "  mov %esi, %edx\n"
        "  call tick_impl\n"
        "  mov %esi, %eax\n"
        "  add $0x28, %rsp\n"
        "  pop %rsi\n"
        "  pop %rbx\n"
        "  ret\n"
        ".seh_endproc\n"

        ".globl fake_entry\n"
        ".def fake_entry; .scl 2; .type 32; .endef\n"
        ".seh_proc fake_entry\n"
        "fake_entry:\n"
        "  push %rbx\n"
        "  .seh_pushreg %rbx\n"
        "  sub $0x20, %rsp\n"
        "  .seh_stackalloc 0x20\n"
        "  .seh_endprologue\n"
        "  .byte 0x48,0x8D,0x04,0x11\n"
        "  .byte 0x4C,0x01,0xC0\n"
        "  .byte 0x4C,0x01,0xC8\n"
        "  .byte 0x48,0x8B,0x5C,0x24,0x50\n"
        "  .byte 0x48,0x01,0xD8\n"
        "  add $0x20, %rsp\n"
        "  pop %rbx\n"
        "  ret\n"
        ".seh_endproc\n"

        ".globl dup_a\n"
        ".def dup_a; .scl 2; .type 32; .endef\n"
        "dup_a:\n"
        "  .byte 0x48,0x31,0xC0,0x48,0x83,0xC0,0x2A,0x48,0x83,0xE8,0x15,0x48,0x83,0xF0,0x07\n"
        "  ret\n"
        ".globl dup_b\n"
        ".def dup_b; .scl 2; .type 32; .endef\n"
        "dup_b:\n"
        "  .byte 0x48,0x31,0xC0,0x48,0x83,0xC0,0x2A,0x48,0x83,0xE8,0x15,0x48,0x83,0xF0,0x07\n"
        "  ret\n"
        ".text\n");

static uint8_t *nestr(const char *s)
{
    size_t n = strlen(s);
    uint8_t *p = calloc(1, 16 + n);
    uint32_t h = 5381;
    for (size_t i = 0; i < n; i++)
        h = h * 33 + (uint8_t)s[i];
    memcpy(p, &h, 4);
    *(uint16_t *)(p + 4) = 1;
    memcpy(p + 9, s, n);
    return p;
}

__attribute__((noinline)) uint64_t level2(uint64_t b) { return fake_entry(1, b, 3, 4, 5) + 1; }
__attribute__((noinline)) uint64_t level1(uint64_t b) { return level2(b) * 2; }

static LRESULT CALLBACK wndproc(HWND h, UINT m, WPARAM w, LPARAM l) { return DefWindowProcW(h, m, w, l); }

static void pump(DWORD ms)
{
    for (DWORD t0 = GetTickCount(); GetTickCount() - t0 < ms;) {
        MSG m;
        while (PeekMessageW(&m, NULL, 0, 0, PM_REMOVE))
            DispatchMessageW(&m);
        Sleep(10);
    }
}

int main(void)
{
    setvbuf(stdout, NULL, _IONBF, 0);
    WNDCLASSW wc = {.lpfnWndProc = wndproc, .hInstance = GetModuleHandleW(NULL), .lpszClassName = L"FakeF1Trace"};
    RegisterClassW(&wc);
    HWND w = CreateWindowExW(0, L"FakeF1Trace", L"F1 25", WS_OVERLAPPEDWINDOW, 0, 0, 320, 200, NULL, NULL, wc.hInstance, NULL);
    ShowWindow(w, SW_SHOW);
    DWORD dummy;
    printf("version.dll import check: %lu\n", GetFileVersionInfoSizeW(L"C:\\windows\\system32\\kernel32.dll", &dummy));
    pump(5000);

    uint8_t *chain_ok = calloc(1, 32);
    *(uint64_t *)(chain_ok + 8) = 0x1122334455667788ull;
    Obj ok = {.counter = 41, .chain = chain_ok, .name = nestr("career"), .speed = 312.5f};
    memcpy(ok.head, "OBJHEAD!", 8);
    Obj bad = ok;
    bad.chain = (uint8_t *)0x10;
    bad.name = NULL;
    uint8_t *page = VirtualAlloc(NULL, 4096, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    *(uint64_t *)(page + 8) = 0xabcdef;
    Obj veh = ok;
    veh.chain = page;

    fake_tick(&ok, 1, "first");
    fake_tick(&bad, 2, "badptr");
    fake_tick(&veh, 3, "veh-armed");
    DWORD old;
    VirtualProtect(page, 4096, PAGE_NOACCESS, &old);
    fake_tick(&veh, 4, "veh-fault");
    printf("survived guarded reads\n");

    for (int i = 0; i < 300; i++)
        fake_tick(&ok, 100 + i, "flood1");
    Sleep(1100);
    for (int i = 0; i < 300; i++)
        fake_tick(&ok, 400 + i, "flood2");
    Sleep(1100);
    for (int i = 0; i < 300; i++)
        fake_tick(&ok, 700 + i, "flood3");

    uint64_t r = 0;
    for (int i = 0; i < 3; i++)
        r += level1(7);
    for (int i = 0; i < 2; i++)
        r += level1(2);
    dup_a();
    dup_b();
    printf("fake_entry sum %llu, counter %u\n", (unsigned long long)r, ok.counter);
    pump(1500);
    DestroyWindow(w);
    return 0;
}
