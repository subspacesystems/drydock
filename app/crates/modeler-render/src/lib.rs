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

use glam::DVec3;
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
    eye: [f32; 4],
    forward: [f32; 4],
    parallel: u32,
    _pad: [u32; 3],
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
    opaque_pipeline: wgpu::RenderPipeline,
    opaque_back_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    extra_line_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    white_view: wgpu::TextureView,
    _white_texture: wgpu::Texture,
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("modeler-render"),
            bind_group_layouts: &[Some(&camera_layout), Some(&material_layout)],
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
                entry_point: Some("vs_line"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout)],
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
            opaque_pipeline,
            opaque_back_pipeline,
            transparent_pipeline,
            line_pipeline,
            extra_line_pipeline,
            sampler,
            white_view,
            _white_texture: white_texture,
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
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            eye: [eye.x as f32, eye.y as f32, eye.z as f32, 1.0],
            forward: [forward.x as f32, forward.y as f32, forward.z as f32, 0.0],
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

        let background = linear_color(style.background);
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
            .filter(|line| line.a.is_finite() && line.b.is_finite() && line.a != line.b)
            .collect();
        let world_per_pixel = 2.0 * (camera.fov_y * 0.5).tan() / target.height as f64;
        let extra_buffer = (!extra.is_empty()).then(|| {
            let mut vertices = Vec::with_capacity(extra.len() * 6);
            for line in &extra {
                let mid = (line.a + line.b) * 0.5;
                let direction = (line.b - line.a).normalize();
                let view = if camera.projection == Projection::Parallel {
                    forward
                } else {
                    (mid - eye).normalize()
                };
                let mut side = direction.cross(view);
                if side.length_squared() <= 1e-24 {
                    side = direction.cross(camera.basis().1);
                }
                if side.length_squared() <= 1e-24 {
                    side = DVec3::Z;
                }
                side = side.normalize();
                let depth = if camera.projection == Projection::Parallel {
                    camera.distance
                } else {
                    (mid - eye).dot(forward).max(1e-6)
                };
                let offset = side * (world_per_pixel * depth * f64::from(line.width_px) * 0.5);
                for position in [
                    line.a + offset,
                    line.a - offset,
                    line.b - offset,
                    line.a + offset,
                    line.b - offset,
                    line.b + offset,
                ] {
                    vertices.push(Vertex {
                        position: [position.x as f32, position.y as f32, position.z as f32],
                        normal: [0.0; 3],
                        uv: [0.0; 2],
                    });
                }
            }
            create_buffer_init(
                device,
                "modeler-render extra line vertices",
                bytemuck::cast_slice(&vertices),
                wgpu::BufferUsages::VERTEX,
            )
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("modeler-render"),
        });
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
                    pass.draw((start as u32) * 6..(end as u32) * 6, 0..1);
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
