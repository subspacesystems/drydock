// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Scene renderer on wgpu: draws a [`FlatScene`] from a [`Camera`] into a texture.
//!
//! Behaviour: `docs/specs/m0-render.md`. The app shows the texture inside its egui
//! viewport; tests use [`render_headless`], which runs on Mesa lavapipe without a GPU.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use glam::{DVec2, DVec3};
use modeler_dae::{Aabb, FlatScene};
use modeler_view::{Camera, Projection};
use wgpu::util::DeviceExt;

/// The color format every viewport texture uses.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// Share of a lit material's color that doesn't depend on the angle to the viewer.
/// Faces seen head-on show their full color; faces seen edge-on show `AMBIENT` of it.
pub const AMBIENT: f32 = 0.4;
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
    /// `Some`: lit materials are shaded by the sun instead of a light at the viewer
    /// (`docs/specs/m5-render-sun.md`).
    pub sun: Option<SunLight>,
}

/// Sunlight for [`Style::sun`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunLight {
    /// Unit vector toward the sun, world axes (Z up). Below the horizon (`z <= 0`) there is
    /// no direct light.
    pub direction: DVec3,
    /// Share of a material's color that direct sunlight adds on a face square to it, 0..=1.
    pub light: f32,
    /// Share of a material's color without direct sunlight, 0..=1.
    pub dark: f32,
    /// Faces cast shadows (`docs/specs/m5-render-shadows.md`).
    pub shadows: bool,
    /// Shadows also fall on the ground (z = 0) where there is no face.
    pub on_ground: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            background: [0.94, 0.95, 0.96, 1.0],
            default_face: [0.85, 0.85, 0.85, 1.0],
            default_line: [0.0, 0.0, 0.0, 1.0],
            sun: None,
        }
    }
}

/// A line segment drawn with the scene, depth-tested against its faces and lines
/// (`docs/specs/m4-axes-depth.md`, `docs/specs/m4-axes-screen.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtraLine {
    pub a: DVec3,
    pub b: DVec3,
    /// sRGB RGBA 0..1.
    pub color: [f32; 4],
    /// Width on screen in target pixels, the same all along the line.
    pub width_px: f32,
    /// `Some([on, off])`: dashes `on` target pixels long with gaps `off` long, measured on
    /// screen from `a`. `None`: solid.
    pub dash_px: Option<[f32; 2]>,
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

/// Number of samples used when the adapter supports 4x MSAA for the target formats.
const SAMPLE_COUNT: u32 = 4;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Shadow-map resolution on each side, for each cascade.
pub const SHADOW_SIZE: u32 = 2048;
/// Most cascades [`shadow_cascades`] returns.
pub const SHADOW_CASCADES: usize = 4;

/// Serializes headless rendering: creating devices on several threads at once can
/// crash some Vulkan drivers (lavapipe).
static HEADLESS_LOCK: Mutex<()> = Mutex::new(());

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    forward: [f32; 4],
    sun_dir: [f32; 4],
    sun_params: [f32; 4],
    background: [f32; 4],
    parallel: u32,
    _pad: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowUniform {
    light_view_proj: [[[f32; 4]; 4]; SHADOW_CASCADES],
    far: [f32; 4],
    texel: [f32; 4],
    bias: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    color: [f32; 4],
    opacity: f32,
    unlit: u32,
    has_texture: u32,
    _pad: u32,
}

#[derive(Clone, Copy)]
struct GpuMaterial {
    color: [f32; 4],
    opacity: f32,
    unlit: bool,
    texture: Option<usize>,
}

struct LoadedTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    has_alpha: bool,
}

struct CachedTexture {
    texture: Arc<LoadedTexture>,
    modified: Option<SystemTime>,
    len: u64,
}

struct GpuBatch {
    vertex_buffer: wgpu::Buffer,
    index_buffer: Option<wgpu::Buffer>,
    index_count: u32,
    vertex_count: u32,
    center: DVec3,
    transparent: bool,
    material: Option<GpuMaterial>,
}

/// Pipelines and shared GPU state. One per device.
pub struct Renderer {
    camera_layout: wgpu::BindGroupLayout,
    material_layout: wgpu::BindGroupLayout,
    shadow_layout: wgpu::BindGroupLayout,
    opaque_pipeline: wgpu::RenderPipeline,
    opaque_back_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    extra_line_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    ground_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    shadow_sampler: wgpu::Sampler,
    white_view: wgpu::TextureView,
    _white_texture: wgpu::Texture,
    fallback_shadow_view: wgpu::TextureView,
    _fallback_shadow_texture: wgpu::Texture,
    shadow_map: Mutex<Option<wgpu::Texture>>,
    textures: Mutex<HashMap<PathBuf, CachedTexture>>,
    texture_loads: AtomicUsize,
}

/// A scene uploaded to the GPU.
pub struct GpuScene {
    /// One line per texture that could not be loaded; those materials fall back to
    /// their color.
    pub warnings: Vec<String>,
    /// Center and radius of the scene's bounding sphere, if the scene isn't empty.
    pub sphere: Option<(glam::DVec3, f64)>,
    bounds: Option<Aabb>,
    batches: Vec<GpuBatch>,
    lines: Vec<GpuBatch>,
    textures: Vec<Arc<LoadedTexture>>,
}

/// An offscreen color + depth target that the renderer draws into.
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    /// Color texture in [`COLOR_FORMAT`], usable as a texture binding and copy source.
    pub color: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    msaa_view: Option<wgpu::TextureView>,
    _msaa_color: Option<wgpu::Texture>,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_color(c: [f32; 4]) -> [f32; 4] {
    [
        srgb_to_linear(c[0]),
        srgb_to_linear(c[1]),
        srgb_to_linear(c[2]),
        c[3],
    ]
}

