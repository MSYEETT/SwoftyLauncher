#include "runtime.hpp"

#include "hud_drawlist.hpp"
#include "hud_guard.hpp"
#include "hud_png.hpp"

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <d3d12.h>
#include <d3dcompiler.h>
#include <dxgi1_6.h>

#include <algorithm>
#include <atomic>
#include <climits>
#include <cmath>
#include <cstdio>
#include <mutex>
#include <vector>

std::string settings_addon(const void *from, std::string &dll);

namespace {

struct Caller {
    std::string id, dll;
};

Caller caller(const void *from)
{
    Caller c;
    c.id = settings_addon(from, c.dll);
    return c;
}

bool gate_on()
{
    return runtime::enabled("overlay", nullptr);
}

std::mutex reg_lock;
hud::Registry reg;
bool implemented;

std::string addon_image(const Caller &c, const char *file, std::string &why)
{
    if (c.id.empty())
        return why = "no addon in settings.json lists " + c.dll + ", so it has no folder", std::string();
    std::string f = file ? file : "";
    std::replace(f.begin(), f.end(), '\\', '/');
    if (f.empty() || f[0] == '/' || f.find(':') != std::string::npos || ("/" + f + "/").find("/../") != std::string::npos)
        return why = "'" + f + "' is not a relative path inside swofty/addons/" + c.id + "/", std::string();
    std::string rel = "addons/" + c.id + "/" + f;
    uint64_t size = 0;
    if (!runtime::file_size(runtime::under_swofty(rel), &size))
        return why = "swofty/" + rel + " is missing", std::string();
    return rel;
}

// ----- D3D12 present hook + minimal solid-geometry renderer -------------------------------------
// The renderer draws the frame's filled rectangles, rectangle outlines, lines and text as solid-colour
// triangles over the game's back buffer; text is the built-in font's pixel blocks (hud_font.hpp).
// Image commands are recorded by the draw list but this build does not paint them (see
// docs/design/native-libraries.md).
//
// It draws only on a swapchain whose creation it saw (the DXGI factory's CreateSwapChain* slots are
// hooked before the game makes its window) and only on the queue that swapchain was created with, so
// its work runs after the game's frame and before the present on the same queue. Swapchains that a
// frame-generation module created or presents are left alone. A ring of command lists behind one
// fence replaces any wait in Present: a frame whose slot is still on the GPU is skipped. A device
// removal within 10 s of an overlay draw is recorded in swofty/overlay-gpu-removed.json, which keeps
// the overlay off on later launches.

struct Vertex {
    float x, y;
    uint32_t rgba;
};

uint32_t to_le(uint32_t rrggbbaa)
{
    uint32_t r = (rrggbbaa >> 24) & 0xff, g = (rrggbbaa >> 16) & 0xff, b = (rrggbbaa >> 8) & 0xff, a = rrggbbaa & 0xff;
    return r | (g << 8) | (b << 16) | (a << 24);
}

void push_rect(std::vector<Vertex> &v, float x, float y, float w, float h, uint32_t col)
{
    float x1 = x + w, y1 = y + h;
    v.push_back({x, y, col});
    v.push_back({x1, y, col});
    v.push_back({x, y1, col});
    v.push_back({x1, y, col});
    v.push_back({x1, y1, col});
    v.push_back({x, y1, col});
}

void push_line(std::vector<Vertex> &v, float x0, float y0, float x1, float y1, float t, uint32_t col)
{
    float dx = x1 - x0, dy = y1 - y0, len = std::sqrt(dx * dx + dy * dy);
    if (len <= 0)
        return;
    float nx = -dy / len * t * 0.5f, ny = dx / len * t * 0.5f;
    v.push_back({x0 + nx, y0 + ny, col});
    v.push_back({x1 + nx, y1 + ny, col});
    v.push_back({x0 - nx, y0 - ny, col});
    v.push_back({x1 + nx, y1 + ny, col});
    v.push_back({x1 - nx, y1 - ny, col});
    v.push_back({x0 - nx, y0 - ny, col});
}

std::vector<Vertex> tessellate(const hud::Frame &f)
{
    std::vector<Vertex> v;
    for (const auto &c : f.cmds) {
        uint32_t col = to_le(c.color);
        switch (c.op) {
        case hud::Op::rect:
            push_rect(v, c.x, c.y, c.w, c.h, col);
            break;
        case hud::Op::rect_outline: {
            float t = c.thickness;
            push_rect(v, c.x, c.y, c.w, t, col);
            push_rect(v, c.x, c.y + c.h - t, c.w, t, col);
            push_rect(v, c.x, c.y + t, t, c.h - 2 * t, col);
            push_rect(v, c.x + c.w - t, c.y + t, t, c.h - 2 * t, col);
            break;
        }
        case hud::Op::line:
            push_line(v, c.x, c.y, c.w, c.h, c.thickness, col);
            break;
        case hud::Op::text:
            hud::glyph_rects(c.x, c.y, c.font_size, c.text,
                             [&](float x, float y, float w, float h) { push_rect(v, x, y, w, h, col); });
            break;
        case hud::Op::image:
            break;
        }
    }
    return v;
}

template <class T> void release(T *&p)
{
    if (p)
        p->Release();
    p = nullptr;
}

// d3d12.dll / dxgi.dll are resolved at arm time so the core keeps no hard import on them: it loads
// unchanged where the overlay feature is off or D3D12 is absent.
using CreateDeviceFn = PFN_D3D12_CREATE_DEVICE;
using SerializeRootFn = PFN_D3D12_SERIALIZE_ROOT_SIGNATURE;
using CreateFactoryFn = HRESULT(WINAPI *)(REFIID, void **);

CreateDeviceFn d3d12_create_device;
SerializeRootFn d3d12_serialize_root;
CreateFactoryFn dxgi_create_factory;

constexpr UINT MAX_VERTS = 1u << 18;
constexpr UINT SLOTS = 3;
constexpr ULONGLONG STATS_MS = 60000;

std::string module_of(const void *addr, HMODULE *mod = nullptr)
{
    HMODULE m = nullptr;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                            reinterpret_cast<LPCWSTR>(addr), &m) ||
        !m)
        return {};
    if (mod)
        *mod = m;
    char path[MAX_PATH] = {};
    DWORD n = GetModuleFileNameA(m, path, MAX_PATH);
    return std::string(hud::guard::base_name(std::string_view(path, n)));
}

std::string self_module()
{
    static std::string name = module_of(reinterpret_cast<const void *>(&module_of));
    return name;
}

std::string now_text()
{
    SYSTEMTIME t;
    GetLocalTime(&t);
    char b[32];
    snprintf(b, sizeof b, "%04u-%02u-%02u %02u:%02u:%02u", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond);
    return b;
}

