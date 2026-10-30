// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The fit check: which faces of two selected parts are closer than the clearance, and the
//! scene the viewport draws with them tinted. No window needed. See
//! `docs/specs/m7-clearance-ui.md`.

use modeler_dae::{FlatScene, Triangle, tint_fit, tint_overhangs};
use modeler_kernel::editpath::EditPath;
use modeler_kernel::model::{InstanceId, Model};
use modeler_kernel::select::Entity;
use std::borrow::Cow;

/// Smallest clearance the field accepts, in millimeters.
pub const MIN_CLEARANCE_MM: f64 = 0.01;
/// Largest clearance the field accepts, in millimeters.
pub const MAX_CLEARANCE_MM: f64 = 5.0;

/// A fit check that is showing, with what it was computed from.
#[derive(Clone, Debug, PartialEq)]
pub struct FitCheck {
    /// The two parts, in selection order (A, B).
    pub a: InstanceId,
    pub b: InstanceId,
    /// The edit path the check was run in.
    pub path: EditPath,
    /// The undo stack's state when it was run (`UndoStack::state`).
    pub undo_state: u64,
    /// The clearance used, in meters.
    pub clearance_m: f64,
    /// The flagged triangles of A, then of B, in world space.
    pub flagged: Vec<Triangle>,
    /// How many of `flagged` belong to A and to B.
    pub in_a: usize,
    pub in_b: usize,
}

/// The two instances when `selection` is exactly two `Entity::Instance` entries (different
/// ids), in selection order; else `None`.
pub fn selected_pair(_selection: &[Entity]) -> Option<(InstanceId, InstanceId)> {
    unimplemented!("m7-clearance-ui")
}

/// The clearance the text field holds, in meters: `text` (trimmed, a plain decimal number
/// in millimeters, `.` as the separator, an optional `mm` suffix) when it is within
/// `MIN_CLEARANCE_MM..=MAX_CLEARANCE_MM`; else `None`.
pub fn parse_clearance_mm(_text: &str) -> Option<f64> {
    unimplemented!("m7-clearance-ui")
}

/// `meters` as millimeters for the field and the report: up to 3 decimals, no trailing
/// zeros or point (`0.0002` -> `"0.2"`, `0.005` -> `"5"`, `0.00025` -> `"0.25"`).
pub fn format_mm(_meters: f64) -> String {
    unimplemented!("m7-clearance-ui")
}

impl FitCheck {
    /// Check the two selected parts in the open body `path`: `selected_pair(selection)`,
    /// `instance_triangles` for each with `outer` = `path.transform(model)`, `close_triangles`
    /// with `clearance_m`, and the flagged triangles of A then of B. Errors, as the message
    /// for the status bar and nothing else done:
    /// - not two instances selected: `"Select two groups or components to check their fit"`;
    /// - `path` does not resolve in `model`: `"Can't check the fit here"`.
    pub fn run(
        _model: &Model,
        _path: &EditPath,
        _selection: &[Entity],
        _undo_state: u64,
        _clearance_m: f64,
    ) -> Result<FitCheck, String> {
        unimplemented!("m7-clearance-ui")
    }

    /// The same two parts in the same path and undo state, checked again with `clearance_m`.
    /// If the parts or the path no longer resolve, a check with nothing flagged.
    pub fn recompute(&self, _model: &Model, _clearance_m: f64) -> FitCheck {
        unimplemented!("m7-clearance-ui")
    }

    /// Still true to the model on screen: the same edit `path`, the same `undo_state`, and
    /// `selection` is the same pair as `selected_pair` gives for (a, b), in either order.
    pub fn is_current(&self, _path: &EditPath, _selection: &[Entity], _undo_state: u64) -> bool {
        unimplemented!("m7-clearance-ui")
    }

    /// The report line: `"{in_a} triangles of A and {in_b} triangles of B are closer than {c} mm"`
    /// with `c = format_mm(clearance_m)`, or `"No triangles closer than {c} mm"` when both are 0.
    pub fn message(&self) -> String {
        unimplemented!("m7-clearance-ui")
    }
}

/// The scene to upload: `scene` with the overhang tint when `overhang_degrees` is `Some`
/// (`tint_overhangs`), then the fit tint when `fit` is `Some` (`tint_fit` with its flagged
/// triangles), so a face that is both shows the fit color. Neither: `scene` itself, not a copy.
pub fn display_scene<'a>(
    scene: &'a FlatScene,
    overhang_degrees: Option<f64>,
    fit: Option<&FitCheck>,
) -> Cow<'a, FlatScene> {
    if overhang_degrees.is_none() && fit.is_none() {
        return Cow::Borrowed(scene);
    }
    let overhangs = overhang_degrees.map(|degrees| tint_overhangs(scene, degrees));
    let tinted = overhangs.as_ref().unwrap_or(scene);
    match fit {
        Some(fit) => Cow::Owned(tint_fit(tinted, &fit.flagged)),
        None => Cow::Owned(tinted.clone()),
    }
}