fn texture_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn blend_state() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Renderer {
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("modeler-render camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("modeler-render material"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_layout_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("modeler-render shadow"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("modeler-render"),
            bind_group_layouts: &[
                Some(&camera_layout),
                Some(&material_layout),
                Some(&shadow_layout),
            ],
            immediate_size: 0,
        });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("modeler-render shadow"),
                bind_group_layouts: &[Some(&camera_layout)],
                immediate_size: 0,
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("modeler-render scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });

        const ATTRS: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        };
        let targets = [Some(wgpu::ColorTargetState {
            format: COLOR_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let transparent_targets = [Some(wgpu::ColorTargetState {
            format: COLOR_FORMAT,
            blend: Some(blend_state()),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let opaque_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("modeler-render opaque faces"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_face"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout.clone())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_face"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        });
        let opaque_back_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("modeler-render opaque faces from behind"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_face"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout.clone())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 16,
                    slope_scale: 0.05,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_face"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        });
        let transparent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("modeler-render transparent faces"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_face"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout.clone())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_face"),
                compilation_options: Default::default(),
                targets: &transparent_targets,
            }),
            multiview_mask: None,
            cache: None,
        });
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("modeler-render lines"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_line"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout.clone())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_line"),
                compilation_options: Default::default(),
                targets: &transparent_targets,
            }),
            multiview_mask: None,
            cache: None,
        });
        let extra_line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("modeler-render extra lines"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_extra_line"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout.clone())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_line"),
                compilation_options: Default::default(),
                targets: &transparent_targets,
            }),
            multiview_mask: None,
            cache: None,
        });

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("modeler-render shadows"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout.clone())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 16,
                    slope_scale: 3.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        let ground_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("modeler-render ground shadow"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_ground"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_ground"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("modeler-render texture"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("modeler-render shadow"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: Some(wgpu::CompareFunction::LessEqual),
            anisotropy_clamp: 1,
            border_color: None,
        });

        let fallback_shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("modeler-render fallback shadow"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let fallback_shadow_view =
            fallback_shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("modeler-render fallback shadow array"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(1),
                ..Default::default()
            });
        let white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("modeler-render white"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let white_view = white_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Lazily uploaded below through the queue-less path: texture data is written by the
        // first upload call using the queue.
        Renderer {
            camera_layout,
            material_layout,
            shadow_layout,
            opaque_pipeline,
            opaque_back_pipeline,
            transparent_pipeline,
            line_pipeline,
            extra_line_pipeline,
            shadow_pipeline,
            ground_pipeline,
            sampler,
            shadow_sampler,
            white_view,
            _white_texture: white_texture,
            fallback_shadow_view,
            _fallback_shadow_texture: fallback_shadow_texture,
            shadow_map: Mutex::new(None),
            textures: Mutex::new(HashMap::new()),
            texture_loads: AtomicUsize::new(0),
        }
    }

    /// How many times this renderer has tried to read a texture image file (decoded or
    /// failed). Uploads reuse textures already loaded from an unchanged file. Behaviour:
    /// `docs/specs/m4-fix-refresh-speed.md`.
    pub fn texture_loads(&self) -> usize {
        self.texture_loads.load(Ordering::Relaxed)
    }

    /// Upload geometry, and load and upload each material's texture.
    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &FlatScene,
    ) -> GpuScene {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self._white_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let mut warnings = Vec::new();
        let mut textures = Vec::new();
        let mut material_textures = vec![None; scene.materials.len()];
        let mut cache = self
            .textures
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Paths tried by this upload: `Some` index for a texture, `None` for a failure.
        let mut loaded: HashMap<PathBuf, Option<usize>> = HashMap::new();

        for (index, material) in scene.materials.iter().enumerate() {
            let Some(path) = &material.texture else {
                continue;
            };
            if let Some(existing) = loaded.get(path) {
                material_textures[index] = *existing;
                continue;
            }
            let metadata = std::fs::metadata(path).ok();
            let cached = metadata.as_ref().and_then(|metadata| {
                let entry = cache.get(path)?;
                let modified = metadata.modified().ok();
                let len = metadata.len();
                (entry.modified == modified && entry.len == len).then(|| Arc::clone(&entry.texture))
            });
            if let Some(texture) = cached {
                let texture_index = textures.len();
                textures.push(texture);
                loaded.insert(path.clone(), Some(texture_index));
                material_textures[index] = Some(texture_index);
                continue;
            }
            self.texture_loads.fetch_add(1, Ordering::Relaxed);
            match load_texture(device, queue, path) {
                Ok(texture) => {
                    let texture = Arc::new(texture);
                    let texture_index = textures.len();
                    textures.push(Arc::clone(&texture));
                    if let Some(metadata) = metadata {
                        cache.insert(
                            path.clone(),
                            CachedTexture {
                                texture,
                                modified: metadata.modified().ok(),
                                len: metadata.len(),
                            },
                        );
                    }
                    loaded.insert(path.clone(), Some(texture_index));
                    material_textures[index] = Some(texture_index);
                }
                Err(reason) => {
                    warnings.push(format!("{}: {reason}", path.display()));
                    loaded.insert(path.clone(), None);
                }
            }
        }

        let mut batches = Vec::new();
        for batch in &scene.batches {
            if batch.indices.is_empty() || batch.positions.is_empty() {
                continue;
            }
            let texture = batch
                .material
                .and_then(|index| material_textures.get(index).copied().flatten());
            let material = batch.material.and_then(|index| {
                scene.materials.get(index).map(|m| GpuMaterial {
                    color: linear_color(m.color),
                    opacity: m.opacity,
                    unlit: m.unlit,
                    texture,
                })
            });
            let transparent = material.is_some_and(|m| {
                let texture_alpha = m
                    .texture
                    .and_then(|index| textures.get(index))
                    .is_some_and(|texture| texture.has_alpha);
                m.opacity < 1.0 || texture_alpha
            });
            let vertices = build_vertices(batch);
            let vertex_buffer = create_buffer_init(
                device,
                "modeler-render vertices",
                bytemuck::cast_slice(&vertices),
                wgpu::BufferUsages::VERTEX,
            );
            let index_buffer = create_buffer_init(
                device,
                "modeler-render indices",
                bytemuck::cast_slice(&batch.indices),
                wgpu::BufferUsages::INDEX,
            );
            batches.push(GpuBatch {
                vertex_buffer,
                index_buffer: Some(index_buffer),
                index_count: batch.indices.len() as u32,
                vertex_count: vertices.len() as u32,
                center: batch_center(&batch.positions),
                transparent,
                material,
            });
        }

        let mut lines = Vec::new();
        for line in &scene.lines {
            if line.positions.is_empty() {
                continue;
            }
            let material = line.material.and_then(|index| {
                scene.materials.get(index).map(|m| GpuMaterial {
                    color: linear_color(m.color),
                    opacity: m.opacity,
                    unlit: true,
                    texture: None,
                })
            });
            let vertices: Vec<Vertex> = line
                .positions
                .iter()
                .map(|position| Vertex {
                    position: *position,
                    normal: [0.0; 3],
                    uv: [0.0; 2],
                })
                .collect();
            let vertex_buffer = create_buffer_init(
                device,
                "modeler-render line vertices",
                bytemuck::cast_slice(&vertices),
                wgpu::BufferUsages::VERTEX,
            );
            lines.push(GpuBatch {
                vertex_buffer,
                index_buffer: None,
                index_count: 0,
                vertex_count: vertices.len() as u32,
                center: batch_center(&line.positions),
                transparent: false,
                material,
            });
        }

        GpuScene {
            warnings,
            sphere: sphere_from_bounds(scene.bounds),
            bounds: scene.bounds,
            batches,
            lines,
            textures,
        }
    }

    /// Create a target of the given size (each side at least 1).
    pub fn viewport(&self, device: &wgpu::Device, width: u32, height: u32) -> Viewport {
        let width = width.max(1);
        let height = height.max(1);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("modeler-render color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let (msaa_texture, msaa_view) = if SAMPLE_COUNT > 1 {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("modeler-render multisample color"),
                size,
                mip_level_count: 1,
                sample_count: SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (Some(texture), Some(view))
        } else {
            (None, None)
        };
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("modeler-render depth"),
            size,
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        Viewport {
            width,
            height,
            color,
            color_view,
            msaa_view,
            _msaa_color: msaa_texture,
            _depth: depth,
            depth_view,
        }
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
        self.render_with_lines(device, queue, target, scene, camera, style, &[]);
    }

    /// Like [`Renderer::render`], plus `extra` lines drawn in the same pass.
    #[allow(clippy::too_many_arguments)]
    pub fn render_with_lines(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &Viewport,
        scene: &GpuScene,
        camera: &Camera,
        style: &Style,
        extra: &[ExtraLine],
    ) {
        let aspect = target.width as f64 / target.height as f64;
        let view = camera.view_matrix();
        let projection = camera.projection_matrix(aspect, scene.sphere);
        let view_proj = (projection * view).as_mat4();
        let (forward, _, _) = camera.basis();
        let eye = camera.eye();
        let background = linear_color(style.background);
        let sun = style.sun;
        let sun_dir = sun
            .map(|sun| {
                let length = sun.direction.length();
                if sun.direction.is_finite() && length > 1e-9 {
                    sun.direction / length
                } else {
                    DVec3::ZERO
                }
            })
            .unwrap_or(DVec3::ZERO);
        let sun_enabled = sun.is_some();
        let sun_light = sun.map_or(0.0, |sun| sun.light);
        let sun_dark = sun.map_or(0.0, |sun| sun.dark);
        let shadows_on =
            sun.is_some_and(|sun| sun.shadows) && sun_dir.z > 0.0 && scene.bounds.is_some();
        let draw_ground = shadows_on && sun.is_some_and(|sun| sun.on_ground);
        let ground_factor = if sun_dir.z > 0.0 {
            let direct = (sun_dark + sun_light * sun_dir.z as f32).min(1.0);
            if direct > 1e-6 {
                (sun_dark / direct).min(1.0)
            } else {
                1.0
            }
        } else {
            1.0
        };
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: (projection * view).inverse().as_mat4().to_cols_array_2d(),
            eye: [eye.x as f32, eye.y as f32, eye.z as f32, 1.0],
            forward: [forward.x as f32, forward.y as f32, forward.z as f32, 0.0],
            sun_dir: [
                sun_dir.x as f32,
                sun_dir.y as f32,
                sun_dir.z as f32,
                f32::from(sun_enabled),
            ],
            sun_params: [sun_light, sun_dark, ground_factor, 0.0],
            background,
            parallel: u32::from(camera.projection == Projection::Parallel),
            _pad: [0; 3],
        };
        let camera_buffer = create_buffer_init(
            device,
            "modeler-render camera",
            bytemuck::bytes_of(&camera_uniform),
            wgpu::BufferUsages::UNIFORM,
        );
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("modeler-render camera"),
            layout: &self.camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let cascades = if shadows_on {
            shadow_cascades(camera, aspect, scene.bounds.unwrap(), sun_dir, draw_ground)
        } else {
            Vec::new()
        };
        let shadows_on = shadows_on && !cascades.is_empty();
        let draw_ground = draw_ground && shadows_on;
        let shadow_setup = if shadows_on {
            let texture = self.shadow_texture(device);
            let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("modeler-render shadow array"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(SHADOW_CASCADES as u32),
                ..Default::default()
            });
            let (region_min, region_max) =
                shadow_region(scene.bounds.unwrap(), sun_dir, draw_ground);
            let mut nearest_sun = f64::NEG_INFINITY;
            let mut farthest_sun = f64::INFINITY;
            for x in [region_min.x, region_max.x] {
                for y in [region_min.y, region_max.y] {
                    for z in [region_min.z, region_max.z] {
                        let along = DVec3::new(x, y, z).dot(sun_dir);
                        nearest_sun = nearest_sun.max(along);
                        farthest_sun = farthest_sun.min(along);
                    }
                }
            }
            let sun_range = (nearest_sun - farthest_sun).max(1e-9);
            let mut uniform = ShadowUniform {
                light_view_proj: [glam::DMat4::IDENTITY.as_mat4().to_cols_array_2d();
                    SHADOW_CASCADES],
                far: [0.0; 4],
                texel: [0.0; 4],
                bias: [0.0; 4],
                params: [2.0 / SHADOW_SIZE as f32, 0.0, 1.0, cascades.len() as f32],
            };
            for (index, cascade) in cascades.iter().enumerate() {
                uniform.light_view_proj[index] =
                    cascade.light_view_proj.as_mat4().to_cols_array_2d();
                uniform.far[index] = cascade.far as f32;
                uniform.texel[index] = cascade.texel as f32;
                uniform.bias[index] = (1.5 * cascade.texel / sun_range) as f32;
            }
            Some((texture, array_view, uniform))
        } else {
            None
        };
        let color_attachment = wgpu::RenderPassColorAttachment {
            view: target.msaa_view.as_ref().unwrap_or(&target.color_view),
            depth_slice: None,
            resolve_target: target.msaa_view.as_ref().map(|_| &target.color_view),
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: background[0] as f64,
                    g: background[1] as f64,
                    b: background[2] as f64,
                    a: style.background[3] as f64,
                }),
                store: wgpu::StoreOp::Store,
            },
        };
        let depth_attachment = wgpu::RenderPassDepthStencilAttachment {
            view: &target.depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        };
        let extra: Vec<ExtraLine> = extra
            .iter()
            .copied()
            .filter(|line| {
                line.a.is_finite()
                    && line.b.is_finite()
                    && line.a != line.b
                    && line.width_px.is_finite()
                    && line.width_px > 0.0
            })
            .collect();
        let (extra_vertices, extra_ranges) = extra_line_geometry(
            &extra,
            camera,
            &(projection * view),
            target.width,
            target.height,
        );
        let extra_buffer = (!extra_vertices.is_empty()).then(|| {
            create_buffer_init(
                device,
                "modeler-render extra line vertices",
                bytemuck::cast_slice(&extra_vertices),
                wgpu::BufferUsages::VERTEX,
            )
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("modeler-render"),
        });
        if let Some((texture, _, _)) = &shadow_setup {
            for (index, cascade) in cascades.iter().enumerate() {
                let layer_view = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("modeler-render shadow layer"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: index as u32,
                    array_layer_count: Some(1),
                    ..Default::default()
                });
                let light_camera = CameraUniform {
                    view_proj: cascade.light_view_proj.as_mat4().to_cols_array_2d(),
                    inv_view_proj: cascade
                        .light_view_proj
                        .inverse()
                        .as_mat4()
                        .to_cols_array_2d(),
                    eye: [0.0; 4],
                    forward: [sun_dir.x as f32, sun_dir.y as f32, sun_dir.z as f32, 0.0],
                    sun_dir: [0.0; 4],
                    sun_params: [0.0; 4],
                    background: [0.0; 4],
                    parallel: 1,
                    _pad: [0; 3],
                };
                let light_buffer = create_buffer_init(
                    device,
                    "modeler-render light camera",
                    bytemuck::bytes_of(&light_camera),
                    wgpu::BufferUsages::UNIFORM,
                );
                let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("modeler-render light camera"),
                    layout: &self.camera_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: light_buffer.as_entire_binding(),
                    }],
                });
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("modeler-render shadow map"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &layer_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_bind_group(0, &light_bind_group, &[]);
                pass.set_pipeline(&self.shadow_pipeline);
                for batch in &scene.batches {
                    pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    if let Some(index_buffer) = &batch.index_buffer {
                        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..batch.index_count, 0, 0..1);
                    } else {
                        pass.draw(0..batch.vertex_count, 0..1);
                    }
                }
            }
        }
        let fallback_shadow_uniform = ShadowUniform {
            light_view_proj: [glam::DMat4::IDENTITY.as_mat4().to_cols_array_2d(); SHADOW_CASCADES],
            far: [0.0; 4],
            texel: [0.0; 4],
            bias: [0.0; 4],
            params: [0.0; 4],
        };
        let (shadow_view, shadow_uniform) = match &shadow_setup {
            Some((_, view, uniform)) => (view, *uniform),
            None => (&self.fallback_shadow_view, fallback_shadow_uniform),
        };
        let shadow_bind_group = self.shadow_bind_group(device, shadow_view, &shadow_uniform);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("modeler-render"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: Some(depth_attachment),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &camera_bind_group, &[]);
            pass.set_bind_group(2, &shadow_bind_group, &[]);

            if draw_ground {
                pass.set_pipeline(&self.ground_pipeline);
                let ground_material = self.material_bind_group(
                    device,
                    scene,
                    GpuMaterial {
                        color: [1.0; 4],
                        opacity: 1.0,
                        unlit: true,
                        texture: None,
                    },
                );
                pass.set_bind_group(1, &ground_material, &[]);
                pass.draw(0..3, 0..1);
            }

            pass.set_pipeline(&self.opaque_pipeline);
            for batch in scene.batches.iter().filter(|batch| !batch.transparent) {
                let material = batch.material.unwrap_or_else(|| GpuMaterial {
                    color: linear_color(style.default_face),
                    opacity: style.default_face[3],
                    unlit: false,
                    texture: None,
                });
                self.draw(device, &mut pass, scene, batch, material);
            }

            pass.set_pipeline(&self.opaque_back_pipeline);
            for batch in scene.batches.iter().filter(|batch| !batch.transparent) {
                let material = batch.material.unwrap_or_else(|| GpuMaterial {
                    color: linear_color(style.default_face),
                    opacity: style.default_face[3],
                    unlit: false,
                    texture: None,
                });
                self.draw(device, &mut pass, scene, batch, material);
            }

            pass.set_pipeline(&self.line_pipeline);
            for batch in &scene.lines {
                let material = batch.material.unwrap_or_else(|| GpuMaterial {
                    color: linear_color(style.default_line),
                    opacity: style.default_line[3],
                    unlit: true,
                    texture: None,
                });
                self.draw(device, &mut pass, scene, batch, material);
            }

            if let Some(extra_buffer) = &extra_buffer {
                pass.set_pipeline(&self.extra_line_pipeline);
                pass.set_vertex_buffer(0, extra_buffer.slice(..));
                let mut start = 0;
                while start < extra.len() {
                    let color = extra[start].color;
                    let mut end = start + 1;
                    while end < extra.len() && extra[end].color == color {
                        end += 1;
                    }
                    let material = GpuMaterial {
                        color: linear_color(color),
                        opacity: color[3],
                        unlit: true,
                        texture: None,
                    };
                    let bind_group = self.material_bind_group(device, scene, material);
                    pass.set_bind_group(1, &bind_group, &[]);
                    let first = extra_ranges[start].0;
                    let last = extra_ranges[end - 1].1;
                    if last > first {
                        pass.draw(first..last, 0..1);
                    }
                    start = end;
                }
            }

            pass.set_pipeline(&self.transparent_pipeline);
            let mut transparent: Vec<&GpuBatch> = scene
                .batches
                .iter()
                .filter(|batch| batch.transparent)
                .collect();
            transparent.sort_by(|a, b| {
                let da = (a.center - eye).length();
                let db = (b.center - eye).length();
                db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
            });
            for batch in transparent {
                let material = batch.material.unwrap_or_else(|| GpuMaterial {
                    color: linear_color(style.default_face),
                    opacity: style.default_face[3],
                    unlit: false,
                    texture: None,
                });
                self.draw(device, &mut pass, scene, batch, material);
            }
        }
        queue.submit(Some(encoder.finish()));
    }

    fn shadow_texture(&self, device: &wgpu::Device) -> wgpu::Texture {
        let mut map = self
            .shadow_map
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if map.is_none() {
            *map = Some(device.create_texture(&wgpu::TextureDescriptor {
                label: Some("modeler-render shadow maps"),
                size: wgpu::Extent3d {
                    width: SHADOW_SIZE,
                    height: SHADOW_SIZE,
                    depth_or_array_layers: SHADOW_CASCADES as u32,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }));
        }
        map.as_ref()
            .expect("shadow texture was just created")
            .clone()
    }

    fn shadow_bind_group(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        uniform: &ShadowUniform,
    ) -> wgpu::BindGroup {
        let buffer = create_buffer_init(
            device,
            "modeler-render shadow",
            bytemuck::bytes_of(uniform),
            wgpu::BufferUsages::UNIFORM,
        );
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("modeler-render shadow"),
            layout: &self.shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer.as_entire_binding(),
                },
            ],
        })
    }

    fn material_bind_group(
        &self,
        device: &wgpu::Device,
        scene: &GpuScene,
        material: GpuMaterial,
    ) -> wgpu::BindGroup {
        let uniform = MaterialUniform {
            color: material.color,
            opacity: material.opacity,
            unlit: u32::from(material.unlit),
            has_texture: u32::from(material.texture.is_some()),
            _pad: 0,
        };
        let uniform_buffer = create_buffer_init(
            device,
            "modeler-render material",
            bytemuck::bytes_of(&uniform),
            wgpu::BufferUsages::UNIFORM,
        );
        let texture_view = material
            .texture
            .and_then(|index| scene.textures.get(index))
            .map_or(&self.white_view, |texture| &texture.view);
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("modeler-render material"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    fn draw(
        &self,
        device: &wgpu::Device,
        pass: &mut wgpu::RenderPass<'_>,
        scene: &GpuScene,
        batch: &GpuBatch,
        material: GpuMaterial,
    ) {
        let bind_group = self.material_bind_group(device, scene, material);
        pass.set_bind_group(1, &bind_group, &[]);
        pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
        if let Some(index_buffer) = &batch.index_buffer {
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..batch.index_count, 0, 0..1);
        } else {
            pass.draw(0..batch.vertex_count, 0..1);
        }
    }
}

