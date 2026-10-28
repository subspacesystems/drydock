// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Shared dialog pieces after the Windows guidelines (`docs/specs/m6-dialogs.md`): text box
//! labels that end with a colon, and a row of buttons at the bottom right with a default
//! button that Enter presses.

use eframe::egui::{self, Color32, Vec2};

/// Smallest size of a dialog button, in points.
pub const BUTTON_MIN_SIZE: Vec2 = Vec2::new(88.0, 28.0);

/// Space above a button row, in points.
pub const ROW_GAP: f32 = 12.0;

/// One button of a [`button_row`].
#[derive(Clone, Copy, Debug)]
pub struct DialogButton<'a> {
    pub label: &'a str,
    /// Highlighted, and pressed by Enter. At most one button of a row.
    pub default: bool,
    pub enabled: bool,
}

/// `label` with trailing whitespace removed and a ":" at the end (unchanged if it already
/// ends with one).
pub fn field_label(label: &str) -> String {
    let label = label.trim_end();
    if label.ends_with(':') {
        label.to_owned()
    } else {
        format!("{label}:")
    }
}

/// The fill of the default button under `visuals`: the selection colour, which differs from
/// an ordinary button's resting fill.
pub fn default_fill(visuals: &egui::Visuals) -> Color32 {
    visuals.selection.bg_fill
}

/// Draw `buttons` on one line at the right edge of `ui`, left to right in the order given,
/// [`ROW_GAP`] below whatever is above. Each is at least [`BUTTON_MIN_SIZE`] with a visible
/// border; the default one is filled with [`default_fill`]. Returns the index of the button
/// clicked this frame, or of the default one when Enter is pressed and it is enabled.
/// Escape is the caller's.
pub fn button_row(ui: &mut egui::Ui, buttons: &[DialogButton<'_>]) -> Option<usize> {
    ui.add_space(ROW_GAP);
    let count = buttons.len() as f32;
    let spacing = ui.spacing().item_spacing.x;
    let available = ui.available_width();
    let needed = count * BUTTON_MIN_SIZE.x + (count - 1.0).max(0.0) * spacing;
    let width = if needed <= available || count == 0.0 {
        BUTTON_MIN_SIZE.x
    } else {
        ((available - (count - 1.0) * spacing) / count - 0.5).max(1.0)
    };
    let min_size = egui::vec2(width, BUTTON_MIN_SIZE.y);
    let mut clicked = None;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        for (index, button) in buttons.iter().enumerate().rev() {
            let mut widget = egui::Button::new(button.label).min_size(min_size);
            if button.default {
                widget = widget.fill(default_fill(ui.visuals()));
            }
            if ui.add_enabled(button.enabled, widget).clicked() {
                clicked = Some(index);
            }
        }
    });
    if clicked.is_some() {
        return clicked;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        buttons
            .iter()
            .position(|button| button.default && button.enabled)
    } else {
        None
    }
}
