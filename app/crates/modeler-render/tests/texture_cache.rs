// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-fix-refresh-speed: the renderer reuses textures across uploads
//! (`docs/specs/m4-fix-refresh-speed.md`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use glam::DVec3;
use modeler_dae::{Aabb, Batch, FlatScene, Material};
use modeler_render::{Renderer, Style};
use modeler_view::{Camera, Projection, StandardView};

/// One device at a time: lavapipe is slow to start several in parallel.
static GPU: Mutex<()> = Mutex::new(());

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
}

fn gpu() -> Gpu {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("headless adapter (lavapipe) available");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("device");
    let renderer = Renderer::new(&device);
    Gpu {
        device,
        queue,
        renderer,
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("modeler-texcache-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_png(path: &Path, size: u32, rgba: [u8; 4]) {
    image::RgbaImage::from_pixel(size, size, image::Rgba(rgba))
        .save(path)
        .unwrap();
}

/// Set the file's modification time `seconds` after the Unix epoch plus a fixed base, so
/// a rewrite is always seen as newer whatever the file system's time resolution.
fn set_mtime(path: &Path, seconds: u64) {
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + seconds))
        .unwrap();
}

fn textured(name: &str, path: &Path) -> Material {
    Material {
        id: name.into(),
        name: name.into(),
        color: [1.0, 1.0, 1.0, 1.0],
        texture: Some(path.to_path_buf()),
        opacity: 1.0,
        unlit: false,
    }
}

