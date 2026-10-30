// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Exporting 3MF and STL files: the options, the export itself (a plain function from the
//! model, the selection and the options to files), the status message and the dialog.
//! Behaviour: `docs/specs/m7-export-ui.md`.

use std::path::{Path, PathBuf};

use eframe::egui;
use modeler_kernel::model::Model;
use modeler_kernel::select::Entity;
use modeler_mesh::print_mesh::Skipped;

use crate::mesh_import::MeshUnit;

/// Smallest curve tolerance the field accepts, in millimeters.
pub const MIN_TOLERANCE_MM: f64 = 0.001;
/// Largest curve tolerance the field accepts, in millimeters.
pub const MAX_TOLERANCE_MM: f64 = 1.0;

/// A file format the app writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    ThreeMf,
    Stl,
}

impl ExportFormat {
    /// The file extension without the dot: `3mf` or `stl`.
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::ThreeMf => "3mf",
            ExportFormat::Stl => "stl",
        }
    }

    /// The name used in titles: `3MF` or `STL`.
    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::ThreeMf => "3MF",
            ExportFormat::Stl => "STL",
        }
    }
}

/// How an STL export is split into files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StlLayout {
    /// One `.stl` per object, in a folder.
    OneFilePerObject,
    /// All objects in one `.stl`.
    SingleFile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportOptions {
    pub format: ExportFormat,
    /// Only the selected groups and components (else the whole model).
    pub selection_only: bool,
    /// The unit the numbers of an STL are written in. Ignored for 3MF, which is always
    /// written in millimeters and says so.
    pub unit: MeshUnit,
    /// Chord tolerance for arcs and circles, in millimeters.
    pub tolerance_mm: f64,
    /// Ignored for 3MF.
    pub layout: StlLayout,
}

impl ExportOptions {
    /// Whole model, millimeters, `print_mesh::DEFAULT_CHORD_TOLERANCE_MM`, a single file.
    pub fn new(format: ExportFormat) -> ExportOptions {
        ExportOptions {
            format,
            selection_only: false,
            unit: MeshUnit::Millimeters,
            tolerance_mm: modeler_mesh::print_mesh::DEFAULT_CHORD_TOLERANCE_MM,
            layout: StlLayout::SingleFile,
        }
    }
}

/// What an export wrote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExportReport {
    /// The files written, in object order.
    pub files: Vec<PathBuf>,
    /// Objects written.
    pub objects: usize,
    /// Triangles written.
    pub triangles: usize,
    /// Groups and components left out, with the reason.
    pub skipped: Vec<Skipped>,
}

/// The tolerance field's text as millimeters: a number with an optional trailing `mm`
/// (spaces around either are fine), finite and within `MIN_TOLERANCE_MM..=MAX_TOLERANCE_MM`.
/// `None` for anything else.
pub fn parse_tolerance_mm(text: &str) -> Option<f64> {
    let _ = text;
    unimplemented!("m7-export-ui")
}

/// Export `model` to `path`.
pub fn export(
    model: &Model,
    selection: &[Entity],
    options: &ExportOptions,
    path: &Path,
) -> Result<ExportReport, String> {
    let _ = (model, selection, options, path);
    unimplemented!("m7-export-ui")
}

/// The status bar line for a finished export.
pub fn status_message(report: &ExportReport, options: &ExportOptions) -> String {
    let _ = (report, options);
    unimplemented!("m7-export-ui")
}

/// The export dialog's state.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportDialog {
    pub options: ExportOptions,
    /// The text in the tolerance field.
    pub tolerance_text: String,
}

impl ExportDialog {
    /// A dialog showing `options`.
    pub fn new(options: ExportOptions) -> ExportDialog {
        ExportDialog {
            tolerance_text: format!("{}", options.tolerance_mm),
            options,
        }
    }
}

/// How the dialog was closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportOutcome {
    Ok,
    Cancel,
}

/// The export dialog's contents. `has_selection` says whether a group or component is
/// selected. `None` until `OK` or `Cancel` is clicked.
pub fn export_dialog(
    ui: &mut egui::Ui,
    dialog: &mut ExportDialog,
    has_selection: bool,
) -> Option<ExportOutcome> {
    let _ = (ui, dialog, has_selection);
    unimplemented!("m7-export-ui")
}