struct Chain {
    void *sc = nullptr;
    ID3D12CommandQueue *queue = nullptr;
    std::string how, by, fg;
    bool streamline = false;
    bool told = false;
};

std::mutex chains_lock;
std::vector<Chain> chains;
std::vector<void *> unknown_told;

struct Pso {
    DXGI_FORMAT fmt;
    ID3D12PipelineState *pso;
};

struct Slot {
    ID3D12CommandAllocator *alloc = nullptr;
    ID3D12GraphicsCommandList *list = nullptr;
    UINT64 done_at = 0;
};

struct Renderer {
    ID3D12Device *dev = nullptr;
    ID3D12RootSignature *root = nullptr;
    ID3DBlob *vs = nullptr, *ps = nullptr;
    std::vector<Pso> psos;
    ID3D12DescriptorHeap *rtv_heap = nullptr;
    UINT rtv_inc = 0;
    Slot slots[SLOTS];
    UINT next = 0;
    ID3D12Resource *vbuf = nullptr;
    uint8_t *vmap = nullptr;
    ID3D12Fence *fence = nullptr;
    UINT64 fence_val = 0;
    bool ready = false;
    bool failed = false;

    bool compile(pD3DCompile fn, const char *entry, const char *model, ID3DBlob **out)
    {
        static const char hlsl[] = "cbuffer C:register(b0){float2 inv;}"
                                   "struct I{float2 p:POSITION;float4 c:COLOR;};"
                                   "struct O{float4 p:SV_POSITION;float4 c:COLOR;};"
                                   "O VS(I i){O o;o.p=float4(i.p.x*inv.x-1.0,1.0-i.p.y*inv.y,0,1);o.c=i.c;return o;}"
                                   "float4 PS(O i):SV_TARGET{return i.c;}";
        ID3DBlob *err = nullptr;
        HRESULT hr = fn(hlsl, sizeof hlsl - 1, nullptr, nullptr, nullptr, entry, model, 0, 0, out, &err);
        if (err)
            err->Release();
        return SUCCEEDED(hr) && *out;
    }

    bool make_root()
    {
        HMODULE compiler = LoadLibraryA("d3dcompiler_47.dll");
        auto compile_fn = compiler ? reinterpret_cast<pD3DCompile>(GetProcAddress(compiler, "D3DCompile")) : nullptr;
        if (!compile_fn) {
            swofty::log("[overlay] d3dcompiler_47.dll!D3DCompile is unavailable; overlay rendering off");
            return false;
        }
        if (!compile(compile_fn, "VS", "vs_5_0", &vs) || !compile(compile_fn, "PS", "ps_5_0", &ps)) {
            swofty::log("[overlay] overlay shader compilation failed");
            return false;
        }
        D3D12_ROOT_PARAMETER rp{};
        rp.ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
        rp.Constants.Num32BitValues = 2;
        rp.ShaderVisibility = D3D12_SHADER_VISIBILITY_VERTEX;
        D3D12_ROOT_SIGNATURE_DESC rs{};
        rs.NumParameters = 1;
        rs.pParameters = &rp;
        rs.Flags = D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT;
        ID3DBlob *blob = nullptr, *rserr = nullptr;
        HRESULT hr = d3d12_serialize_root(&rs, D3D_ROOT_SIGNATURE_VERSION_1, &blob, &rserr);
        if (rserr)
            rserr->Release();
        if (SUCCEEDED(hr))
            hr = dev->CreateRootSignature(0, blob->GetBufferPointer(), blob->GetBufferSize(), IID_PPV_ARGS(&root));
        release(blob);
        return SUCCEEDED(hr);
    }

    ID3D12PipelineState *pso_for(DXGI_FORMAT fmt)
    {
        for (auto &p : psos)
            if (p.fmt == fmt)
                return p.pso;
        D3D12_INPUT_ELEMENT_DESC layout[] = {
            {"POSITION", 0, DXGI_FORMAT_R32G32_FLOAT, 0, 0, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0},
            {"COLOR", 0, DXGI_FORMAT_R8G8B8A8_UNORM, 0, 8, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0}};
        D3D12_GRAPHICS_PIPELINE_STATE_DESC pd{};
        pd.pRootSignature = root;
        pd.VS = {vs->GetBufferPointer(), vs->GetBufferSize()};
        pd.PS = {ps->GetBufferPointer(), ps->GetBufferSize()};
        pd.InputLayout = {layout, 2};
        pd.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
        pd.NumRenderTargets = 1;
        pd.RTVFormats[0] = fmt;
        pd.SampleDesc.Count = 1;
        pd.SampleMask = UINT_MAX;
        pd.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
        pd.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
        auto &rt = pd.BlendState.RenderTarget[0];
        rt.BlendEnable = TRUE;
        rt.SrcBlend = D3D12_BLEND_SRC_ALPHA;
        rt.DestBlend = D3D12_BLEND_INV_SRC_ALPHA;
        rt.BlendOp = D3D12_BLEND_OP_ADD;
        rt.SrcBlendAlpha = D3D12_BLEND_ONE;
        rt.DestBlendAlpha = D3D12_BLEND_INV_SRC_ALPHA;
        rt.BlendOpAlpha = D3D12_BLEND_OP_ADD;
        rt.RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
        ID3D12PipelineState *pso = nullptr;
        if (FAILED(dev->CreateGraphicsPipelineState(&pd, IID_PPV_ARGS(&pso))))
            return nullptr;
        psos.push_back({fmt, pso});
        return pso;
    }

