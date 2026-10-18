// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Scene renderer on wgpu: draws a [`FlatScene`] from a [`Camera`] into a texture.
//!
//! Behaviour: `docs/specs/m0-render.md`. The app shows the texture inside its egui
//! viewport; tests use [`render_headless`], which runs on Mesa lavapipe without a GPU.

use std::fmt;

use modeler_dae::FlatScene;
use modeler_view::Camera;

/// The color format every viewport texture uses.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// Textures larger than this on either side are downscaled on upload.
pub const MAX_TEXTURE_SIZE: u32 = 2048;

/// How the scene looks, apart from its materials.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// Background, sRGB RGBA 0..1.
    pub background: [f32; 4],
    /// Color for faces without a material, sRGB RGBA 0..1.
    pub default_face: [f32; 4],
    /// Color for lines without a material, sRGB RGBA 0..1.
    pub default_line: [f32; 4],
}

impl Default for Style {
    fn default() -> Self {
        Style {
            background: [0.94, 0.95, 0.96, 1.0],
            default_face: [0.85, 0.85, 0.85, 1.0],
            default_line: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[derive(Debug)]
pub enum RenderError {
    /// No usable adapter or device (for headless use: no Vulkan driver, e.g. lavapipe).
    NoDevice(String),
    /// Reading back the rendered image failed.
    Readback(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::NoDevice(msg) => write!(f, "no GPU device: {msg}"),
            RenderError::Readback(msg) => write!(f, "readback failed: {msg}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Pipelines and shared GPU state. One per device.
pub struct Renderer {
    _private: (),
}

/// A scene uploaded to the GPU.
pub struct GpuScene {
    /// One line per texture that could not be loaded; those materials fall back to
    /// their color.
    pub warnings: Vec<String>,
    /// Center and radius of the scene's bounding sphere, if the scene isn't empty.
    pub sphere: Option<(glam::DVec3, f64)>,
}

/// An offscreen color + depth target that the renderer draws into.
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    /// Color texture in [`COLOR_FORMAT`], usable as a texture binding and copy source.
    pub color: wgpu::Texture,
    pub color_view: wgpu::TextureView,
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Renderer {
        let _ = device;
        unimplemented!("m0-render")
    }

    /// Upload geometry, and load and upload each material's texture.
    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &FlatScene,
    ) -> GpuScene {
        let _ = (device, queue, scene);
        unimplemented!("m0-render")
    }

    /// Create a target of the given size (each side at least 1).
    pub fn viewport(&self, device: &wgpu::Device, width: u32, height: u32) -> Viewport {
        let _ = (device, width, height);
        unimplemented!("m0-render")
    }

    /// Draw `scene` from `camera` into `target`.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &Viewport,
        scene: &GpuScene,
        camera: &Camera,
        style: &Style,
    ) {
        let _ = (device, queue, target, scene, camera, style);
        unimplemented!("m0-render")
    }
}

/// Render one image without a window: creates its own device (Vulkan preferred, any
/// backend accepted), uploads, renders and reads back. Also returns the upload warnings.
pub fn render_headless(
    scene: &FlatScene,
    camera: &Camera,
    width: u32,
    height: u32,
    style: &Style,
) -> Result<(image::RgbaImage, Vec<String>), RenderError> {
    let _ = (scene, camera, width, height, style);
    unimplemented!("m0-render")
}
