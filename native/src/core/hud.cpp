#include "runtime.hpp"

#include "hud_drawlist.hpp"

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <d3d12.h>
#include <d3dcompiler.h>
#include <dxgi1_6.h>

#include <atomic>
#include <climits>
#include <cmath>
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
// The renderer draws the frame's filled rectangles, rectangle outlines and lines as solid-colour
// triangles over the game's back buffer. Text and image commands are recorded by the draw list but
// this build does not paint them (see docs/design/native-libraries.md). Every step is guarded: the
// first failure sets failed and the overlay goes quiet for the rest of the process, so a drawing
// fault never takes the game down with it.

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

constexpr UINT MAX_VERTS = 1u << 16;

struct Renderer {
    ID3D12Device *dev = nullptr;
    ID3D12RootSignature *root = nullptr;
    ID3D12PipelineState *pso = nullptr;
    ID3D12DescriptorHeap *rtv_heap = nullptr;
    UINT rtv_inc = 0;
    ID3D12CommandAllocator *alloc = nullptr;
    ID3D12GraphicsCommandList *list = nullptr;
    ID3D12Resource *vbuf = nullptr;
    Vertex *vmap = nullptr;
    ID3D12Fence *fence = nullptr;
    UINT64 fence_val = 0;
    HANDLE fence_event = nullptr;
    std::vector<ID3D12Resource *> targets;
    DXGI_FORMAT fmt = DXGI_FORMAT_UNKNOWN;
    UINT width = 0, height = 0, buffers = 0;
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

