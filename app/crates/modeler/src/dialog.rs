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
    let _ = label;
    unimplemented!("m6-dialogs")
}

/// The fill of the default button under `visuals`: the selection colour, which differs from
/// an ordinary button's resting fill.
pub fn default_fill(visuals: &egui::Visuals) -> Color32 {
    let _ = visuals;
    unimplemented!("m6-dialogs")
}

/// Draw `buttons` on one line at the right edge of `ui`, left to right in the order given,
/// [`ROW_GAP`] below whatever is above. Each is at least [`BUTTON_MIN_SIZE`] with a visible
/// border; the default one is filled with [`default_fill`]. Returns the index of the button
/// clicked this frame, or of the default one when Enter is pressed and it is enabled.
/// Escape is the caller's.
pub fn button_row(ui: &mut egui::Ui, buttons: &[DialogButton<'_>]) -> Option<usize> {
    let _ = (ui, buttons);
    unimplemented!("m6-dialogs")
}