    bool make_resources()
    {
        for (auto &s : slots) {
            if (FAILED(dev->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&s.alloc))))
                return false;
            if (FAILED(dev->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, s.alloc, nullptr, IID_PPV_ARGS(&s.list))))
                return false;
            s.list->Close();
        }
        if (FAILED(dev->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&fence))))
            return false;
        D3D12_DESCRIPTOR_HEAP_DESC hd{};
        hd.Type = D3D12_DESCRIPTOR_HEAP_TYPE_RTV;
        hd.NumDescriptors = SLOTS;
        if (FAILED(dev->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&rtv_heap))))
            return false;
        rtv_inc = dev->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
        D3D12_HEAP_PROPERTIES up{};
        up.Type = D3D12_HEAP_TYPE_UPLOAD;
        D3D12_RESOURCE_DESC rd{};
        rd.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
        rd.Width = UINT64(sizeof(Vertex)) * MAX_VERTS * SLOTS;
        rd.Height = 1;
        rd.DepthOrArraySize = 1;
        rd.MipLevels = 1;
        rd.Format = DXGI_FORMAT_UNKNOWN;
        rd.SampleDesc.Count = 1;
        rd.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
        if (FAILED(dev->CreateCommittedResource(&up, D3D12_HEAP_FLAG_NONE, &rd, D3D12_RESOURCE_STATE_GENERIC_READ,
                                                nullptr, IID_PPV_ARGS(&vbuf))))
            return false;
        D3D12_RANGE none{0, 0};
        return SUCCEEDED(vbuf->Map(0, &none, reinterpret_cast<void **>(&vmap)));
    }

    std::vector<ID3D12CommandQueue *> queues;

    bool same_device(ID3D12CommandQueue *q)
    {
        if (std::find(queues.begin(), queues.end(), q) != queues.end())
            return true;
        ID3D12Device *d = nullptr;
        if (FAILED(q->GetDevice(IID_PPV_ARGS(&d))) || !d)
            return false;
        bool same = d == dev;
        d->Release();
        if (same)
            queues.push_back(q);
        return same;
    }

    bool init(ID3D12CommandQueue *q)
    {
        if (FAILED(q->GetDevice(IID_PPV_ARGS(&dev))))
            return false;
        if (!make_root() || !make_resources())
            return false;
        ready = true;
        return true;
    }

    enum Outcome { drawn, busy, broken };

    Outcome render(IDXGISwapChain3 *sc, ID3D12CommandQueue *q, DXGI_FORMAT fmt, UINT width, UINT height,
                   const std::vector<Vertex> &verts)
    {
        Slot &s = slots[next];
        if (fence->GetCompletedValue() < s.done_at)
            return busy;
        ID3D12PipelineState *pso = pso_for(fmt);
        if (!pso)
            return broken;
        ID3D12Resource *buf = nullptr;
        if (FAILED(sc->GetBuffer(sc->GetCurrentBackBufferIndex(), IID_PPV_ARGS(&buf))) || !buf)
            return broken;
        size_t n = std::min<size_t>(verts.size(), MAX_VERTS);
        size_t base = size_t(next) * MAX_VERTS * sizeof(Vertex);
        memcpy(vmap + base, verts.data(), n * sizeof(Vertex));
        if (FAILED(s.alloc->Reset()) || FAILED(s.list->Reset(s.alloc, pso))) {
            buf->Release();
            return broken;
        }
        D3D12_CPU_DESCRIPTOR_HANDLE rtv = rtv_heap->GetCPUDescriptorHandleForHeapStart();
        rtv.ptr += SIZE_T(next) * rtv_inc;
        dev->CreateRenderTargetView(buf, nullptr, rtv);
        D3D12_RESOURCE_BARRIER b{};
        b.Transition.pResource = buf;
        b.Transition.StateBefore = D3D12_RESOURCE_STATE_PRESENT;
        b.Transition.StateAfter = D3D12_RESOURCE_STATE_RENDER_TARGET;
        b.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
        s.list->ResourceBarrier(1, &b);
        s.list->OMSetRenderTargets(1, &rtv, FALSE, nullptr);
        D3D12_VIEWPORT vp{0, 0, float(width), float(height), 0, 1};
        D3D12_RECT sr{0, 0, LONG(width), LONG(height)};
        s.list->RSSetViewports(1, &vp);
        s.list->RSSetScissorRects(1, &sr);
        s.list->SetGraphicsRootSignature(root);
        float inv[2] = {2.0f / float(width ? width : 1), 2.0f / float(height ? height : 1)};
        s.list->SetGraphicsRoot32BitConstants(0, 2, inv, 0);
        s.list->SetPipelineState(pso);
        s.list->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        D3D12_VERTEX_BUFFER_VIEW vbv{vbuf->GetGPUVirtualAddress() + base, UINT(n * sizeof(Vertex)), sizeof(Vertex)};
        s.list->IASetVertexBuffers(0, 1, &vbv);
        s.list->DrawInstanced(UINT(n), 1, 0, 0);
        std::swap(b.Transition.StateBefore, b.Transition.StateAfter);
        s.list->ResourceBarrier(1, &b);
        if (FAILED(s.list->Close())) {
            buf->Release();
            return broken;
        }
        ID3D12CommandList *lists[] = {s.list};
        q->ExecuteCommandLists(1, lists);
        q->Signal(fence, ++fence_val);
        s.done_at = fence_val;
        buf->Release();
        next = (next + 1) % SLOTS;
        return drawn;
    }

    void shutdown()
    {
        release(rtv_heap);
        release(vbuf);
        vmap = nullptr;
        for (auto &s : slots) {
            release(s.list);
            release(s.alloc);
        }
        release(fence);
        for (auto &p : psos)
            release(p.pso);
        psos.clear();
        release(root);
        release(vs);
        release(ps);
        release(dev);
        ready = false;
    }
};

Renderer renderer;
std::atomic<bool> armed, off, lost_once;
std::atomic<ID3D12Device *> live_dev;
std::atomic<ULONGLONG> last_draw_ms;
std::string drawn_by;

struct Stats {
    ULONGLONG since = 0;
    unsigned drawn = 0, busy = 0;
} stats;

using Present_t = HRESULT(STDMETHODCALLTYPE *)(IDXGISwapChain3 *, UINT, UINT);

safetyhook::InlineHook h_present, h_create, h_create_hwnd, h_create_core, h_create_comp;
thread_local int creating;

void device_lost(HRESULT present_hr)
{
    if (lost_once.exchange(true))
        return;
    off.store(true);
    ID3D12Device *dev = live_dev.load();
    HRESULT reason = dev ? dev->GetDeviceRemovedReason() : S_OK;
    ULONGLONG now = GetTickCount64(), last = last_draw_ms.load();
    if (!hud::guard::removal_after_draw(now, last)) {
        swofty::log("[overlay] the GPU was removed (present 0x%08lx, reason 0x%08lx) %s; the overlay stops drawing",
                    static_cast<unsigned long>(present_hr), static_cast<unsigned long>(reason),
                    last ? "more than 10 s after the overlay last drew" : "and the overlay had not drawn");
        return;
    }
    hud::guard::Removal r;
    r.when = now_text();
    r.present_hr = uint32_t(present_hr);
    r.reason = uint32_t(reason);
    r.after_draw_ms = now - last;
    r.swapchain_by = drawn_by;
    bool saved = runtime::write_file(runtime::under_swofty(hud::guard::REMOVED_FILE), hud::guard::removal_json(r));
    swofty::log("[overlay] the GPU was removed (present 0x%08lx, reason 0x%08lx) %.1f s after the overlay last drew; "
                "the overlay stops drawing%s",
                static_cast<unsigned long>(present_hr), static_cast<unsigned long>(reason), double(now - last) / 1000,
                saved ? " and stays off on later launches while swofty/overlay-gpu-removed.json exists"
                      : "; writing swofty/overlay-gpu-removed.json failed");
}