    bool make_pipeline()
    {
        HMODULE compiler = LoadLibraryA("d3dcompiler_47.dll");
        auto compile_fn = compiler ? reinterpret_cast<pD3DCompile>(GetProcAddress(compiler, "D3DCompile")) : nullptr;
        if (!compile_fn) {
            swofty::log("[overlay] d3dcompiler_47.dll!D3DCompile is unavailable; overlay rendering off");
            return false;
        }
        ID3DBlob *vs = nullptr, *ps = nullptr;
        if (!compile(compile_fn, "VS", "vs_5_0", &vs) || !compile(compile_fn, "PS", "ps_5_0", &ps)) {
            release(vs);
            release(ps);
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
        if (FAILED(hr)) {
            release(vs);
            release(ps);
            return false;
        }
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
        hr = dev->CreateGraphicsPipelineState(&pd, IID_PPV_ARGS(&pso));
        release(vs);
        release(ps);
        return SUCCEEDED(hr);
    }

    bool make_resources()
    {
        if (FAILED(dev->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&alloc))))
            return false;
        if (FAILED(dev->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, alloc, nullptr, IID_PPV_ARGS(&list))))
            return false;
        list->Close();
        if (FAILED(dev->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&fence))))
            return false;
        fence_event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
        if (!fence_event)
            return false;
        D3D12_HEAP_PROPERTIES up{};
        up.Type = D3D12_HEAP_TYPE_UPLOAD;
        D3D12_RESOURCE_DESC rd{};
        rd.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
        rd.Width = sizeof(Vertex) * MAX_VERTS;
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

    void release_targets()
    {
        for (auto *t : targets)
            release(t);
        targets.clear();
    }

    bool acquire(IDXGISwapChain3 *sc, const DXGI_SWAP_CHAIN_DESC &desc)
    {
        release_targets();
        release(rtv_heap);
        D3D12_DESCRIPTOR_HEAP_DESC hd{};
        hd.Type = D3D12_DESCRIPTOR_HEAP_TYPE_RTV;
        hd.NumDescriptors = desc.BufferCount;
        if (FAILED(dev->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&rtv_heap))))
            return false;
        rtv_inc = dev->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
        D3D12_CPU_DESCRIPTOR_HANDLE h = rtv_heap->GetCPUDescriptorHandleForHeapStart();
        for (UINT i = 0; i < desc.BufferCount; i++) {
            ID3D12Resource *buf = nullptr;
            if (FAILED(sc->GetBuffer(i, IID_PPV_ARGS(&buf))))
                return false;
            dev->CreateRenderTargetView(buf, nullptr, h);
            targets.push_back(buf);
            h.ptr += rtv_inc;
        }
        buffers = desc.BufferCount;
        width = desc.BufferDesc.Width;
        height = desc.BufferDesc.Height;
        fmt = desc.BufferDesc.Format;
        return true;
    }

    bool init(IDXGISwapChain3 *sc)
    {
        DXGI_SWAP_CHAIN_DESC desc{};
        if (FAILED(sc->GetDesc(&desc)))
            return false;
        if (FAILED(sc->GetDevice(IID_PPV_ARGS(&dev))))
            return false;
        fmt = desc.BufferDesc.Format;
        if (!make_pipeline() || !make_resources() || !acquire(sc, desc))
            return false;
        ready = true;
        swofty::log("[overlay] renderer up: %ux%u, %u buffer(s), format %d", width, height, buffers, int(fmt));
        return true;
    }

    void wait()
    {
        UINT64 v = ++fence_val;
        queue()->Signal(fence, v);
        if (fence->GetCompletedValue() < v) {
            fence->SetEventOnCompletion(v, fence_event);
            WaitForSingleObject(fence_event, 1000);
        }
    }

    ID3D12CommandQueue *queue();

    void render(IDXGISwapChain3 *sc, const hud::Frame &f)
    {
        std::vector<Vertex> verts = tessellate(f);
        if (verts.empty())
            return;
        if (verts.size() > MAX_VERTS)
            verts.resize(MAX_VERTS);
        memcpy(vmap, verts.data(), verts.size() * sizeof(Vertex));
        UINT idx = sc->GetCurrentBackBufferIndex();
        if (idx >= targets.size())
            return;
        alloc->Reset();
        list->Reset(alloc, pso);
        D3D12_RESOURCE_BARRIER b{};
        b.Transition.pResource = targets[idx];
        b.Transition.StateBefore = D3D12_RESOURCE_STATE_PRESENT;
        b.Transition.StateAfter = D3D12_RESOURCE_STATE_RENDER_TARGET;
        b.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
        list->ResourceBarrier(1, &b);
        D3D12_CPU_DESCRIPTOR_HANDLE rtv = rtv_heap->GetCPUDescriptorHandleForHeapStart();
        rtv.ptr += SIZE_T(idx) * rtv_inc;
        list->OMSetRenderTargets(1, &rtv, FALSE, nullptr);
        D3D12_VIEWPORT vp{0, 0, float(width), float(height), 0, 1};
        D3D12_RECT sr{0, 0, LONG(width), LONG(height)};
        list->RSSetViewports(1, &vp);
        list->RSSetScissorRects(1, &sr);
        list->SetGraphicsRootSignature(root);
        float inv[2] = {2.0f / float(width ? width : 1), 2.0f / float(height ? height : 1)};
        list->SetGraphicsRoot32BitConstants(0, 2, inv, 0);
        list->SetPipelineState(pso);
        list->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        D3D12_VERTEX_BUFFER_VIEW vbv{vbuf->GetGPUVirtualAddress(), UINT(verts.size() * sizeof(Vertex)), sizeof(Vertex)};
        list->IASetVertexBuffers(0, 1, &vbv);
        list->DrawInstanced(UINT(verts.size()), 1, 0, 0);
        std::swap(b.Transition.StateBefore, b.Transition.StateAfter);
        list->ResourceBarrier(1, &b);
        list->Close();
        ID3D12CommandList *lists[] = {list};
        queue()->ExecuteCommandLists(1, lists);
        wait();
    }

    void drop_targets_for_resize()
    {
        if (!ready)
            return;
        wait();
        release_targets();
        buffers = 0;
    }

    void shutdown()
    {
        release_targets();
        release(rtv_heap);
        release(vbuf);
        vmap = nullptr;
        release(list);
        release(alloc);
        release(fence);
        if (fence_event)
            CloseHandle(fence_event), fence_event = nullptr;
        release(pso);
        release(root);
        release(dev);
        ready = false;
    }
};