fn quad(material: usize, x: f32) -> Batch {
    Batch {
        material: Some(material),
        positions: vec![
            [x - 0.5, -0.5, 0.0],
            [x + 0.5, -0.5, 0.0],
            [x + 0.5, 0.5, 0.0],
            [x - 0.5, 0.5, 0.0],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        texcoords: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

fn scene(materials: Vec<Material>) -> FlatScene {
    let n = materials.len();
    FlatScene {
        materials,
        batches: (0..n).map(|i| quad(i, i as f32 * 1.5)).collect(),
        lines: vec![],
        bounds: Some(Aabb {
            min: [-0.5, -0.5, 0.0],
            max: [n as f32 * 1.5, 0.5, 0.0],
        }),
    }
}

fn camera() -> Camera {
    let mut c = Camera {
        target: DVec3::ZERO,
        distance: 5.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 35f64.to_radians(),
        projection: Projection::Parallel,
    };
    c.set_standard_view(StandardView::Top);
    c
}

#[test]
fn a_new_renderer_has_loaded_nothing() {
    let _lock = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let g = gpu();
    assert_eq!(g.renderer.texture_loads(), 0);
}

#[test]
fn uploading_the_same_scene_again_reads_no_file() {
    let _lock = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_dir("again");
    let (a, b) = (dir.join("a.png"), dir.join("b.png"));
    write_png(&a, 8, [255, 0, 0, 255]);
    write_png(&b, 8, [0, 0, 255, 255]);
    let s = scene(vec![textured("a", &a), textured("b", &b)]);
    let g = gpu();
    let first = g.renderer.upload(&g.device, &g.queue, &s);
    assert!(first.warnings.is_empty(), "{:?}", first.warnings);
    assert_eq!(g.renderer.texture_loads(), 2);
    for _ in 0..3 {
        let again = g.renderer.upload(&g.device, &g.queue, &s);
        assert!(again.warnings.is_empty(), "{:?}", again.warnings);
    }
    assert_eq!(g.renderer.texture_loads(), 2);
}

#[test]
fn materials_sharing_a_file_read_it_once() {
    let _lock = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_dir("shared");
    let a = dir.join("a.png");
    write_png(&a, 8, [0, 255, 0, 255]);
    let s = scene(vec![
        textured("one", &a),
        textured("two", &a),
        textured("three", &a),
    ]);
    let g = gpu();
    let _ = g.renderer.upload(&g.device, &g.queue, &s);
    assert_eq!(g.renderer.texture_loads(), 1);
}

#[test]
fn a_scene_with_a_new_texture_reads_only_that_file() {
    let _lock = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_dir("added");
    let (a, b) = (dir.join("a.png"), dir.join("b.png"));
    write_png(&a, 8, [255, 0, 0, 255]);
    write_png(&b, 8, [0, 0, 255, 255]);
    let g = gpu();
    let _ = g
        .renderer
        .upload(&g.device, &g.queue, &scene(vec![textured("a", &a)]));
    assert_eq!(g.renderer.texture_loads(), 1);
    let _ = g.renderer.upload(
        &g.device,
        &g.queue,
        &scene(vec![textured("a", &a), textured("b", &b)]),
    );
    assert_eq!(g.renderer.texture_loads(), 2);
}

#[test]
fn a_texture_dropped_from_one_scene_is_still_reused_later() {
    let _lock = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_dir("dropped");
    let (a, b) = (dir.join("a.png"), dir.join("b.png"));
    write_png(&a, 8, [255, 0, 0, 255]);
    write_png(&b, 8, [0, 0, 255, 255]);
    let g = gpu();
    let both = scene(vec![textured("a", &a), textured("b", &b)]);
    let _ = g.renderer.upload(&g.device, &g.queue, &both);
    let _ = g
        .renderer
        .upload(&g.device, &g.queue, &scene(vec![textured("a", &a)]));
    let _ = g.renderer.upload(&g.device, &g.queue, &both);
    assert_eq!(g.renderer.texture_loads(), 2);
}

#[test]
fn a_changed_file_is_read_again() {
    let _lock = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_dir("changed");
    let a = dir.join("a.png");
    write_png(&a, 8, [255, 0, 0, 255]);
    set_mtime(&a, 0);
    let s = scene(vec![textured("a", &a)]);
    let g = gpu();
    let _ = g.renderer.upload(&g.device, &g.queue, &s);
    assert_eq!(g.renderer.texture_loads(), 1);
    // Same path, new content and a later modification time.
    write_png(&a, 16, [0, 255, 0, 255]);
    set_mtime(&a, 100);
    let again = g.renderer.upload(&g.device, &g.queue, &s);
    assert!(again.warnings.is_empty(), "{:?}", again.warnings);
    assert_eq!(g.renderer.texture_loads(), 2);
    let _ = g.renderer.upload(&g.device, &g.queue, &s);
    assert_eq!(g.renderer.texture_loads(), 2);
}

#[test]
fn a_missing_file_warns_on_every_upload_and_loads_once_it_exists() {
    let _lock = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_dir("missing");
    let a = dir.join("late.png");
    let s = scene(vec![textured("a", &a)]);
    let g = gpu();
    for attempt in 1..=2 {
        let up = g.renderer.upload(&g.device, &g.queue, &s);
        assert_eq!(up.warnings.len(), 1, "{:?}", up.warnings);
        assert!(up.warnings[0].contains("late.png"), "{}", up.warnings[0]);
        assert_eq!(g.renderer.texture_loads(), attempt);
    }
    write_png(&a, 8, [0, 255, 0, 255]);
    let up = g.renderer.upload(&g.device, &g.queue, &s);
    assert!(up.warnings.is_empty(), "{:?}", up.warnings);
    assert_eq!(g.renderer.texture_loads(), 3);
    let _ = g.renderer.upload(&g.device, &g.queue, &s);
    assert_eq!(g.renderer.texture_loads(), 3);
}

#[test]
fn scenes_from_cached_textures_render() {
    let _lock = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_dir("render");
    let a = dir.join("a.png");
    write_png(&a, 8, [255, 0, 0, 255]);
    let s = scene(vec![textured("a", &a)]);
    let g = gpu();
    let first = g.renderer.upload(&g.device, &g.queue, &s);
    let second = g.renderer.upload(&g.device, &g.queue, &s);
    // The first scene is dropped while the second still uses the shared texture.
    drop(first);
    let viewport = g.renderer.viewport(&g.device, 32, 32);
    g.renderer.render(
        &g.device,
        &g.queue,
        &viewport,
        &second,
        &camera(),
        &Style::default(),
    );
    let _ = g.device.poll(wgpu::PollType::wait_indefinitely());
    assert_eq!(g.renderer.texture_loads(), 1);
}
