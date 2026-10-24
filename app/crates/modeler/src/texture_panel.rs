// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Texture Position panel: typed move, turn and scale values and a reset for the
//! texture on the selected face's side. Kept apart from `App` so tests can drive it with
//! `egui_kittest`; the panel returns the [`TextureOp`] the user asked for and the app
//! applies it through `Editor::position_texture`.
//!
//! Behaviour: `docs/specs/m4-app-texture-position-ui.md`.

use eframe::egui;
use glam::DVec2;

use crate::editor::TextureOp;

/// The values typed into the panel. Each is a change relative to the texture as it lies now.
#[derive(Clone, Debug)]
pub struct TextureForm {
    /// Slide by this much in texture space, meters.
    pub shift: [f64; 2],
    /// Turn by this many degrees.
    pub turn: f64,
    /// Stretch by this factor per axis.
    pub scale: [f64; 2],
}

impl PartialEq for TextureForm {
    /// Values that are both NaN compare equal, so an untouched invalid entry still reads
    /// as the same form.
    fn eq(&self, other: &TextureForm) -> bool {
        fn same(a: f64, b: f64) -> bool {
            a == b || (a.is_nan() && b.is_nan())
        }
        same(self.shift[0], other.shift[0])
            && same(self.shift[1], other.shift[1])
            && same(self.turn, other.turn)
            && same(self.scale[0], other.scale[0])
            && same(self.scale[1], other.scale[1])
    }
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

/// The button that shows or hides the texture handles in the viewport: "Position in
/// Viewport" while they are hidden (enabled only when `available`), "Done" while they show
/// (`on`, always enabled). True on the frame it is clicked.
/// Behaviour: `docs/specs/m4-app-texture-handles.md`.
pub fn handles_button(ui: &mut egui::Ui, available: bool, on: bool) -> bool {
    let label = if on {
        "Done"
    } else {
        "Position in Viewport"
    };
    ui.add_enabled(on || available, egui::Button::new(label))
        .clicked()
}

/// Draw the panel into `ui`. `available` is true while a textured side is selected; the
/// controls are disabled otherwise. `Some` on the frame the user asks for something.
pub fn texture_panel(
    ui: &mut egui::Ui,
    form: &mut TextureForm,
    available: bool,
) -> Option<TextureOp> {
    ui.heading("Texture Position");
    if !available {
        ui.label("Select a face with a texture");
    }

    let mut op = None;
    ui.add_enabled_ui(available, |ui| {
        ui.horizontal(|ui| {
            ui.label("Move X (m)");
            ui.add(egui::DragValue::new(&mut form.shift[0]).clamp_existing_to_range(false));
        });
        ui.horizontal(|ui| {
            ui.label("Move Y (m)");
            ui.add(egui::DragValue::new(&mut form.shift[1]).clamp_existing_to_range(false));
        });
        let move_valid = form.shift[0].is_finite() && form.shift[1].is_finite();
        if ui
            .add_enabled(move_valid, egui::Button::new("Move"))
            .clicked()
        {
            op = Some(TextureOp::Move(DVec2::new(form.shift[0], form.shift[1])));
            form.shift = [0.0, 0.0];
        }

        ui.horizontal(|ui| {
            ui.label("Turn (deg)");
            ui.add(egui::DragValue::new(&mut form.turn).clamp_existing_to_range(false));
        });
        if op.is_none()
            && ui
                .add_enabled(form.turn.is_finite(), egui::Button::new("Turn"))
                .clicked()
        {
            op = Some(TextureOp::Rotate {
                angle: form.turn.to_radians(),
                about: DVec2::ZERO,
            });
            form.turn = 0.0;
        }

        ui.horizontal(|ui| {
            ui.label("Scale X");
            ui.add(egui::DragValue::new(&mut form.scale[0]).clamp_existing_to_range(false));
        });
        ui.horizontal(|ui| {
            ui.label("Scale Y");
            ui.add(egui::DragValue::new(&mut form.scale[1]).clamp_existing_to_range(false));
        });
        let scale_valid = form.scale[0].is_finite()
            && form.scale[0] > 0.0
            && form.scale[1].is_finite()
            && form.scale[1] > 0.0;
        if op.is_none()
            && ui
                .add_enabled(scale_valid, egui::Button::new("Scale"))
                .clicked()
        {
            op = Some(TextureOp::Scale {
                factor: DVec2::new(form.scale[0], form.scale[1]),
                about: DVec2::ZERO,
            });
            form.scale = [1.0, 1.0];
        }

        if op.is_none() && ui.button("Reset Texture").clicked() {
            op = Some(TextureOp::Reset);
        }
    });
    op
}