std::string fg_caller(void *caller)
{
    static void *last;
    static std::string last_fg;
    if (caller == last)
        return last_fg;
    std::string name = module_of(caller);
    last = caller;
    last_fg = hud::guard::fg_module(name) && !hud::guard::streamline_hook(name) ? name : std::string();
    return last_fg;
}

struct FgWatch {
    std::wstring path;
    FILETIME at{};
    ULONGLONG checked = 0;
    hud::guard::FgSetting now;
    bool tripped = false, told = false;
} fg_watch;

std::wstring hardware_settings_path()
{
    using GetFolder = HRESULT(WINAPI *)(HWND, int, HANDLE, DWORD, LPWSTR);
    HMODULE shell = GetModuleHandleW(L"shell32.dll");
    if (!shell)
        shell = LoadLibraryW(L"shell32.dll");
    auto get = shell ? reinterpret_cast<GetFolder>(GetProcAddress(shell, "SHGetFolderPathW")) : nullptr;
    wchar_t docs[MAX_PATH] = {};
    if (!get || FAILED(get(nullptr, 0x0005, nullptr, 0, docs)))
        return {};
    return std::wstring(docs) + L"\\My Games\\F1 25\\hardwaresettings\\hardware_settings_config.xml";
}

bool frame_gen_off(ULONGLONG now)
{
    FgWatch &w = fg_watch;
    if (w.tripped)
        return false;
    if (w.checked && now - w.checked < 2000)
        return !w.now.on();
    w.checked = now;
    if (w.path.empty())
        w.path = hardware_settings_path();
    WIN32_FILE_ATTRIBUTE_DATA fa{};
    if (w.path.empty() || !GetFileAttributesExW(w.path.c_str(), GetFileExInfoStandard, &fa)) {
        w.now = {};
    } else if (CompareFileTime(&fa.ftLastWriteTime, &w.at) || !w.told) {
        w.at = fa.ftLastWriteTime;
        w.now = hud::guard::fg_setting(runtime::read_file(w.path, 1 << 20));
    }
    bool off = !w.now.on();
    if (!w.told) {
        w.told = true;
        swofty::log("[overlay] the game's hardware settings (%s) have frame generation %s (frame_gen mode %d, "
                    "multi_frame_gen %d)",
                    runtime::narrow(w.path).c_str(), !w.now.known ? "unknown" : off ? "off" : "on", w.now.mode,
                    w.now.multi);
    } else if (!off) {
        swofty::log("[overlay] the game's hardware settings now have frame generation %s (frame_gen mode %d); the "
                    "overlay stops drawing for this session",
                    w.now.known ? "on" : "unknown", w.now.mode);
    }
    if (!off)
        w.tripped = true;
    return off;
}

void flush_stats(ULONGLONG now)
{
    if (!stats.since)
        stats.since = now;
    if (now - stats.since < STATS_MS)
        return;
    if (stats.drawn || stats.busy)
        swofty::log("[overlay] last %llu s: %u frame(s) drawn, %u skipped while the GPU still held the slot",
                    static_cast<unsigned long long>((now - stats.since) / 1000), stats.drawn, stats.busy);
    stats = {now, 0, 0};
}

struct Capture {
    bool on = false;
    std::wstring dir;
    ULONGLONG every_ms = 10000, last = 0;
    unsigned count = 0;
    bool pending = false;
    std::atomic<bool> busy{false};
    UINT64 done_at = 0;
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT fp{};
    UINT width = 0, height = 0;
    DXGI_FORMAT fmt = DXGI_FORMAT_UNKNOWN;
    ID3D12CommandAllocator *alloc = nullptr;
    ID3D12GraphicsCommandList *list = nullptr;
    ID3D12Resource *buf = nullptr;
    UINT64 size = 0;
    bool failed = false;
} cap;

std::string env(const char *name)
{
    char v[1024] = {};
    DWORD n = GetEnvironmentVariableA(name, v, sizeof v);
    return n && n < sizeof v ? std::string(v, n) : std::string();
}

void capture_setup()
{
    std::string dir = env("SWOFTY_FRAME_CAPTURE");
    if (dir.empty())
        return;
    std::string every = env("SWOFTY_FRAME_CAPTURE_EVERY");
    double s = every.empty() ? 10 : atof(every.c_str());
    cap.every_ms = ULONGLONG((s > 0.5 ? s : 0.5) * 1000);
    std::replace(dir.begin(), dir.end(), '/', '\\');
    while (!dir.empty() && dir.back() == '\\')
        dir.pop_back();
    cap.dir = runtime::wide(dir);
    for (size_t i = cap.dir.find(L'\\', 3); i != std::wstring::npos; i = cap.dir.find(L'\\', i + 1))
        CreateDirectoryW(cap.dir.substr(0, i).c_str(), nullptr);
    CreateDirectoryW(cap.dir.c_str(), nullptr);
    cap.on = true;
    swofty::log("[capture] frame capture on: the composed frame every %.1f s to %s", double(cap.every_ms) / 1000,
                dir.c_str());
}

DWORD WINAPI capture_write(void *)
{
    ULONGLONG t0 = GetTickCount64();
    uint8_t *mapped = nullptr;
    D3D12_RANGE all{0, SIZE_T(cap.size)};
    std::string why;
    if (FAILED(cap.buf->Map(0, &all, reinterpret_cast<void **>(&mapped))) || !mapped) {
        why = "mapping the readback buffer failed";
    } else {
        std::vector<uint8_t> rgb;
        bool ok = hud::png::to_rgb(hud::png::fmt_of(int(cap.fmt)), mapped + cap.fp.Offset, cap.fp.Footprint.RowPitch,
                                   cap.width, cap.height, rgb);
        D3D12_RANGE none{0, 0};
        cap.buf->Unmap(0, &none);
        if (!ok) {
            why = "the back buffer format is not one the capture converts";
        } else {
            std::string png = hud::png::encode_rgb(rgb.data(), cap.width, cap.height);
            wchar_t name[64];
            _snwprintf(name, 64, L"\\frame-%04u.png", cap.count);
            std::wstring path = cap.dir + name;
            if (!runtime::write_file(path, png))
                why = "writing " + runtime::narrow(path) + " failed";
            else
                swofty::log("[capture] wrote %s (%ux%u, format %d, %zu KB, %llu ms off the render thread)",
                            runtime::narrow(path).c_str(), cap.width, cap.height, int(cap.fmt), png.size() / 1024,
                            static_cast<unsigned long long>(GetTickCount64() - t0));
        }
    }
    if (!why.empty())
        swofty::log("[capture] frame %u: %s", cap.count, why.c_str());
    cap.busy.store(false);
    return 0;
}

