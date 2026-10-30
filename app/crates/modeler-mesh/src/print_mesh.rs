// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Print mesh: the solid groups and components of a model as world-space triangles in
//! millimeters, ready for the STL and 3MF writers.
//!
//! Behaviour: `docs/specs/m7-print-mesh.md`.

use glam::DVec3;
use modeler_kernel::model::{InstanceId, Model};
use modeler_kernel::solid::SolidReport;

/// One triangle, corners counter-clockwise seen from outside the solid, in millimeters.
pub type Triangle = [DVec3; 3];

/// Default chord tolerance in millimeters.
pub const DEFAULT_CHORD_TOLERANCE_MM: f64 = 0.01;

/// What to export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintScope<'a> {
    /// Every group and component placed in the model root.
    Model,
    /// Only these instances of the model root (with everything nested in them). Ids that
    /// aren't instances of the root are ignored.
    Instances(&'a [InstanceId]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintOptions {
    /// How far a re-segmented arc or circle may stray from the true curve, in millimeters.
    pub chord_tolerance_mm: f64,
}

impl Default for PrintOptions {
    fn default() -> Self {
        PrintOptions {
            chord_tolerance_mm: DEFAULT_CHORD_TOLERANCE_MM,
        }
    }
}

/// One exported solid.
#[derive(Clone, Debug, PartialEq)]
pub struct PrintObject {
    pub name: String,
    /// sRGB, straight alpha; `None` when nothing in the object is painted.
    pub color: Option<[f32; 4]>,
    pub triangles: Vec<Triangle>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SkipReason {
    /// No faces and nothing nested in it.
    NoGeometry,
    /// The solid check found problems; the report says which.
    NotSolid(SolidReport),
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unimplemented!("m7-print-mesh")
    }
}

/// A group or component that was left out, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Skipped {
    pub name: String,
    pub reason: SkipReason,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrintMesh {
    pub objects: Vec<PrintObject>,
    pub skipped: Vec<Skipped>,
}

/// The print mesh of `model` for `scope`.
pub fn print_mesh(_model: &Model, _scope: PrintScope, _options: &PrintOptions) -> PrintMesh {
    unimplemented!("m7-print-mesh")
}
