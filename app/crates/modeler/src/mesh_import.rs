// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Opening and importing STL and 3MF files: file kinds, the units choice and the reader.
//! Behaviour: `docs/specs/m7-import-ui.md`.

use std::path::Path;

use eframe::egui;
use modeler_kernel::model::Model;

/// A triangle-mesh file format the app reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshFormat {
    Stl,
    ThreeMf,
}

/// The unit an STL file's numbers are in (STL files do not say).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshUnit {
    Millimeters,
    Centimeters,
    Meters,
    Inches,
}

impl MeshUnit {
    /// Every unit in the order the units choice lists them: mm, cm, m, in.
    pub const ALL: [MeshUnit; 4] = [
        MeshUnit::Millimeters,
        MeshUnit::Centimeters,
        MeshUnit::Meters,
        MeshUnit::Inches,
    ];

    /// Meters per unit: 0.001, 0.01, 1.0, 0.0254.
    pub fn meters(self) -> f64 {
        match self {
            MeshUnit::Millimeters => 0.001,
            MeshUnit::Centimeters => 0.01,
            MeshUnit::Meters => 1.0,
            MeshUnit::Inches => 0.0254,
        }
    }

    /// The short label the units choice shows: `mm`, `cm`, `m`, `in`.
    pub fn label(self) -> &'static str {
        match self {
            MeshUnit::Millimeters => "mm",
            MeshUnit::Centimeters => "cm",
            MeshUnit::Meters => "m",
            MeshUnit::Inches => "in",
        }
    }
}

impl Default for MeshUnit {
    /// Millimeters.
    fn default() -> MeshUnit {
        MeshUnit::Millimeters
    }
}

/// How the units choice was closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitsOutcome {
    Ok,
    Cancel,
}

/// The format of the file at `path` by its extension (`.stl`, `.3mf`, any letter case);
/// `None` for anything else.
pub fn mesh_format(path: &Path) -> Option<MeshFormat> {
    let extension = path.extension()?;
    if extension.eq_ignore_ascii_case("stl") {
        Some(MeshFormat::Stl)
    } else if extension.eq_ignore_ascii_case("3mf") {
        Some(MeshFormat::ThreeMf)
    } else {
        None
    }
}

/// True when opening `path` must ask for units first: STL files, not 3MF (which carries its
/// own) and not other files.
pub fn needs_units(path: &Path) -> bool {
    mesh_format(path) == Some(MeshFormat::Stl)
}

/// Read the STL or 3MF file at `path` into a kernel model with one group per solid. `unit`
/// applies to STL only. `Err` is a one-line message that names the file (also for a file
/// that is not STL or 3MF).
pub fn read_mesh(path: &Path, unit: MeshUnit) -> Result<Model, String> {
    let result = match mesh_format(path) {
        Some(MeshFormat::Stl) => modeler_mesh::import_stl(path, unit.meters()),
        Some(MeshFormat::ThreeMf) => modeler_mesh::import_3mf(path),
        None => Err(format!("{}: not an STL or 3MF file", path.display())),
    };
    result.map_err(|error| error.replace(['\r', '\n'], " "))
}

/// The units choice: a line naming `file`, the four units as radio buttons labelled with
/// `MeshUnit::label` (the one in `unit` selected), and `OK` and `Cancel` buttons. `None`
/// until one of those buttons is clicked.
pub fn units_choice(ui: &mut egui::Ui, file: &str, unit: &mut MeshUnit) -> Option<UnitsOutcome> {
    ui.label(format!("Choose the unit of {file}"));
    for candidate in MeshUnit::ALL {
        ui.radio_value(unit, candidate, candidate.label());
    }
    let mut outcome = None;
    ui.horizontal(|ui| {
        if ui.button("OK").clicked() {
            outcome = Some(UnitsOutcome::Ok);
        }
        if ui.button("Cancel").clicked() {
            outcome = Some(UnitsOutcome::Cancel);
        }
    });
    outcome
}
