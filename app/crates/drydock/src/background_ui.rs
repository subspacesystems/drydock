// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Background section of Model Info: a color for the plain background, and a checkbox
//! with a color for the sky and for the ground. Kept apart from `App` like `units_ui` so
//! tests can drive it with `egui_kittest`; the section returns the settings the user
//! changed and the app applies them through `Editor::set_background`.
//!
//! Behaviour: `docs/specs/m10-background-model-info.md`.

use drydock_kernel::background::Background;
use eframe::egui;

/// Draw the section into `ui`: a "Background" heading, a "Background:" row with a color
/// button, then a "Sky" checkbox with a color button and a "Ground" checkbox with a color
/// button (rows in that order; the checkbox labels are exactly "Sky" and "Ground"). `Some`
/// on the frame the user changes anything, holding all the settings as changed (one change
/// per frame); `None` otherwise. A color button is disabled-looking but still usable when
/// its checkbox is off (the color is kept).
pub fn background_section(ui: &mut egui::Ui, background: &Background) -> Option<Background> {
    let _ = (ui, background);
    unimplemented!()
}
