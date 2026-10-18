# render: D3D12/DXGI present path and the swapchain overlay

Owner: see `subsystems.tsv`. Consumer: `native/src/core/hud.cpp` (the overlay standard library, experimental `overlay` feature gate).

## Map
- The game renders with Direct3D 12 (`d3d12core.dll` + `dxgi.dll`); there is no exe-side present call site catalogued yet.
- The overlay reaches the present path through the stable COM vtable ABI, not through any `F1_25.exe` address, so it needs no KB VA and survives game updates. At arm time it creates a throwaway `D3D12CreateDevice` + `CreateDXGIFactory1` + `CreateSwapChainForHwnd` probe, reads three vtable slots, installs safetyhook inline hooks on the shared code they point at, then destroys the probe:
  - `IDXGISwapChain::Present` — vtable slot 8 (vt+0x40). Draws the frame, then calls the original.
  - `IDXGISwapChain::ResizeBuffers` — vtable slot 13 (vt+0x68). Releases the overlay's back-buffer references before the game resizes.
  - `ID3D12CommandQueue::ExecuteCommandLists` — vtable slot 10 (vt+0x50). Captures the game's `DIRECT` command queue (the one Present needs to submit the overlay's command list).

## Open
- The exe-side present call site (the render-loop function that calls `swapchain->Present`): decompile and catalogue as `Render_*` once observed in game, for a lower-overhead exe hook than the vtable probe.
- Text and image rendering: the first landing paints solid rects / outlines / lines only; a font atlas and textured-quad path remain.

## Inbox
