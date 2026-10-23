// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Texture Position panel: typed move, turn and scale values and a reset for the
//! texture on the selected face's side. Kept apart from `App` so tests can drive it with
//! `egui_kittest`; the panel returns the [`TextureOp`] the user asked for and the app
//! applies it through `Editor::position_texture`.
//!
//! Behaviour: `docs/specs/m4-app-texture-position-ui.md`.

use eframe::egui;

use crate::editor::TextureOp;

/// The values typed into the panel. Each is a change relative to the texture as it lies now.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureForm {
    /// Slide by this much in texture space, meters.
    pub shift: [f64; 2],
    /// Turn by this many degrees.
    pub turn: f64,
    /// Stretch by this factor per axis.
    pub scale: [f64; 2],
}

impl Default for TextureForm {
    /// The neutral values: no shift, no turn, scale 1 on both axes.
    fn default() -> TextureForm {
        TextureForm {
            shift: [0.0, 0.0],
            turn: 0.0,
            scale: [1.0, 1.0],
        }
    }
}

/// Draw the panel into `ui`. `available` is true while a textured side is selected; the
/// controls are disabled otherwise. `Some` on the frame the user asks for something.
pub fn texture_panel(
    ui: &mut egui::Ui,
    form: &mut TextureForm,
    available: bool,
) -> Option<TextureOp> {
    let _ = (ui, form, available);
    unimplemented!("m4-app-texture-position-ui")
}