fn create_buffer_init(
    device: &wgpu::Device,
    label: &str,
    contents: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage,
    })
}

fn screen_fraction_to_param(f: f64, w0: f64, w1: f64) -> f64 {
    if (w0 - w1).abs() <= 1e-30 {
        f
    } else {
        (f / w1) / ((1.0 - f) / w0 + f / w1)
    }
}

/// Build screen-sized quads for `extra` lines, splitting them where the clip depth leaves
/// the [0, w] range and cutting dashes on the CPU. Returns the vertices and, per line,
/// the vertex range it occupies.
fn extra_line_geometry(
    extra: &[ExtraLine],
    camera: &Camera,
    view_proj: &glam::DMat4,
    width: u32,
    height: u32,
) -> (Vec<Vertex>, Vec<(u32, u32)>) {
    const MAX_DASHES: usize = 65_536;
    let world_per_pixel = 2.0 * (camera.fov_y * 0.5).tan() / f64::from(height);
    let (forward, right, _) = camera.basis();
    let eye = camera.eye();
    let parallel = camera.projection == Projection::Parallel;
    // Stop a little in front of the eye so the projection never divides by zero.
    let w_min = (camera.distance.abs() * 1e-4).max(1e-9);
    let (width, height) = (f64::from(width), f64::from(height));
    let mut vertices = Vec::new();
    let mut ranges = Vec::with_capacity(extra.len());

    for line in extra {
        let start = vertices.len() as u32;
        let (a, b) = (line.a, line.b);
        let ca = *view_proj * glam::DVec4::new(a.x, a.y, a.z, 1.0);
        let cb = *view_proj * glam::DVec4::new(b.x, b.y, b.z, 1.0);
        let dw = cb.w - ca.w;
        let dz = cb.z - ca.z;

        let (t0, t1) = if parallel {
            (0.0, 1.0)
        } else {
            let (mut lo, mut hi): (f64, f64) = (0.0, 1.0);
            if dw.abs() <= 1e-30 {
                if ca.w <= w_min {
                    ranges.push((start, start));
                    continue;
                }
            } else {
                let t = (w_min - ca.w) / dw;
                if dw > 0.0 {
                    lo = lo.max(t);
                } else {
                    hi = hi.min(t);
                }
            }
            if hi <= lo {
                ranges.push((start, start));
                continue;
            }
            (lo, hi)
        };

        let direction = (b - a).normalize();
        let mut side = if parallel {
            direction.cross(forward)
        } else {
            direction.cross((a + b) * 0.5 - eye)
        };
        if side.length_squared() <= 1e-24 {
            side = direction.cross(right);
        }
        if side.length_squared() <= 1e-24 {
            side = DVec3::Z;
        }
        let side = side.normalize();
        let half_width = f64::from(line.width_px) * 0.5;

        let screen_at = |t: f64| -> DVec2 {
            let clip = ca + (cb - ca) * t;
            DVec2::new(
                (clip.x / clip.w + 1.0) * 0.5 * width,
                (1.0 - clip.y / clip.w) * 0.5 * height,
            )
        };

        // Parameters where the clip depth crosses the near plane (z = 0) or the far
        // plane (z = w). Each piece on one side keeps the shader's clamp exact.
        let mut cuts = vec![t0, t1];
        if dz.abs() > 1e-30 {
            let t = -ca.z / dz;
            if t > t0 && t < t1 {
                cuts.push(t);
            }
        }
        let depth_span = dz - dw;
        if depth_span.abs() > 1e-30 {
            let t = (ca.w - ca.z) / depth_span;
            if t > t0 && t < t1 {
                cuts.push(t);
            }
        }
        cuts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        cuts.dedup_by(|x, y| (*x - *y).abs() <= 1e-12);

        let dash = line.dash_px.and_then(|[on, off]| {
            (on.is_finite() && off.is_finite() && on > 0.0 && off > 0.0)
                .then_some((f64::from(on), f64::from(off)))
        });
        let mut spans: Vec<(f64, f64)> = Vec::new();
        match dash {
            None => spans.push((t0, t1)),
            Some((on, off)) => {
                let length = screen_at(t0).distance(screen_at(t1));
                if !length.is_finite() || length <= 1e-9 {
                    spans.push((t0, t1));
                } else {
                    let w0 = ca.w + dw * t0;
                    let w1 = ca.w + dw * t1;
                    let span = t1 - t0;
                    let mut k = 0;
                    while k < MAX_DASHES {
                        let start_px = k as f64 * (on + off);
                        if start_px >= length {
                            break;
                        }
                        let f0 = start_px / length;
                        let f1 = ((start_px + on) / length).min(1.0);
                        let u0 = screen_fraction_to_param(f0, w0, w1);
                        let u1 = screen_fraction_to_param(f1, w0, w1);
                        let (ta, tb) = (t0 + u0 * span, t0 + u1 * span);
                        if tb > ta {
                            spans.push((ta, tb));
                        }
                        k += 1;
                    }
                }
            }
        }

        for (ta, tb) in spans {
            let mut bounds = vec![ta, tb];
            bounds.extend(cuts.iter().copied().filter(|c| *c > ta && *c < tb));
            bounds.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
            for pair in bounds.windows(2) {
                let (u0, u1) = (pair[0], pair[1]);
                if u1 - u0 <= 1e-12 {
                    continue;
                }
                let p0 = a + (b - a) * u0;
                let p1 = a + (b - a) * u1;
                let d0 = if parallel {
                    camera.distance.abs()
                } else {
                    ca.w + dw * u0
                };
                let d1 = if parallel {
                    camera.distance.abs()
                } else {
                    ca.w + dw * u1
                };
                if d0 <= 0.0 || d1 <= 0.0 {
                    continue;
                }
                let o0 = side * (world_per_pixel * d0 * half_width);
                let o1 = side * (world_per_pixel * d1 * half_width);
                for position in [p0 + o0, p0 - o0, p1 - o1, p0 + o0, p1 - o1, p1 + o1] {
                    vertices.push(Vertex {
                        position: [position.x as f32, position.y as f32, position.z as f32],
                        normal: [0.0; 3],
                        uv: [0.0; 2],
                    });
                }
            }
        }
        ranges.push((start, vertices.len() as u32));
    }
    (vertices, ranges)
}

