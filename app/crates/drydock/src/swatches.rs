// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Small previews of material texture images for the Materials panel, loaded off the UI
//! thread and kept by path. Behaviour: `docs/specs/m4-material-swatch-texture.md`.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use eframe::egui;

/// Longest side of a preview, pixels.
pub const SWATCH_MAX_SIDE: u32 = 64;

/// What the cache knows about one image path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwatchState {
    /// Never asked for.
    Unknown,
    /// Being read and scaled.
    Loading,
    /// Ready to draw: the preview's size in pixels.
    Ready { size: [u32; 2] },
    /// The file couldn't be read or decoded; never tried again.
    Failed(String),
}

/// Previews by image path. Cloning shares the cache. A cache never makes two
/// `MaterialsPanel`s compare unequal.
#[derive(Clone, Default)]
pub struct Swatches {
    inner: Arc<Cache>,
}

impl std::fmt::Debug for Swatches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Swatches")
    }
}

impl PartialEq for Swatches {
    fn eq(&self, _: &Swatches) -> bool {
        true
    }
}

/// One image's preview and, once uploaded, its texture.
struct Entry {
    state: SwatchState,
    preview: Option<image::RgbaImage>,
    texture: Option<egui::TextureHandle>,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<String, Entry>,
    queue: VecDeque<String>,
    worker: Option<JoinHandle<()>>,
    /// Loads wait for this before running, so a load's result can't overtake the call that
    /// queued it. The first follow-up call lets them begin.
    open: bool,
    /// Paths queued or being read.
    pending: usize,
}

#[derive(Default)]
struct Cache {
    inner: Mutex<Inner>,
    work: Condvar,
    done: Condvar,
}

impl Swatches {
    /// The preview of the image at `path` to draw this frame, starting a load on the first
    /// ask; `None` while loading or when the image can't be read.
    pub fn get(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureId> {
        let cache = Arc::clone(&self.inner);
        let mut inner = cache.inner.lock().unwrap();

        if let Some(entry) = inner.entries.get_mut(path) {
            if let Some(texture) = &entry.texture {
                return Some(texture.id());
            }
            if !matches!(entry.state, SwatchState::Ready { .. }) {
                return None;
            }
            let preview = entry.preview.take()?;
            let size = [preview.width() as usize, preview.height() as usize];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, preview.as_raw());
            let texture = ctx.load_texture(path.to_owned(), image, egui::TextureOptions::LINEAR);
            let id = texture.id();
            entry.texture = Some(texture);
            Self::open(&cache, &mut inner);
            return Some(id);
        }

        let was_running = inner.worker.is_some();
        inner.entries.insert(
            path.to_owned(),
            Entry {
                state: SwatchState::Loading,
                preview: None,
                texture: None,
            },
        );
        inner.queue.push_back(path.to_owned());
        inner.pending += 1;
        if inner.worker.is_none() {
            let worker_cache = Arc::clone(&cache);
            let ctx = ctx.clone();
            inner.worker = Some(std::thread::spawn(move || worker(worker_cache, ctx)));
        }
        if was_running {
            Self::open(&cache, &mut inner);
        }
        None
    }

    /// Where `path` stands; never starts a load.
    pub fn state(&mut self, path: &str) -> SwatchState {
        let mut inner = self.inner.inner.lock().unwrap();
        let state = inner
            .entries
            .get(path)
            .map_or(SwatchState::Unknown, |entry| entry.state.clone());
        Self::open(&self.inner, &mut inner);
        state
    }

    /// Block until every load started so far has finished (tests; the app never calls it).
    pub fn wait(&mut self) {
        let mut inner = self.inner.inner.lock().unwrap();
        Self::open(&self.inner, &mut inner);
        while inner.pending > 0 {
            inner = self.inner.done.wait(inner).unwrap();
        }
    }

    fn open(cache: &Cache, inner: &mut Inner) {
        if !inner.open {
            inner.open = true;
            cache.work.notify_all();
        }
    }
}

fn worker(cache: Arc<Cache>, ctx: egui::Context) {
    {
        let mut inner = cache.inner.lock().unwrap();
        while !inner.open {
            inner = cache.work.wait(inner).unwrap();
        }
    }
    loop {
        let path = {
            let mut inner = cache.inner.lock().unwrap();
            match inner.queue.pop_front() {
                Some(path) => path,
                None => {
                    inner.worker = None;
                    return;
                }
            }
        };
        let result = image::open(&path).map(|image| thumbnail(image.to_rgba8(), SWATCH_MAX_SIDE));
        let mut inner = cache.inner.lock().unwrap();
        if let Some(entry) = inner.entries.get_mut(&path) {
            match result {
                Ok(preview) => {
                    entry.state = SwatchState::Ready {
                        size: [preview.width(), preview.height()],
                    };
                    entry.preview = Some(preview);
                }
                Err(error) => entry.state = SwatchState::Failed(error.to_string()),
            }
        }
        inner.pending -= 1;
        cache.done.notify_all();
        drop(inner);
        ctx.request_repaint();
    }
}

/// `image` scaled to fit in `max_side` x `max_side`, aspect kept, each side at least 1;
/// an image that already fits is returned unchanged.
pub fn thumbnail(image: image::RgbaImage, max_side: u32) -> image::RgbaImage {
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    if max_side == 0 || longest <= max_side {
        return image;
    }
    let scale = max_side as f32 / longest as f32;
    let new_width = ((width as f32 * scale).round() as u32).clamp(1, max_side);
    let new_height = ((height as f32 * scale).round() as u32).clamp(1, max_side);
    image::imageops::resize(
        &image,
        new_width,
        new_height,
        image::imageops::FilterType::Triangle,
    )
}

/// The part of an image of `image_size` (pixels) that fills a swatch of `swatch_size`
/// without stretching (centered crop), as UV coordinates in 0..1.
pub fn cover_uv(image_size: [u32; 2], swatch_size: egui::Vec2) -> egui::Rect {
    let whole = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    let [width, height] = image_size;
    if width == 0
        || height == 0
        || !swatch_size.x.is_finite()
        || !swatch_size.y.is_finite()
        || swatch_size.x <= 0.0
        || swatch_size.y <= 0.0
    {
        return whole;
    }
    let image_aspect = width as f32 / height as f32;
    let swatch_aspect = swatch_size.x / swatch_size.y;
    if image_aspect > swatch_aspect {
        let fraction = swatch_aspect / image_aspect;
        let margin = (1.0 - fraction) / 2.0;
        egui::Rect::from_min_max(egui::pos2(margin, 0.0), egui::pos2(1.0 - margin, 1.0))
    } else {
        let fraction = image_aspect / swatch_aspect;
        let margin = (1.0 - fraction) / 2.0;
        egui::Rect::from_min_max(egui::pos2(0.0, margin), egui::pos2(1.0, 1.0 - margin))
    }
}
