// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Exporting 3MF and STL files: the options, the export itself (a plain function from the
//! model, the selection and the options to files), the status message and the dialog.
//! Behaviour: `docs/specs/m7-export-ui.md`.

use std::path::{Path, PathBuf};

use eframe::egui;
use modeler_kernel::model::Model;
use modeler_kernel::select::Entity;
use modeler_mesh::print_mesh::{self, PrintObject};
use modeler_mesh::print_mesh::{PrintMesh, PrintOptions, PrintScope, Skipped};

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
    let text = text.trim();
    let text = text.strip_suffix("mm").unwrap_or(text).trim();
    let value: f64 = text.parse().ok()?;
    (value.is_finite() && (MIN_TOLERANCE_MM..=MAX_TOLERANCE_MM).contains(&value)).then_some(value)
}

/// Export `model` to `path`.
pub fn export(
    model: &Model,
    selection: &[Entity],
    options: &ExportOptions,
    path: &Path,
) -> Result<ExportReport, String> {
    if !options.tolerance_mm.is_finite()
        || !(MIN_TOLERANCE_MM..=MAX_TOLERANCE_MM).contains(&options.tolerance_mm)
    {
        return Err(format!(
            "Curve tolerance {} mm is outside {}..={} mm",
            options.tolerance_mm, MIN_TOLERANCE_MM, MAX_TOLERANCE_MM
        ));
    }

    let selection: Vec<_> = selection
        .iter()
        .filter_map(|entity| match entity {
            Entity::Instance(id) => Some(*id),
            _ => None,
        })
        .collect();
    let scope = if options.selection_only {
        if selection.is_empty() {
            return Err("Nothing selected".to_string());
        }
        PrintScope::Instances(&selection)
    } else {
        PrintScope::Model
    };

    let mesh = print_mesh::print_mesh(
        model,
        scope,
        &PrintOptions {
            chord_tolerance_mm: options.tolerance_mm,
        },
    );
    if !mesh
        .objects
        .iter()
        .any(|object| !object.triangles.is_empty())
    {
        let mut message = "Nothing to export".to_string();
        if !mesh.skipped.is_empty() {
            message.push_str(": ");
            message.push_str(&skipped_list(&mesh));
        }
        return Err(message);
    }

    let mut objects: Vec<PrintObject> = mesh
        .objects
        .iter()
        .filter(|object| !object.triangles.is_empty())
        .cloned()
        .collect();
    let mut report = ExportReport {
        objects: objects.len(),
        triangles: objects.iter().map(|object| object.triangles.len()).sum(),
        skipped: mesh.skipped,
        ..ExportReport::default()
    };

    match options.format {
        ExportFormat::ThreeMf => {
            let target = with_extension(path, "3mf");
            modeler_mesh::threemf_write::write(&objects, &target)
                .map_err(|err| format!("{}: {err}", target.display()))?;
            report.files = vec![target];
        }
        ExportFormat::Stl => {
            scale_objects(&mut objects, options.unit);
            if options.layout == StlLayout::OneFilePerObject {
                report.files = modeler_mesh::stl_write::write_separate(&objects, path)
                    .map_err(|err| format!("{}: {err}", path.display()))?;
            } else {
                let target = with_extension(path, "stl");
                modeler_mesh::stl_write::write_combined(&objects, &target)
                    .map_err(|err| format!("{}: {err}", target.display()))?;
                report.files = vec![target];
            }
        }
    }
    Ok(report)
}

fn skipped_list(mesh: &PrintMesh) -> String {
    mesh.skipped
        .iter()
        .map(|skipped| format!("{} {}", skipped.name, skipped.reason))
        .collect::<Vec<_>>()
        .join("; ")
}

fn with_extension(path: &Path, extension: &str) -> PathBuf {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
    {
        path.to_path_buf()
    } else {
        let mut name = path.as_os_str().to_os_string();
        name.push(".");
        name.push(extension);
        PathBuf::from(name)
    }
}

fn scale_objects(objects: &mut [PrintObject], unit: MeshUnit) {
    let scale = 1.0 / (unit.meters() * 1000.0);
    for object in objects {
        for triangle in &mut object.triangles {
            for corner in triangle {
                *corner *= scale;
            }
        }
    }
}

/// The status bar line for a finished export.
pub fn status_message(report: &ExportReport, options: &ExportOptions) -> String {
    let objects = if report.objects == 1 {
        "object"
    } else {
        "objects"
    };
    let mut message =
        if options.format == ExportFormat::Stl && options.layout == StlLayout::OneFilePerObject {
            let folder = report
                .files
                .first()
                .and_then(|file| file.parent())
                .map(file_name)
                .unwrap_or_default();
            format!(
                "Exported {} {objects} ({} triangles) as {} files in {folder}",
                report.objects,
                report.triangles,
                report.files.len()
            )
        } else {
            let file = report
                .files
                .first()
                .map(|file| file_name(file))
                .unwrap_or_default();
            format!(
                "Exported {} {objects} ({} triangles) to {file}",
                report.objects, report.triangles
            )
        };
    for skipped in &report.skipped {
        message.push_str(&format!("; skipped {} {}", skipped.name, skipped.reason));
    }
    message
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
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
    let parsed = parse_tolerance_mm(&dialog.tolerance_text);
    if let Some(tolerance) = parsed {
        dialog.options.tolerance_mm = tolerance;
    }
    if !has_selection && dialog.options.selection_only {
        dialog.options.selection_only = false;
    }

    ui.radio_value(&mut dialog.options.selection_only, false, "Whole model");
    ui.add_enabled_ui(has_selection, |ui| {
        ui.radio_value(&mut dialog.options.selection_only, true, "Selection");
    });

    if dialog.options.format == ExportFormat::Stl {
        ui.separator();
        for unit in MeshUnit::ALL {
            ui.radio_value(&mut dialog.options.unit, unit, unit.label());
        }
        ui.separator();
        ui.radio_value(
            &mut dialog.options.layout,
            StlLayout::OneFilePerObject,
            "One file per object",
        );
        ui.radio_value(
            &mut dialog.options.layout,
            StlLayout::SingleFile,
            "Single file",
        );
    }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Curve tolerance (mm)");
        ui.text_edit_singleline(&mut dialog.tolerance_text);
    });

    let mut outcome = None;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(parsed.is_some(), egui::Button::new("OK"))
            .clicked()
        {
            outcome = Some(ExportOutcome::Ok);
        }
        if ui.button("Cancel").clicked() {
            outcome = Some(ExportOutcome::Cancel);
        }
    });
    outcome
}
