// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The app's prompts as modal windows (`docs/specs/m11-modal-dialogs.md`): unsaved changes,
//! mesh units, 3D Text and export options. Each is a function the caller runs every frame
//! while the prompt is open; it shows the prompt with [`crate::dialog::modal_dialog_window`]
//! and returns the answer on the frame it is given. Closing without an answer (Escape, the
//! title bar's close) is Cancel.

use eframe::egui;

use crate::dialog::{self, DialogWindow};
use crate::look;
use crate::mesh_export::{ExportDialog, ExportOutcome};
use crate::mesh_import::{MeshUnit, UnitsOutcome};
use crate::text_tool::{TextDialog, TextOutcome};

/// The answer to the unsaved-changes prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

/// "Unsaved changes" (window title), the line "Save changes to {name}?" and three buttons:
/// Save (the primary one, `look::primary_button`), Discard, Cancel; 380 x 120.
pub fn unsaved_prompt(ctx: &egui::Context, name: &str) -> Option<UnsavedChoice> {
    let mut open = true;
    let mut choice = None;
    let window = DialogWindow {
        title: "Unsaved changes",
        size: egui::vec2(380.0, 120.0),
    };
    dialog::modal_dialog_window(
        ctx,
        egui::Id::new("unsaved-changes"),
        &window,
        &mut open,
        |ui| {
            ui.label(format!("Save changes to {name}?"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if look::primary_button(ui, "Save").clicked() {
                    choice = Some(UnsavedChoice::Save);
                }
                if ui.button("Discard").clicked() {
                    choice = Some(UnsavedChoice::Discard);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(UnsavedChoice::Cancel);
                }
            });
        },
    );
    if choice.is_none() && !open {
        choice = Some(UnsavedChoice::Cancel);
    }
    choice
}

/// "Mesh units": [`crate::mesh_import::units_choice`] for `file` and `unit`; 320 x 220.
pub fn units_prompt(ctx: &egui::Context, file: &str, unit: &mut MeshUnit) -> Option<UnitsOutcome> {
    let mut open = true;
    let mut outcome = None;
    let window = DialogWindow {
        title: "Mesh units",
        size: egui::vec2(320.0, 220.0),
    };
    dialog::modal_dialog_window(ctx, egui::Id::new("mesh-units"), &window, &mut open, |ui| {
        outcome = crate::mesh_import::units_choice(ui, file, unit)
    });
    if outcome.is_none() && !open {
        outcome = Some(UnitsOutcome::Cancel);
    }
    outcome
}

/// "3D Text": [`crate::text_tool::text_dialog`]; 360 x 220.
pub fn text_prompt(ctx: &egui::Context, dialog: &mut TextDialog) -> Option<TextOutcome> {
    let mut open = true;
    let mut outcome = None;
    let window = DialogWindow {
        title: "3D Text",
        size: egui::vec2(360.0, 220.0),
    };
    dialog::modal_dialog_window(
        ctx,
        egui::Id::new("text-dialog"),
        &window,
        &mut open,
        |ui| outcome = crate::text_tool::text_dialog(ui, dialog),
    );
    if outcome.is_none() && !open {
        outcome = Some(TextOutcome::Cancel);
    }
    outcome
}

/// "Export STL" / "Export 3MF" (`"Export {format label}"`): [`crate::mesh_export::export_dialog`];
/// 360 x 330.
pub fn export_prompt(
    ctx: &egui::Context,
    dialog: &mut ExportDialog,
    has_selection: bool,
) -> Option<ExportOutcome> {
    let mut open = true;
    let mut outcome = None;
    let title = format!("Export {}", dialog.options.format.label());
    let window = DialogWindow {
        title: &title,
        size: egui::vec2(360.0, 330.0),
    };
    dialog::modal_dialog_window(
        ctx,
        egui::Id::new("export-dialog"),
        &window,
        &mut open,
        |ui| outcome = crate::mesh_export::export_dialog(ui, dialog, has_selection),
    );
    if outcome.is_none() && !open {
        outcome = Some(ExportOutcome::Cancel);
    }
    outcome
}
