// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The button row of the Open / Import file dialog, kept apart from `app` so tests can
//! drive it with `egui_kittest`. Behaviour: `docs/specs/m10-open-dialog-primary.md`.

use eframe::egui;

use crate::look;

/// What an open-file dialog is picking a file for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMode {
    /// Replace the open document.
    Open,
    /// Add the file as a component source.
    Component,
    /// Add the file's groups to the open model.
    Models,
}

/// What the user did in the button row this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenButtons {
    /// The confirming button was clicked.
    pub chosen: bool,
    /// Cancel was clicked.
    pub cancelled: bool,
}

/// The confirming button's label: "Open" for `Open`, "Import" for the other two modes.
pub fn choose_label(mode: OpenMode) -> &'static str {
    match mode {
        OpenMode::Open => "Open",
        OpenMode::Component | OpenMode::Models => "Import",
    }
}

/// Draws the row (confirming button via `look::primary_button`, then an ordinary Cancel)
/// in a horizontal layout and reports the clicks.
pub fn open_buttons(ui: &mut egui::Ui, mode: OpenMode) -> OpenButtons {
    let mut buttons = OpenButtons::default();
    ui.horizontal(|ui| {
        if look::primary_button(ui, choose_label(mode)).clicked() {
            buttons.chosen = true;
        }
        if ui.button("Cancel").clicked() {
            buttons.cancelled = true;
        }
    });
    buttons
}
