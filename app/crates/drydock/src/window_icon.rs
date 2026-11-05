// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The icon of the main window (Linux title bar, dock and task switcher; also used on X11
//! as `_NET_WM_ICON`). Our own artwork from `assets/make-icon.py`, never egui's default.
//!
//! Behaviour: `docs/specs/m9-linux-titlebar.md`.

use eframe::egui::IconData;

/// The application id: the Wayland `app_id` and X11 `WM_CLASS` of the main window, and the
/// name (without `.desktop`) of the Linux desktop entry. Reverse-DNS, ASCII letters, digits
/// and `_` or `-` within dot-separated parts, at least three parts, no part starting with
/// a digit.
pub const APP_ID: &str = "com.subspacesystems.Drydock";

/// The 256 px icon as a PNG file (`assets/drydock-256.png`). The desktop-entry task reuses
/// it to write the hicolor icons.
pub const ICON_PNG: &[u8] = include_bytes!("../assets/drydock-256.png");

/// [`ICON_PNG`] decoded to RGBA8, ready for `ViewportBuilder::with_icon`. `width == height`
/// and `rgba.len() == width * height * 4`. Decodes with the `image` crate; the PNG is
/// embedded, so a decode failure is a build defect and may panic.
pub fn window_icon() -> IconData {
    let image = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)
        .expect("embedded icon PNG must decode")
        .to_rgba8();
    let (width, height) = image.dimensions();
    IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}