void capture_step(IDXGISwapChain3 *sc, ID3D12CommandQueue *q, const DXGI_SWAP_CHAIN_DESC &desc, ULONGLONG now)
{
    if (cap.failed || cap.busy.load())
        return;
    if (cap.pending) {
        if (renderer.fence->GetCompletedValue() < cap.done_at)
            return;
        cap.pending = false;
        cap.busy.store(true);
        HANDLE t = CreateThread(nullptr, 0, capture_write, nullptr, 0, nullptr);
        if (t)
            CloseHandle(t);
        else
            cap.busy.store(false);
        return;
    }
    if (cap.last && now - cap.last < cap.every_ms)
        return;
    if (hud::png::fmt_of(int(desc.BufferDesc.Format)) == hud::png::Fmt::unknown) {
        cap.failed = true;
        swofty::log("[capture] back buffer format %d is not one the capture converts; capture off",
                    int(desc.BufferDesc.Format));
        return;
    }
    ID3D12Device *dev = renderer.dev;
    if (!cap.alloc && (FAILED(dev->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&cap.alloc))) ||
                       FAILED(dev->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, cap.alloc, nullptr,
                                                     IID_PPV_ARGS(&cap.list))) ||
                       FAILED(cap.list->Close()))) {
        cap.failed = true;
        swofty::log("[capture] making the capture command list failed; capture off");
        return;
    }
    ID3D12Resource *bb = nullptr;
    if (FAILED(sc->GetBuffer(sc->GetCurrentBackBufferIndex(), IID_PPV_ARGS(&bb))) || !bb)
        return;
    D3D12_RESOURCE_DESC rd = bb->GetDesc();
    UINT64 total = 0;
    dev->GetCopyableFootprints(&rd, 0, 1, 0, &cap.fp, nullptr, nullptr, &total);
    if (!cap.buf || cap.size < total) {
        release(cap.buf);
        D3D12_HEAP_PROPERTIES hp{};
        hp.Type = D3D12_HEAP_TYPE_READBACK;
        D3D12_RESOURCE_DESC bd{};
        bd.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
        bd.Width = total;
        bd.Height = 1;
        bd.DepthOrArraySize = 1;
        bd.MipLevels = 1;
        bd.Format = DXGI_FORMAT_UNKNOWN;
        bd.SampleDesc.Count = 1;
        bd.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
        if (FAILED(dev->CreateCommittedResource(&hp, D3D12_HEAP_FLAG_NONE, &bd, D3D12_RESOURCE_STATE_COPY_DEST, nullptr,
                                                IID_PPV_ARGS(&cap.buf)))) {
            bb->Release();
            cap.failed = true;
            swofty::log("[capture] making a %llu-byte readback buffer failed; capture off",
                        static_cast<unsigned long long>(total));
            return;
        }
        cap.size = total;
    }
    if (FAILED(cap.alloc->Reset()) || FAILED(cap.list->Reset(cap.alloc, nullptr))) {
        bb->Release();
        cap.failed = true;
        return;
    }
    D3D12_RESOURCE_BARRIER b{};
    b.Transition.pResource = bb;
    b.Transition.StateBefore = D3D12_RESOURCE_STATE_PRESENT;
    b.Transition.StateAfter = D3D12_RESOURCE_STATE_COPY_SOURCE;
    b.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
    cap.list->ResourceBarrier(1, &b);
    D3D12_TEXTURE_COPY_LOCATION dst{}, src{};
    dst.pResource = cap.buf;
    dst.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    dst.PlacedFootprint = cap.fp;
    src.pResource = bb;
    src.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    src.SubresourceIndex = 0;
    cap.list->CopyTextureRegion(&dst, 0, 0, 0, &src, nullptr);
    std::swap(b.Transition.StateBefore, b.Transition.StateAfter);
    cap.list->ResourceBarrier(1, &b);
    if (FAILED(cap.list->Close())) {
        bb->Release();
        cap.failed = true;
        return;
    }
    ID3D12CommandList *lists[] = {cap.list};
    q->ExecuteCommandLists(1, lists);
    q->Signal(renderer.fence, ++renderer.fence_val);
    bb->Release();
    cap.done_at = renderer.fence_val;
    cap.pending = true;
    cap.last = now;
    cap.count++;
    cap.width = desc.BufferDesc.Width;
    cap.height = desc.BufferDesc.Height;
    cap.fmt = desc.BufferDesc.Format;
}

void paint_frame(IDXGISwapChain3 *sc, ID3D12CommandQueue *q, const std::vector<hud::Paintsub> &subs,
                 const DXGI_SWAP_CHAIN_DESC &desc, ULONGLONG now);

void draw_frame(IDXGISwapChain3 *sc, void *caller)
{
    if (off.load() || renderer.failed)
        return;
    std::vector<hud::Paintsub> subs;
    if (implemented) {
        std::lock_guard<std::mutex> g(reg_lock);
        subs = reg.subs();
    }
    if (subs.empty() && !cap.on)
        return;
    std::string pfg = fg_caller(caller);
    Chain c;
    bool fresh = false;
    {
        std::lock_guard<std::mutex> g(chains_lock);
        auto it = std::find_if(chains.begin(), chains.end(), [&](const Chain &x) { return x.sc == sc; });
        if (it == chains.end()) {
            if (std::find(unknown_told.begin(), unknown_told.end(), sc) == unknown_told.end()) {
                unknown_told.push_back(sc);
                swofty::log("[overlay] swapchain %p was created before the overlay hooked swapchain creation, so its "
                            "present queue is unknown; the overlay does not draw on it",
                            static_cast<void *>(sc));
            }
            return;
        }
        bool fg_was = !it->fg.empty();
        if (!fg_was && !pfg.empty())
            it->fg = pfg + " (calls Present)";
        fresh = !it->told || (!fg_was && !it->fg.empty());
        it->told = true;
        c = *it;
    }
    if (!c.fg.empty()) {
        if (fresh)
            swofty::log("[overlay] frame generation (%s) owns swapchain %p; the overlay does not draw on it",
                        c.fg.c_str(), static_cast<void *>(sc));
        return;
    }
    if (c.streamline && !frame_gen_off(GetTickCount64())) {
        if (!fg_watch.told || fresh)
            swofty::log("[overlay] swapchain %p was created through Streamline's frame generation hook "
                        "(sl.dlss_g.dll) and frame generation is not off in the game's settings; the overlay does not "
                        "draw on it",
                        static_cast<void *>(sc));
        return;
    }
    if (!renderer.ready) {
        if (!renderer.init(c.queue)) {
            renderer.failed = true;
            renderer.shutdown();
            swofty::log("[overlay] renderer init failed; the overlay stays armed but draws nothing");
            return;
        }
        live_dev.store(renderer.dev);
    }
    if (!renderer.same_device(c.queue)) {
        renderer.failed = true;
        swofty::log("[overlay] swapchain %p presents on queue %p of another device than the overlay renderer's; the "
                    "overlay stops drawing",
                    static_cast<void *>(sc), static_cast<void *>(c.queue));
        return;
    }
    if (renderer.dev->GetDeviceRemovedReason() != S_OK) {
        device_lost(S_OK);
        return;
    }
    DXGI_SWAP_CHAIN_DESC desc{};
    if (FAILED(sc->GetDesc(&desc)))
        return;
    if (!(desc.BufferUsage & DXGI_USAGE_RENDER_TARGET_OUTPUT)) {
        if (fresh)
            swofty::log("[overlay] swapchain %p back buffers are not render targets (usage 0x%x); the overlay does not "
                        "draw on it",
                        static_cast<void *>(sc), unsigned(desc.BufferUsage));
        return;
    }
    if (fresh) {
        drawn_by = c.by;
        swofty::log("[overlay] drawing on swapchain %p with the %s queue %p it was created with (%ux%u, format %d, %u "
                    "buffer(s), created by %s)",
                    static_cast<void *>(sc), "direct", static_cast<void *>(c.queue), desc.BufferDesc.Width,
                    desc.BufferDesc.Height, int(desc.BufferDesc.Format), desc.BufferCount, c.by.c_str());
    }
    ULONGLONG now = GetTickCount64();
    if (!subs.empty())
        paint_frame(sc, c.queue, subs, desc, now);
    if (cap.on)
        capture_step(sc, c.queue, desc, now);
}

