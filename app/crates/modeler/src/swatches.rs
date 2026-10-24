// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Small previews of material texture images for the Materials panel, loaded off the UI
//! thread and kept by path. Behaviour: `docs/specs/m4-material-swatch-texture.md`.

use std::sync::Arc;

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
    inner: Arc<()>,
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

impl Swatches {
    /// The preview of the image at `path` to draw this frame, starting a load on the first
    /// ask; `None` while loading or when the image can't be read.
    pub fn get(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureId> {
        let _ = (&self.inner, ctx, path);
        unimplemented!("m4-material-swatch-texture")
    }

    /// Where `path` stands; never starts a load.
    pub fn state(&mut self, path: &str) -> SwatchState {
        let _ = path;
        unimplemented!("m4-material-swatch-texture")
    }

    /// Block until every load started so far has finished (tests; the app never calls it).
    pub fn wait(&mut self) {
        unimplemented!("m4-material-swatch-texture")
    }
}

/// `image` scaled to fit in `max_side` x `max_side`, aspect kept, each side at least 1;
/// an image that already fits is returned unchanged.
pub fn thumbnail(image: image::RgbaImage, max_side: u32) -> image::RgbaImage {
    let _ = (image, max_side);
    unimplemented!("m4-material-swatch-texture")
}

/// The part of an image of `image_size` (pixels) that fills a swatch of `swatch_size`
/// without stretching (centered crop), as UV coordinates in 0..1.
pub fn cover_uv(image_size: [u32; 2], swatch_size: egui::Vec2) -> egui::Rect {
    let _ = (image_size, swatch_size);
    unimplemented!("m4-material-swatch-texture")
}
