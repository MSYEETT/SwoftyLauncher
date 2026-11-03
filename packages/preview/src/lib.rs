//! Runs shader configs on preview clips (DESIGN.md §7).
//!
//! `Preview` compiles nothing itself: it takes effects already compiled to SPIR-V and runs their
//! passes with wgpu on the frames of a `Clip`, with the instance semantics of the core's postfx
//! runtime. Create and use it off the UI thread.

mod clip;
mod gpu;

pub use clip::{CachedClip, Clip, ClipEntry, ClipInfo, ClipStore, Frame, INDEX_URL};
pub use gpu::Preview;
pub use swofty_shaders::ActiveInstance;

/// One compiled effect for the preview: SPIR-V per pass, in pass order.
#[derive(Debug, Clone)]
pub struct CompiledEffect {
    pub effect: swofty_shaders::Effect,
    pub spirv: Vec<Vec<u8>>,
    /// Decoded image textures by texture name (RGBA8, width, height).
    pub images: Vec<(String, u32, u32, Vec<u8>)>,
}