void paint_frame(IDXGISwapChain3 *sc, ID3D12CommandQueue *q, const std::vector<hud::Paintsub> &subs,
                 const DXGI_SWAP_CHAIN_DESC &desc, ULONGLONG now)
{
    hud::Frame frame;
    hud::build(subs, frame, float(desc.BufferDesc.Width), float(desc.BufferDesc.Height));
    std::vector<Vertex> verts = tessellate(frame);
    flush_stats(now);
    if (verts.empty())
        return;
    switch (renderer.render(sc, q, desc.BufferDesc.Format, desc.BufferDesc.Width, desc.BufferDesc.Height, verts)) {
    case Renderer::drawn:
        if (!stats.drawn && !last_draw_ms.load())
            swofty::log("[overlay] first overlay frame submitted: %zu vertices", verts.size());
        last_draw_ms.store(now);
        stats.drawn++;
        break;
    case Renderer::busy:
        stats.busy++;
        break;
    case Renderer::broken:
        renderer.failed = true;
        swofty::log("[overlay] recording the overlay for swapchain %p failed (format %d); the overlay stops drawing",
                    static_cast<void *>(sc), int(desc.BufferDesc.Format));
        break;
    }
}

std::atomic<bool> drawing;

HRESULT STDMETHODCALLTYPE present_hook(IDXGISwapChain3 *sc, UINT sync, UINT flags)
{
    if (!(flags & DXGI_PRESENT_TEST) && !drawing.exchange(true)) {
        draw_frame(sc, __builtin_return_address(0));
        drawing.store(false);
    }
    HRESULT hr = h_present.call<HRESULT>(sc, sync, flags);
    if ((hr == DXGI_ERROR_DEVICE_REMOVED || hr == DXGI_ERROR_DEVICE_RESET) && armed.load())
        device_lost(hr);
    return hr;
}

void captured(const char *how, IUnknown *device, void *sc, UINT w, UINT h, DXGI_FORMAT fmt, UINT buffers, HMODULE impl)
{
    void *frames[48];
    USHORT n = RtlCaptureStackBackTrace(0, 48, frames, nullptr);
    std::vector<std::string> mods;
    for (USHORT i = 0; i < n; i++)
        mods.push_back(module_of(frames[i]));
    char impl_path[MAX_PATH] = {};
    DWORD impl_n = impl ? GetModuleFileNameA(impl, impl_path, MAX_PATH) : 0;
    std::string impl_name(hud::guard::base_name(std::string_view(impl_path, impl_n)));
    std::string by = hud::guard::chain(mods, self_module(), impl_name);
    ID3D12CommandQueue *q = nullptr;
    if (!device || FAILED(device->QueryInterface(IID_PPV_ARGS(&q))) || !q) {
        swofty::log("[overlay] swapchain %p (%s, by %s) was not created on a D3D12 command queue; the overlay ignores it",
                    sc, how, by.c_str());
        return;
    }
    D3D12_COMMAND_QUEUE_DESC qd = q->GetDesc();
    q->Release();
    Chain c;
    c.sc = sc;
    c.queue = q;
    c.how = how;
    c.by = by;
    c.fg = hud::guard::fg_in(mods);
    c.streamline = hud::guard::streamline_in(mods);
    if (qd.Type != D3D12_COMMAND_LIST_TYPE_DIRECT && c.fg.empty())
        c.fg = "a non-direct present queue";
    {
        std::lock_guard<std::mutex> g(chains_lock);
        auto it = std::find_if(chains.begin(), chains.end(), [&](const Chain &x) { return x.sc == sc; });
        if (it != chains.end())
            *it = c;
        else
            chains.push_back(c);
        unknown_told.erase(std::remove(unknown_told.begin(), unknown_told.end(), sc), unknown_told.end());
    }
    swofty::log("[overlay] swapchain %p created through %s on queue %p (type %d, %ux%u, format %d, %u buffer(s)); "
                "created by %s%s%s",
                sc, how, static_cast<void *>(q), int(qd.Type), w, h, int(fmt), buffers, by.c_str(),
                c.fg.empty() ? (c.streamline ? "; through Streamline's frame generation hook" : "")
                             : "; frame generation: ",
                c.fg.c_str());
}

HMODULE impl_create, impl_hwnd, impl_core, impl_comp;

HRESULT STDMETHODCALLTYPE create_hook(IDXGIFactory *f, IUnknown *dev, DXGI_SWAP_CHAIN_DESC *d, IDXGISwapChain **out)
{
    creating++;
    HRESULT hr = h_create.call<HRESULT>(f, dev, d, out);
    if (--creating == 0 && SUCCEEDED(hr) && out && *out && d)
        captured("IDXGIFactory::CreateSwapChain", dev, *out, d->BufferDesc.Width, d->BufferDesc.Height,
                 d->BufferDesc.Format, d->BufferCount, impl_create);
    return hr;
}

HRESULT STDMETHODCALLTYPE create_hwnd_hook(IDXGIFactory2 *f, IUnknown *dev, HWND wnd, const DXGI_SWAP_CHAIN_DESC1 *d,
                                           const DXGI_SWAP_CHAIN_FULLSCREEN_DESC *fs, IDXGIOutput *o,
                                           IDXGISwapChain1 **out)
{
    creating++;
    HRESULT hr = h_create_hwnd.call<HRESULT>(f, dev, wnd, d, fs, o, out);
    if (--creating == 0 && SUCCEEDED(hr) && out && *out && d)
        captured("CreateSwapChainForHwnd", dev, *out, d->Width, d->Height, d->Format, d->BufferCount, impl_hwnd);
    return hr;
}