fn build_vertices(batch: &modeler_dae::Batch) -> Vec<Vertex> {
    batch
        .positions
        .iter()
        .enumerate()
        .map(|(index, position)| Vertex {
            position: *position,
            normal: batch.normals.get(index).copied().unwrap_or([0.0, 0.0, 1.0]),
            uv: batch.texcoords.get(index).copied().unwrap_or([0.0, 0.0]),
        })
        .collect()
}

fn batch_center(positions: &[[f32; 3]]) -> DVec3 {
    let mut min = DVec3::splat(f64::MAX);
    let mut max = DVec3::splat(f64::MIN);
    for p in positions {
        let p = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        min = min.min(p);
        max = max.max(p);
    }
    (min + max) * 0.5
}

/// The box shadows are needed in: the scene's bounds plus, when shadows fall on the
/// ground, the ground area (z = 0) the scene's shadow can reach along `sun`. Returns the
/// box's min and max corners.
pub fn shadow_region(bounds: Aabb, sun: DVec3, on_ground: bool) -> (DVec3, DVec3) {
    let mut min = DVec3::new(
        bounds.min[0] as f64,
        bounds.min[1] as f64,
        bounds.min[2] as f64,
    );
    let mut max = DVec3::new(
        bounds.max[0] as f64,
        bounds.max[1] as f64,
        bounds.max[2] as f64,
    );
    if on_ground && sun.z > 0.0 {
        for x in [min.x, max.x] {
            for y in [min.y, max.y] {
                for z in [min.z, max.z] {
                    if z >= 0.0 {
                        let p = DVec3::new(x, y, z);
                        let q = p - sun * (z / sun.z);
                        min = min.min(q);
                        max = max.max(q);
                    }
                }
            }
        }
    }
    (min, max)
}

