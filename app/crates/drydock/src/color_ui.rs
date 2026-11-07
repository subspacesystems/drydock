// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Color buttons for colors the model stores as sRGB components 0..1. egui's
//! `color_edit_button_rgb` reads its floats as linear, which shows such a color too light;
//! these helpers edit through `color_edit_button_srgb` on 8-bit values instead.
//!
//! Behaviour: `docs/specs/m10-color-picker-srgb.md`.

use eframe::egui;

/// Stored sRGB components (0..1) to 8-bit: `round(c * 255)`, clamped to 0..=255.
pub fn srgb_to_bytes(color: [f32; 3]) -> [u8; 3] {
    let _ = color;
    unimplemented!("m10-color-picker-srgb")
}

/// 8-bit sRGB to stored components: `byte / 255`.
pub fn bytes_to_srgb(bytes: [u8; 3]) -> [f32; 3] {
    let _ = bytes;
    unimplemented!("m10-color-picker-srgb")
}

/// A color button editing the stored sRGB `color`. Returns true, and updates `color`, only
/// when the 8-bit triple the user picked differs from the one shown; otherwise `color`
/// is left exactly as it was (no drift for colors that are not exact multiples of 1/255).
pub fn srgb_color_button(ui: &mut egui::Ui, color: &mut [f32; 3]) -> bool {
    let _ = (ui, color);
    unimplemented!("m10-color-picker-srgb")
}
