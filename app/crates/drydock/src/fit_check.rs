// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The fit check: which faces of two selected parts are closer than the clearance, and the
//! scene the viewport draws with them tinted. No window needed. See
//! `docs/specs/m7-clearance-ui.md`.

use drydock_dae::{
    FlatScene, Triangle, close_triangles, instance_triangles, tint_fit, tint_overhangs,
};
use drydock_kernel::editpath::EditPath;
use drydock_kernel::model::{InstanceId, Model};
use drydock_kernel::select::Entity;
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
pub fn selected_pair(selection: &[Entity]) -> Option<(InstanceId, InstanceId)> {
    let [Entity::Instance(a), Entity::Instance(b)] = selection else {
        return None;
    };
    (a != b).then_some((*a, *b))
}

/// The clearance the text field holds, in meters: `text` (trimmed, a plain decimal number
/// in millimeters, `.` as the separator, an optional `mm` suffix) when it is within
/// `MIN_CLEARANCE_MM..=MAX_CLEARANCE_MM`; else `None`.
pub fn parse_clearance_mm(text: &str) -> Option<f64> {
    let mut number = text.trim();
    if let Some(without_suffix) = number.strip_suffix("mm") {
        number = without_suffix.trim_end();
    }
    if number.is_empty()
        || !number.chars().any(|c| c.is_ascii_digit())
        || number.chars().any(|c| c != '.' && !c.is_ascii_digit())
        || number.matches('.').count() > 1
    {
        return None;
    }
    let mm: f64 = number.parse().ok()?;
    (MIN_CLEARANCE_MM..=MAX_CLEARANCE_MM)
        .contains(&mm)
        .then_some(mm / 1000.0)
}

/// `meters` as millimeters for the field and the report: up to 3 decimals, no trailing
/// zeros or point (`0.0002` -> `"0.2"`, `0.005` -> `"5"`, `0.00025` -> `"0.25"`).
pub fn format_mm(meters: f64) -> String {
    let text = format!("{:.3}", meters * 1000.0);
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

impl FitCheck {
    /// Check the two selected parts in the open body `path`: `selected_pair(selection)`,
    /// `instance_triangles` for each with `outer` = `path.transform(model)`, `close_triangles`
    /// with `clearance_m`, and the flagged triangles of A then of B. Errors, as the message
    /// for the status bar and nothing else done:
    /// - not two instances selected: `"Select two groups or components to check their fit"`;
    /// - `path` does not resolve in `model`: `"Can't check the fit here"`.
    pub fn run(
        model: &Model,
        path: &EditPath,
        selection: &[Entity],
        undo_state: u64,
        clearance_m: f64,
    ) -> Result<FitCheck, String> {
        let (a, b) = selected_pair(selection)
            .ok_or_else(|| "Select two groups or components to check their fit".to_owned())?;
        let body = path
            .body(model)
            .ok_or_else(|| "Can't check the fit here".to_owned())?;
        let outer = path
            .transform(model)
            .ok_or_else(|| "Can't check the fit here".to_owned())?;
        let ta = instance_triangles(model, body, a, outer);
        let tb = instance_triangles(model, body, b, outer);
        let report = close_triangles(&ta, &tb, clearance_m);
        let mut flagged = Vec::with_capacity(report.a.len() + report.b.len());
        flagged.extend(report.a.iter().map(|&index| ta[index]));
        flagged.extend(report.b.iter().map(|&index| tb[index]));
        Ok(FitCheck {
            a,
            b,
            path: path.clone(),
            undo_state,
            clearance_m,
            in_a: report.a.len(),
            in_b: report.b.len(),
            flagged,
        })
    }

    /// The same two parts in the same path and undo state, checked again with `clearance_m`.
    /// If the parts or the path no longer resolve, a check with nothing flagged.
    pub fn recompute(&self, model: &Model, clearance_m: f64) -> FitCheck {
        let (Some(body), Some(outer)) = (self.path.body(model), self.path.transform(model)) else {
            return FitCheck {
                clearance_m,
                flagged: Vec::new(),
                in_a: 0,
                in_b: 0,
                ..self.clone()
            };
        };
        let ta = instance_triangles(model, body, self.a, outer);
        let tb = instance_triangles(model, body, self.b, outer);
        let report = close_triangles(&ta, &tb, clearance_m);
        let mut flagged = Vec::with_capacity(report.a.len() + report.b.len());
        flagged.extend(report.a.iter().map(|&index| ta[index]));
        flagged.extend(report.b.iter().map(|&index| tb[index]));
        FitCheck {
            clearance_m,
            flagged,
            in_a: report.a.len(),
            in_b: report.b.len(),
            ..self.clone()
        }
    }

    /// Still true to the model on screen: the same edit `path`, the same `undo_state`, and
    /// `selection` is the same pair as `selected_pair` gives for (a, b), in either order.
    pub fn is_current(&self, path: &EditPath, selection: &[Entity], undo_state: u64) -> bool {
        self.path == *path
            && self.undo_state == undo_state
            && matches!(
                selected_pair(selection),
                Some((a, b)) if (a == self.a && b == self.b) || (a == self.b && b == self.a)
            )
    }

    /// The report line: `"{in_a} triangles of A and {in_b} triangles of B are closer than {c} mm"`
    /// with `c = format_mm(clearance_m)`, or `"No triangles closer than {c} mm"` when both are 0.
    pub fn message(&self) -> String {
        let clearance = format_mm(self.clearance_m);
        if self.in_a == 0 && self.in_b == 0 {
            format!("No triangles closer than {clearance} mm")
        } else {
            format!(
                "{} triangles of A and {} triangles of B are closer than {clearance} mm",
                self.in_a, self.in_b
            )
        }
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
    let mut shown = Cow::Borrowed(scene);
    if let Some(degrees) = overhang_degrees {
        shown = Cow::Owned(tint_overhangs(shown.as_ref(), degrees));
    }
    if let Some(fit) = fit {
        shown = Cow::Owned(tint_fit(shown.as_ref(), &fit.flagged));
    }
    shown
}