/// One slice of the view's depth range and the light camera whose shadow map covers it
/// (`docs/specs/m5-shadow-cascades.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowCascade {
    /// View depth (distance along the camera's forward from the eye) where this cascade
    /// ends. A fragment uses the first cascade whose `far` is at least its depth.
    pub far: f64,
    /// World to the light's clip space: orthographic along `-sun`, x and y in -1..1 over
    /// the map, depth 0..1.
    pub light_view_proj: glam::DMat4,
    /// World size of one shadow-map texel (square texels).
    pub texel: f64,
}

/// The shadow cascades for one frame: 1 to [`SHADOW_CASCADES`] slices of the camera's
/// view of [`shadow_region`], nearest first, each with a light camera fitted to it.
/// Empty when the sun is at or below the horizon or nothing of the region is in front
/// of the camera. Behaviour: `docs/specs/m5-shadow-cascades.md`.
pub fn shadow_cascades(
    camera: &Camera,
    aspect: f64,
    bounds: Aabb,
    sun: DVec3,
    on_ground: bool,
) -> Vec<ShadowCascade> {
    let sun_length = sun.length();
    if !sun.is_finite() || !sun_length.is_finite() || sun_length <= 1e-12 {
        return Vec::new();
    }
    let sun = sun / sun_length;
    if !sun.is_finite() || sun.z <= 0.0 {
        return Vec::new();
    }

    let (min, max) = shadow_region(bounds, sun, on_ground);
    if !min.is_finite() || !max.is_finite() {
        return Vec::new();
    }
    let (forward, _, _) = camera.basis();
    let eye = camera.eye();
    if !forward.is_finite() || !eye.is_finite() {
        return Vec::new();
    }
    let perspective = camera.projection == Projection::Perspective;
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };

    let mut depths = [0.0f64; 8];
    let mut along_sun = [0.0f64; 8];
    let mut index = 0;
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                let p = DVec3::new(x, y, z);
                depths[index] = (p - eye).dot(forward);
                along_sun[index] = p.dot(sun);
                index += 1;
            }
        }
    }
    let mut nearest_depth = f64::INFINITY;
    let mut deepest = f64::NEG_INFINITY;
    let mut nearest_sun = f64::NEG_INFINITY;
    let mut farthest_sun = f64::INFINITY;
    for i in 0..8 {
        nearest_depth = nearest_depth.min(depths[i]);
        deepest = deepest.max(depths[i]);
        nearest_sun = nearest_sun.max(along_sun[i]);
        farthest_sun = farthest_sun.min(along_sun[i]);
    }
    if perspective && deepest <= 1e-9 {
        return Vec::new();
    }
    if !deepest.is_finite() || !nearest_depth.is_finite() {
        return Vec::new();
    }

    let near = if perspective { 0.0 } else { nearest_depth };
    let distance = camera.distance.abs();
    let mut fars = Vec::with_capacity(SHADOW_CASCADES);
    if distance.is_finite() && distance > 0.0 {
        for i in 0..SHADOW_CASCADES.saturating_sub(1) {
            let far = 2.0 * distance * 4f64.powi(i as i32);
            if far > near + 1e-9 && far < deepest - 1e-9 {
                fars.push(far);
            }
        }
    }
    fars.push(deepest);

    let (axis_x, axis_y) = light_axes(sun);
    let tan_half = (camera.fov_y * 0.5).tan().abs();
    let parallel_half_height = distance * tan_half;
    let parallel_half_width = parallel_half_height * aspect;

    let center_r = (min + max) * 0.5;
    let radius_r = ((max - min).length() * 0.5).max(1e-9);
    let sun_range = (nearest_sun - farthest_sun).max(1e-9);
    let depth_pad = sun_range * 1e-9 + 1e-12;
    let z_scale = 1.0 / (sun_range + 2.0 * depth_pad);

    let mut cascades = Vec::with_capacity(fars.len());
    for (i, &far) in fars.iter().enumerate() {
        let lower = if i == 0 { near } else { fars[i - 1] };
        let (center, radius) = if perspective {
            let depth = far.max(0.0);
            let half_height = depth * tan_half;
            let half_width = half_height * aspect;
            let span = (far - lower).max(0.0);
            let center = eye + forward * ((lower + far) * 0.5);
            let radius = 0.5
                * ((2.0 * half_width).powi(2) + (2.0 * half_height).powi(2) + span * span).sqrt();
            (center, radius)
        } else {
            let span = (far - lower).max(0.0);
            let center = eye + forward * ((lower + far) * 0.5);
            let radius = 0.5
                * ((2.0 * parallel_half_width).powi(2)
                    + (2.0 * parallel_half_height).powi(2)
                    + span * span)
                    .sqrt();
            (center, radius)
        };
        let (center, radius) = if radius_r < radius {
            (center_r, radius_r)
        } else {
            (center, radius)
        };
        let extent = radius * (1.0 + 8.0 / SHADOW_SIZE as f64) + 1e-12;
        let texel = 2.0 * extent / SHADOW_SIZE as f64;
        let cx = (center.dot(axis_x) / texel).round() * texel;
        let cy = (center.dot(axis_y) / texel).round() * texel;
        let light_view_proj = glam::DMat4::from_cols(
            glam::DVec4::new(axis_x.x / extent, axis_y.x / extent, -sun.x * z_scale, 0.0),
            glam::DVec4::new(axis_x.y / extent, axis_y.y / extent, -sun.y * z_scale, 0.0),
            glam::DVec4::new(axis_x.z / extent, axis_y.z / extent, -sun.z * z_scale, 0.0),
            glam::DVec4::new(
                -cx / extent,
                -cy / extent,
                (nearest_sun + depth_pad) * z_scale,
                1.0,
            ),
        );
        cascades.push(ShadowCascade {
            far,
            light_view_proj,
            texel,
        });
    }
    cascades
}