Renderer renderer;
std::atomic<ID3D12CommandQueue *> command_queue;
std::atomic<bool> armed;

ID3D12CommandQueue *Renderer::queue()
{
    return command_queue.load();
}

using Present_t = HRESULT(STDMETHODCALLTYPE *)(IDXGISwapChain3 *, UINT, UINT);
using Resize_t = HRESULT(STDMETHODCALLTYPE *)(IDXGISwapChain3 *, UINT, UINT, UINT, DXGI_FORMAT, UINT);
using Exec_t = void(STDMETHODCALLTYPE *)(ID3D12CommandQueue *, UINT, ID3D12CommandList *const *);

safetyhook::InlineHook h_present, h_resize, h_exec;

void draw_frame(IDXGISwapChain3 *sc)
{
    if (renderer.failed || !command_queue.load())
        return;
    if (!renderer.ready && !renderer.init(sc)) {
        renderer.failed = true;
        renderer.shutdown();
        swofty::log("[overlay] renderer init failed; the overlay stays armed but draws nothing");
        return;
    }
    DXGI_SWAP_CHAIN_DESC desc{};
    if (SUCCEEDED(sc->GetDesc(&desc)) &&
        (renderer.buffers == 0 || desc.BufferDesc.Width != renderer.width || desc.BufferDesc.Height != renderer.height ||
         desc.BufferCount != renderer.buffers) &&
        !renderer.acquire(sc, desc)) {
        renderer.failed = true;
        renderer.shutdown();
        swofty::log("[overlay] renderer could not re-acquire back buffers after a resize; overlay off");
        return;
    }
    std::vector<hud::Paintsub> subs;
    {
        std::lock_guard<std::mutex> g(reg_lock);
        if (reg.count() == 0)
            return;
        subs = reg.subs();
    }
    hud::Frame frame;
    hud::build(subs, frame, float(renderer.width), float(renderer.height));
    renderer.render(sc, frame);
}

HRESULT STDMETHODCALLTYPE present_hook(IDXGISwapChain3 *sc, UINT sync, UINT flags)
{
    if (!(flags & DXGI_PRESENT_TEST))
        draw_frame(sc);
    return h_present.call<HRESULT>(sc, sync, flags);
}

HRESULT STDMETHODCALLTYPE resize_hook(IDXGISwapChain3 *sc, UINT count, UINT w, UINT h, DXGI_FORMAT fmt, UINT flags)
{
    renderer.drop_targets_for_resize();
    return h_resize.call<HRESULT>(sc, count, w, h, fmt, flags);
}

void STDMETHODCALLTYPE exec_hook(ID3D12CommandQueue *q, UINT n, ID3D12CommandList *const *lists)
{
    if (!command_queue.load() && q) {
        D3D12_COMMAND_QUEUE_DESC d = q->GetDesc();
        if (d.Type == D3D12_COMMAND_LIST_TYPE_DIRECT) {
            command_queue.store(q);
            swofty::log("[overlay] captured the game's direct command queue %p", static_cast<void *>(q));
        }
    }
    h_exec.call<void>(q, n, lists);
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
        void **q_vt = *reinterpret_cast<void ***>(q);
        ok = install(h_present, sc_vt[8], reinterpret_cast<void *>(&present_hook), "IDXGISwapChain::Present") &&
             install(h_resize, sc_vt[13], reinterpret_cast<void *>(&resize_hook), "IDXGISwapChain::ResizeBuffers") &&
             install(h_exec, q_vt[10], reinterpret_cast<void *>(&exec_hook), "ID3D12CommandQueue::ExecuteCommandLists");
        if (ok)
            swofty::log("[overlay] present hook armed: Present (vt+0x40), ResizeBuffers (vt+0x68), "
                        "ExecuteCommandLists (vt+0x50) on the game's D3D12 swapchain");
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
    if (!gate_on()) {
        swofty::log("[overlay] the overlay feature is experimental and off; the HUD present hook is not armed");
        return;
    }
    implemented = true;
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

}
