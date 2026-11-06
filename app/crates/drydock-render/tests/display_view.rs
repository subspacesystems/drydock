// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-viewport-gamma: the view handed to egui reads the stored bytes as plain values
//! (`docs/specs/m10-viewport-gamma.md`). egui's shader treats what it samples as gamma
//! values, so an sRGB view would be linearized on sampling and shown too dark.

use std::sync::Mutex;

use drydock_dae::FlatScene;
use drydock_render::{DISPLAY_FORMAT, Renderer, Style};
use drydock_view::{Camera, Projection, StandardView};
use glam::DVec3;

static GPU: Mutex<()> = Mutex::new(());

const BG: [u8; 3] = [0x05, 0x0A, 0x19];

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

fn camera() -> Camera {
    let fov_y = 35f64.to_radians();
    let mut c = Camera {
        target: DVec3::ZERO,
        distance: 2.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y,
        projection: Projection::Parallel,
    };
    c.set_standard_view(StandardView::Top);
    c
}

const SHADER: &str = "
@group(0) @binding(0) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(t, vec2<i32>(pos.xy), 0);
}
";

/// Sample `view` into an `Rgba8Unorm` target without any conversion; return pixel (1, 1).
fn sample(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    size: u32,
) -> [u8; 4] {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(view),
        }],
    });
    let extent = wgpu::Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());
    let row = 256u32; // size * 4 <= 256 for the sizes used here
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row) * u64::from(size),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(size),
            },
        },
        extent,
    );
    queue.submit(Some(encoder.finish()));
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let data = slice.get_mapped_range().unwrap();
    let at = (row + 4) as usize;
    [data[at], data[at + 1], data[at + 2], data[at + 3]]
}

/// The Dark background, rendered into the viewport and sampled through the display
/// view, comes out as the brand colour, not linearized twice.
#[test]
fn display_view_shows_the_background_unchanged() {
    let _g = GPU.lock().unwrap_or_else(|p| p.into_inner());
    let Some((device, queue)) = device() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let renderer = Renderer::new(&device);
    let scene = renderer.upload(&device, &queue, &FlatScene::default());
    let viewport = renderer.viewport(&device, 32, 32);
    let style = Style {
        background: [
            f32::from(BG[0]) / 255.0,
            f32::from(BG[1]) / 255.0,
            f32::from(BG[2]) / 255.0,
            1.0,
        ],
        ..Style::default()
    };
    renderer.render(&device, &queue, &viewport, &scene, &camera(), &style);
    let px = sample(&device, &queue, &viewport.display_view, 32);
    for c in 0..3 {
        assert!(
            px[c].abs_diff(BG[c]) <= 2,
            "channel {c}: got {px:?}, want {BG:?}"
        );
    }
}

/// Every size has a display view in `DISPLAY_FORMAT`, and the texture itself stays sRGB.
#[test]
fn every_viewport_has_a_display_view() {
    let _g = GPU.lock().unwrap_or_else(|p| p.into_inner());
    let Some((device, _queue)) = device() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let renderer = Renderer::new(&device);
    assert_eq!(DISPLAY_FORMAT, wgpu::TextureFormat::Rgba8Unorm);
    for (w, h) in [(1, 1), (0, 0), (7, 3), (64, 48), (200, 100)] {
        let viewport = renderer.viewport(&device, w, h);
        assert_eq!(viewport.color.format(), drydock_render::COLOR_FORMAT);
        // A view in the display format can only be made from a texture that lists it;
        // sampling it is checked above. Here: the field exists and sizes are clamped.
        assert_eq!((viewport.width, viewport.height), (w.max(1), h.max(1)));
        let _ = &viewport.display_view;
    }
}