/// Fixed light-space x and y axes for a sun direction, chosen so camera motion can
/// snap the shadow-map center to whole texels.
fn light_axes(sun: DVec3) -> (DVec3, DVec3) {
    let x = if sun.z.abs() > 8f64.to_radians().cos() {
        DVec3::Y.cross(sun)
    } else {
        DVec3::Z.cross(sun)
    };
    let x = if x.length_squared() > 1e-24 {
        x.normalize()
    } else {
        DVec3::X
    };
    let y = sun.cross(x).normalize();
    (x, y)
}

fn sphere_from_bounds(bounds: Option<Aabb>) -> Option<(DVec3, f64)> {
    bounds.map(|bounds| {
        let min = DVec3::new(
            bounds.min[0] as f64,
            bounds.min[1] as f64,
            bounds.min[2] as f64,
        );
        let max = DVec3::new(
            bounds.max[0] as f64,
            bounds.max[1] as f64,
            bounds.max[2] as f64,
        );
        ((min + max) * 0.5, (max - min).length() * 0.5)
    })
}

fn load_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &std::path::Path,
) -> Result<LoadedTexture, String> {
    let image = image::open(path)
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let scale = if width > MAX_TEXTURE_SIZE || height > MAX_TEXTURE_SIZE {
        MAX_TEXTURE_SIZE as f64 / width.max(height) as f64
    } else {
        1.0
    };
    let width = ((width as f64 * scale).round() as u32).max(1);
    let height = ((height as f64 * scale).round() as u32).max(1);
    let image = if scale < 1.0 {
        image::imageops::resize(&image, width, height, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    let has_alpha = image.pixels().any(|pixel| pixel.0[3] < 255);
    let mip_level_count = 32 - width.max(height).leading_zeros();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("modeler-render texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for level in 0..mip_level_count {
        let level_width = (width >> level).max(1);
        let level_height = (height >> level).max(1);
        let pixels = if level == 0 {
            image.clone()
        } else {
            image::imageops::resize(
                &image,
                level_width,
                level_height,
                image::imageops::FilterType::Triangle,
            )
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(level_width * 4),
                rows_per_image: Some(level_height),
            },
            wgpu::Extent3d {
                width: level_width,
                height: level_height,
                depth_or_array_layers: 1,
            },
        );
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Ok(LoadedTexture {
        _texture: texture,
        view,
        has_alpha,
    })
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
    render_headless_with_lines(scene, camera, width, height, style, &[])
}

/// [`render_headless`] with `extra` lines drawn in the same pass.
pub fn render_headless_with_lines(
    scene: &FlatScene,
    camera: &Camera,
    width: u32,
    height: u32,
    style: &Style,
    extra: &[ExtraLine],
) -> Result<(image::RgbaImage, Vec<String>), RenderError> {
    let _guard = HEADLESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|error| RenderError::NoDevice(error.to_string()))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|error| RenderError::NoDevice(error.to_string()))?;
    let renderer = Renderer::new(&device);
    let width = width.max(1);
    let height = height.max(1);
    let gpu_scene = renderer.upload(&device, &queue, scene);
    let viewport = renderer.viewport(&device, width, height);
    renderer.render_with_lines(&device, &queue, &viewport, &gpu_scene, camera, style, extra);
    let image = readback(&device, &queue, &viewport)?;
    Ok((image, gpu_scene.warnings))
}

fn readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    viewport: &Viewport,
) -> Result<image::RgbaImage, RenderError> {
    let unpadded_bytes_per_row = viewport.width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
    let buffer_size = u64::from(padded_bytes_per_row) * u64::from(viewport.height);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("modeler-render readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("modeler-render readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &viewport.color,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(viewport.height),
            },
        },
        wgpu::Extent3d {
            width: viewport.width,
            height: viewport.height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| RenderError::Readback(error.to_string()))?;
    receiver
        .recv()
        .map_err(|error| RenderError::Readback(error.to_string()))?
        .map_err(|error| RenderError::Readback(error.to_string()))?;
    let data = slice
        .get_mapped_range()
        .map_err(|error| RenderError::Readback(error.to_string()))?;
    let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * viewport.height) as usize);
    for row in 0..viewport.height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        pixels.extend_from_slice(&data[start..end]);
    }
    drop(data);
    buffer.unmap();
    image::RgbaImage::from_raw(viewport.width, viewport.height, pixels)
        .ok_or_else(|| RenderError::Readback("image size mismatch".into()))
}
