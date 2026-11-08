// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The app's prompts as modal windows (`docs/specs/m11-modal-dialogs.md`): unsaved changes,
//! mesh units, 3D Text and export options. Each is a function the caller runs every frame
//! while the prompt is open; it shows the prompt with [`crate::dialog::modal_dialog_window`]
//! and returns the answer on the frame it is given. Closing without an answer (Escape, the
//! title bar's close) is Cancel.

use eframe::egui;

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
    let _ = (ctx, name);
    unimplemented!("m11-modal-dialogs")
}

/// "Mesh units": [`crate::mesh_import::units_choice`] for `file` and `unit`; 320 x 220.
pub fn units_prompt(ctx: &egui::Context, file: &str, unit: &mut MeshUnit) -> Option<UnitsOutcome> {
    let _ = (ctx, file, unit);
    unimplemented!("m11-modal-dialogs")
}

/// "3D Text": [`crate::text_tool::text_dialog`]; 360 x 220.
pub fn text_prompt(ctx: &egui::Context, dialog: &mut TextDialog) -> Option<TextOutcome> {
    let _ = (ctx, dialog);
    unimplemented!("m11-modal-dialogs")
}

/// "Export STL" / "Export 3MF" (`"Export {format label}"`): [`crate::mesh_export::export_dialog`];
/// 360 x 330.
pub fn export_prompt(
    ctx: &egui::Context,
    dialog: &mut ExportDialog,
    has_selection: bool,
) -> Option<ExportOutcome> {
    let _ = (ctx, dialog, has_selection);
    unimplemented!("m11-modal-dialogs")
}