HRESULT STDMETHODCALLTYPE create_core_hook(IDXGIFactory2 *f, IUnknown *dev, IUnknown *wnd,
                                           const DXGI_SWAP_CHAIN_DESC1 *d, IDXGIOutput *o, IDXGISwapChain1 **out)
{
    creating++;
    HRESULT hr = h_create_core.call<HRESULT>(f, dev, wnd, d, o, out);
    if (--creating == 0 && SUCCEEDED(hr) && out && *out && d)
        captured("CreateSwapChainForCoreWindow", dev, *out, d->Width, d->Height, d->Format, d->BufferCount, impl_core);
    return hr;
}

HRESULT STDMETHODCALLTYPE create_comp_hook(IDXGIFactory2 *f, IUnknown *dev, const DXGI_SWAP_CHAIN_DESC1 *d,
                                           IDXGIOutput *o, IDXGISwapChain1 **out)
{
    creating++;
    HRESULT hr = h_create_comp.call<HRESULT>(f, dev, d, o, out);
    if (--creating == 0 && SUCCEEDED(hr) && out && *out && d)
        captured("CreateSwapChainForComposition", dev, *out, d->Width, d->Height, d->Format, d->BufferCount,
                 impl_comp);
    return hr;
}

bool install(safetyhook::InlineHook &h, void *target, void *detour, const char *what)
{
    auto made = safetyhook::InlineHook::create(target, detour);
    if (!made) {
        swofty::log("[overlay] hooking %s at %p failed", what, target);
        return false;
    }
    h = std::move(*made);
    return true;
}

bool resolve_d3d12()
{
    HMODULE d3d12 = LoadLibraryA("d3d12.dll");
    HMODULE dxgi = LoadLibraryA("dxgi.dll");
    d3d12_create_device = d3d12 ? reinterpret_cast<CreateDeviceFn>(GetProcAddress(d3d12, "D3D12CreateDevice")) : nullptr;
    d3d12_serialize_root =
        d3d12 ? reinterpret_cast<SerializeRootFn>(GetProcAddress(d3d12, "D3D12SerializeRootSignature")) : nullptr;
    dxgi_create_factory = dxgi ? reinterpret_cast<CreateFactoryFn>(GetProcAddress(dxgi, "CreateDXGIFactory1")) : nullptr;
    if (d3d12_create_device && d3d12_serialize_root && dxgi_create_factory)
        return true;
    swofty::log("[overlay] d3d12.dll / dxgi.dll entry points are unavailable; the present hook is not armed");
    return false;
}

bool hook_factory(void **vt)
{
    struct Want {
        int slot;
        safetyhook::InlineHook *h;
        void *detour;
        HMODULE *impl;
        const char *what;
        bool required;
    } wants[] = {
        {15, &h_create_hwnd, reinterpret_cast<void *>(&create_hwnd_hook), &impl_hwnd, "CreateSwapChainForHwnd", true},
        {10, &h_create, reinterpret_cast<void *>(&create_hook), &impl_create, "CreateSwapChain", false},
        {16, &h_create_core, reinterpret_cast<void *>(&create_core_hook), &impl_core, "CreateSwapChainForCoreWindow",
         false},
        {24, &h_create_comp, reinterpret_cast<void *>(&create_comp_hook), &impl_comp, "CreateSwapChainForComposition",
         false},
    };
    std::vector<void *> done;
    std::string armed_list;
    for (auto &w : wants) {
        void *target = vt[w.slot];
        if (std::find(done.begin(), done.end(), target) != done.end()) {
            swofty::log("[overlay] IDXGIFactory %s shares its code with another hooked slot; not hooked twice", w.what);
            continue;
        }
        module_of(target, w.impl);
        if (!install(*w.h, target, w.detour, w.what)) {
            if (w.required)
                return false;
            continue;
        }
        done.push_back(target);
        char b[64];
        snprintf(b, sizeof b, "%s%s (vt+0x%x)", armed_list.empty() ? "" : ", ", w.what, unsigned(w.slot * 8));
        armed_list += b;
    }
    swofty::log("[overlay] swapchain creation hooked on the DXGI factory (%s, in %s)", armed_list.c_str(),
                module_of(vt[15]).c_str());
    return true;
}

bool arm()
{
    if (!resolve_d3d12())
        return false;
    WNDCLASSEXW wc{};
    wc.cbSize = sizeof wc;
    wc.lpfnWndProc = DefWindowProcW;
    wc.hInstance = GetModuleHandleW(nullptr);
    wc.lpszClassName = L"SwoftyOverlayProbe";
    RegisterClassExW(&wc);
    HWND wnd = CreateWindowExW(0, wc.lpszClassName, L"", WS_OVERLAPPEDWINDOW, 0, 0, 64, 64, nullptr, nullptr,
                               wc.hInstance, nullptr);
    if (!wnd) {
        swofty::log("[overlay] could not make a probe window to read the swapchain vtable");
        UnregisterClassW(wc.lpszClassName, wc.hInstance);
        return false;
    }
    ID3D12Device *dev = nullptr;
    ID3D12CommandQueue *q = nullptr;
    IDXGIFactory4 *factory = nullptr;
    IDXGISwapChain1 *swap1 = nullptr;
    IDXGISwapChain3 *swap = nullptr;
    bool ok = false;
    do {
        if (FAILED(d3d12_create_device(nullptr, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&dev))))
            break;
        D3D12_COMMAND_QUEUE_DESC qd{};
        qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
        if (FAILED(dev->CreateCommandQueue(&qd, IID_PPV_ARGS(&q))))
            break;
        if (FAILED(dxgi_create_factory(IID_PPV_ARGS(&factory))))
            break;
        DXGI_SWAP_CHAIN_DESC1 sd{};
        sd.BufferCount = 2;
        sd.Width = 64;
        sd.Height = 64;
        sd.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
        sd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
        sd.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
        sd.SampleDesc.Count = 1;
        if (FAILED(factory->CreateSwapChainForHwnd(q, wnd, &sd, nullptr, nullptr, &swap1)))
            break;
        if (FAILED(swap1->QueryInterface(IID_PPV_ARGS(&swap))))
            break;
        void **sc_vt = *reinterpret_cast<void ***>(swap);
        void **f_vt = *reinterpret_cast<void ***>(factory);
        ok = hook_factory(f_vt) &&
             install(h_present, sc_vt[8], reinterpret_cast<void *>(&present_hook), "IDXGISwapChain::Present");
        if (ok)
            swofty::log("[overlay] present hook armed: Present (vt+0x40) on the D3D12 swapchain (in %s); the overlay "
                        "draws only on swapchains created after this point, on their creation queue",
                        module_of(sc_vt[8]).c_str());
    } while (false);
    release(swap);
    release(swap1);
    release(factory);
    release(q);
    release(dev);
    DestroyWindow(wnd);
    UnregisterClassW(wc.lpszClassName, wc.hInstance);
    return ok;
}

