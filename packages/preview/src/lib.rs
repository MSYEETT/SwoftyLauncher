//! Runs shader configs on preview clips (DESIGN.md §7).

use std::path::Path;

pub use swofty_shaders::ActiveInstance;

/// One compiled effect for the preview: SPIR-V per pass, in pass order.
#[derive(Debug, Clone)]
pub struct CompiledEffect {
    pub effect: swofty_shaders::Effect,
    pub spirv: Vec<Vec<u8>>,
    /// Decoded image textures by texture name (RGBA8, width, height).
    pub images: Vec<(String, u32, u32, Vec<u8>)>,
}

/// A decoded frame: RGBA8 sRGB.
#[derive(Debug, Clone)]
pub struct Frame {
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClipInfo {
    pub title: String,
    pub fps: f32,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    pub view: String,
}

pub struct Clip {
    pub info: ClipInfo,
}

impl Clip {
    pub fn open(dir: &Path) -> Result<Self, String> {
        let _ = dir;
        Err("not written yet".into())
    }

    pub fn len(&self) -> usize {
        self.info.frames
    }

    pub fn is_empty(&self) -> bool {
        self.info.frames == 0
    }

    pub fn frame(&self, index: usize) -> Result<Frame, String> {
        let _ = index;
        Err("not written yet".into())
    }
}

pub struct Preview {}

impl Preview {
    pub fn new() -> Result<Self, String> {
        Err("not written yet".into())
    }

    pub fn set_effects(&mut self, effects: Vec<CompiledEffect>) {
        let _ = effects;
    }

    pub fn set_instances(&mut self, instances: Vec<ActiveInstance>) {
        let _ = instances;
    }

    /// The frame with the instances applied.
    pub fn render(&mut self, frame: &Frame) -> Result<Frame, String> {
        Ok(frame.clone())
    }
}