bool blocked_by_removal()
{
    std::wstring path = runtime::under_swofty(hud::guard::REMOVED_FILE);
    uint64_t size = 0;
    if (!runtime::file_size(path, &size))
        return false;
    hud::guard::Removal r;
    if (hud::guard::parse_removal(runtime::read_file(path, 1 << 16), r))
        swofty::log("[overlay] off: swofty/%s records the GPU removed %.1f s after the overlay drew (%s, present %s, "
                    "reason %s, swapchain by %s); the overlay stays off while that file exists",
                    hud::guard::REMOVED_FILE, double(r.after_draw_ms) / 1000, r.when.c_str(),
                    hud::guard::hex32(r.present_hr).c_str(), hud::guard::hex32(r.reason).c_str(),
                    r.swapchain_by.c_str());
    else
        swofty::log("[overlay] off: swofty/%s exists (unreadable); the overlay stays off while that file exists",
                    hud::guard::REMOVED_FILE);
    return true;
}

// ----- SwoftyOverlay table ----------------------------------------------------------------------

int ov_available()
{
    return implemented && armed.load() ? 1 : 0;
}

int ov_on_paint(SwoftyPaintFn fn, void *user, uint32_t *token)
{
    Caller c = caller(__builtin_return_address(0));
    if (!implemented || !gate_on())
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!fn)
        return SWOFTY_ASSET_INVALID;
    uint32_t t;
    {
        std::lock_guard<std::mutex> g(reg_lock);
        t = reg.add(fn, user, c.id.empty() ? c.dll : c.id);
    }
    if (token)
        *token = t;
    swofty::log("[overlay] api: %s on_paint -> ok (token %u)", c.id.empty() ? c.dll.c_str() : c.id.c_str(), t);
    return SWOFTY_ASSET_OK;
}

int ov_remove(uint32_t token)
{
    if (!implemented || !gate_on())
        return SWOFTY_ASSET_UNAVAILABLE;
    std::lock_guard<std::mutex> g(reg_lock);
    return reg.remove(token) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_INVALID;
}

void ov_size(const SwoftyPaint *paint, float *width, float *height)
{
    auto *f = reinterpret_cast<const hud::Frame *>(paint);
    if (width)
        *width = f ? f->width : 0;
    if (height)
        *height = f ? f->height : 0;
}

void ov_rect(SwoftyPaint *paint, float x, float y, float w, float h, uint32_t rgba)
{
    if (paint)
        hud::rect(*reinterpret_cast<hud::Frame *>(paint), x, y, w, h, rgba);
}

void ov_rect_outline(SwoftyPaint *paint, float x, float y, float w, float h, float thickness, uint32_t rgba)
{
    if (paint)
        hud::rect_outline(*reinterpret_cast<hud::Frame *>(paint), x, y, w, h, thickness, rgba);
}

void ov_line(SwoftyPaint *paint, float x0, float y0, float x1, float y1, float thickness, uint32_t rgba)
{
    if (paint)
        hud::line(*reinterpret_cast<hud::Frame *>(paint), x0, y0, x1, y1, thickness, rgba);
}

void ov_text(SwoftyPaint *paint, float x, float y, float size, uint32_t rgba, const char *utf8)
{
    if (paint && utf8)
        hud::text(*reinterpret_cast<hud::Frame *>(paint), x, y, size, rgba, utf8);
}

void ov_measure(const SwoftyPaint *, float size, const char *utf8, float *width, float *height)
{
    hud::measure(size, utf8 ? utf8 : "", width, height);
}

int ov_load_image(const char *id, const char *file)
{
    Caller c = caller(__builtin_return_address(0));
    if (!implemented || !gate_on())
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!id || !*id)
        return SWOFTY_ASSET_INVALID;
    std::string why, rel = addon_image(c, file, why);
    if (rel.empty()) {
        swofty::log("[overlay] api: %s load_image %s -> invalid: %s", c.id.empty() ? c.dll.c_str() : c.id.c_str(), id,
                    why.c_str());
        return SWOFTY_ASSET_INVALID;
    }
    std::lock_guard<std::mutex> g(reg_lock);
    int slot = reg.intern_image(c.id + "\n" + id);
    swofty::log("[overlay] api: %s load_image %s (swofty/%s) -> ok (slot %d, not painted in this build)",
                c.id.c_str(), id, rel.c_str(), slot);
    return SWOFTY_ASSET_OK;
}

int ov_image(SwoftyPaint *paint, const char *id, float x, float y, float w, float h, uint32_t tint)
{
    if (!implemented || !gate_on())
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!id || !*id)
        return SWOFTY_ASSET_INVALID;
    Caller c = caller(__builtin_return_address(0));
    int slot;
    {
        std::lock_guard<std::mutex> g(reg_lock);
        slot = reg.find_image(c.id + "\n" + id);
    }
    if (slot < 0)
        return SWOFTY_ASSET_INVALID;
    if (paint)
        hud::image(*reinterpret_cast<hud::Frame *>(paint), slot, x, y, w, h, tint);
    return SWOFTY_ASSET_OK;
}

const SwoftyOverlay table = {ov_available,  ov_on_paint, ov_remove,  ov_size,       ov_rect,       ov_rect_outline,
                             ov_line,       ov_text,     ov_measure, ov_load_image, ov_image};

void setup()
{
    capture_setup();
    bool on = gate_on();
    if (!on && !cap.on) {
        swofty::log("[overlay] the overlay feature is experimental and off; the HUD present hook is not armed");
        return;
    }
    if (on && blocked_by_removal()) {
        if (!cap.on)
            return;
        on = false;
    }
    if (!on)
        swofty::log("[overlay] the overlay is off; the present hook arms for SWOFTY_FRAME_CAPTURE only");
    implemented = on;
    if (arm())
        armed.store(true);
    else
        swofty::log("[overlay] the present hook did not arm; on_paint callbacks will not be drawn");
}

}

namespace runtime {

const SwoftyOverlay *overlay_table()
{
    return &table;
}

void hud_setup()
{
    setup();
}

void hud_fatal()
{
    ID3D12Device *dev = live_dev.load();
    if (dev && !lost_once.load() && dev->GetDeviceRemovedReason() != S_OK)
        device_lost(S_OK);
}

}
