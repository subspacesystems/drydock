// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The solid check in the window: the Solid rows of Entity Info, the Repair message and the
//! problem marks in the viewport. No window needed. See `docs/specs/m7-solid-ui.md`.

use std::borrow::Cow;

use drydock_dae::{FlatScene, SolidMarks, solid_marks, tint_problems};
use drydock_kernel::editpath::EditPath;
use drydock_kernel::model::{BodyId, DefinitionId, InstanceId, Model};
use drydock_kernel::repair::{RepairReport, Unfixed};
use drydock_kernel::select::Entity;
use drydock_kernel::solid::{SolidReport, check_body};
use drydock_kernel::units::{LengthFormat, Unit, meters_per};

/// The solid check of the one selected group or component.
#[derive(Clone, Debug, PartialEq)]
pub struct SolidStatus {
    pub instance: InstanceId,
    pub definition: DefinitionId,
    /// `check_body` of the definition's body.
    pub report: SolidReport,
}

/// The status when `selection` is exactly one entry, an instance of `body` whose definition
/// has a body; else `None`.
pub fn solid_status(model: &Model, body: BodyId, selection: &[Entity]) -> Option<SolidStatus> {
    let [Entity::Instance(id)] = selection else {
        return None;
    };
    let instance = model.body(body)?.instance(*id)?;
    let report = check_body(model, BodyId::Definition(instance.definition))?;
    Some(SolidStatus {
        instance: *id,
        definition: instance.definition,
        report,
    })
}

/// Whether Repair has anything to work on: the report is not `is_clean`.
pub fn can_repair(status: &SolidStatus) -> bool {
    !status.report.is_clean()
}

fn trim_number(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

/// A volume in cubic meters as text in the cube of `format`'s unit: `mm³`, `cm³`, `m³`, `in³`
/// or `ft³`; the number to the format's decimals (2 for architectural, in `ft³`), trailing
/// zeros and point removed, no space before the suffix; 0 for a value that isn't finite.
/// `1e-6` m³ is `"1000mm³"` in millimeters and `"1cm³"` in centimeters.
pub fn format_volume(cubic_meters: f64, format: LengthFormat) -> String {
    let cubic_meters = if cubic_meters.is_finite() {
        cubic_meters
    } else {
        0.0
    };
    let (unit, decimals) = match format {
        LengthFormat::Decimal { unit, decimals } => (unit, decimals),
        LengthFormat::Architectural { .. } => (Unit::Foot, 2),
    };
    let suffix = match unit {
        Unit::Millimeter => "mm³",
        Unit::Centimeter => "cm³",
        Unit::Meter => "m³",
        Unit::Inch => "in³",
        Unit::Foot => "ft³",
    };
    let meters = meters_per(unit);
    let value = cubic_meters / (meters * meters * meters);
    let d = decimals.min(LengthFormat::MAX_DECIMALS) as usize;
    format!("{}{suffix}", trim_number(value, d))
}

/// The label and value rows Entity Info shows for `status`, in order (see the spec).
pub fn status_rows(status: &SolidStatus, format: LengthFormat) -> Vec<(String, String)> {
    let report = &status.report;
    let mut rows = vec![(
        "Solid".to_string(),
        if report.is_solid() { "Yes" } else { "No" }.to_string(),
    )];
    if let Some(volume) = report.volume {
        rows.push(("Volume".to_string(), format_volume(volume, format)));
    }
    let counts = [
        ("Open edges", report.open_edges.len()),
        ("Loose edges", report.loose_edges.len()),
        ("Shared edges", report.shared_edges.len()),
        ("Reversed faces", report.reversed_faces.len()),
        ("Internal faces", report.internal_faces.len()),
        ("Nested", report.nested_instances),
    ];
    for (label, count) in counts {
        if count > 0 {
            rows.push((label.to_string(), count.to_string()));
        }
    }
    if report.shells != 1 {
        rows.push(("Shells".to_string(), report.shells.to_string()));
    }
    rows
}

fn count(n: usize, one: &str, many: &str) -> Option<String> {
    if n == 0 {
        None
    } else if n == 1 {
        Some(format!("1 {one}"))
    } else {
        Some(format!("{n} {many}"))
    }
}

/// The one-line status-bar text for a repair (see the spec).
pub fn repair_message(report: &RepairReport) -> String {
    let mut text = if report.changed() {
        let mut parts = Vec::new();
        if let Some(part) = count(
            report.removed_internal.len() + report.removed_stray.len(),
            "face",
            "faces",
        ) {
            parts.push(format!("removed {part}"));
        }
        if let Some(part) = count(report.removed_edges.len(), "edge", "edges") {
            parts.push(format!("removed {part}"));
        }
        if let Some(part) = count(report.filled.len(), "hole", "holes") {
            parts.push(format!("filled {part}"));
        }
        if let Some(part) = count(report.reversed.len(), "face", "faces") {
            parts.push(format!("turned {part}"));
        }
        format!("Repaired: {}", parts.join(", "))
    } else if report.unfixed.is_empty() {
        "Nothing to repair".to_string()
    } else {
        "Nothing could be repaired".to_string()
    };

    if report.unfixed.is_empty() {
        return text;
    }

    let mut not_one_loop = 0;
    let mut not_flat = 0;
    let mut nested = 0;
    let mut bad_fill = 0;
    let mut covered = 0;
    let mut shared = 0;
    let mut nested_instances = 0;
    for unfixed in &report.unfixed {
        match unfixed {
            Unfixed::NotOneLoop(_) => not_one_loop += 1,
            Unfixed::NotFlat(_) => not_flat += 1,
            Unfixed::Nested(_) => nested += 1,
            Unfixed::BadFill(_) => bad_fill += 1,
            Unfixed::Covered(_) => covered += 1,
            Unfixed::SharedEdge(_) => shared += 1,
            Unfixed::NestedInstances(n) => nested_instances += n,
        }
    }
    let kinds = [
        ("open borders not forming one loop", not_one_loop),
        ("holes that aren't flat", not_flat),
        ("ring-shaped holes", nested),
        ("fills that aren't valid faces", bad_fill),
        ("fills that would overlap a face", covered),
        ("edges used by three or more faces", shared),
        ("groups or components inside", nested_instances),
    ];
    let parts: Vec<String> = kinds
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(label, n)| format!("{label}: {n}"))
        .collect();
    text.push_str("; could not fix: ");
    text.push_str(&parts.join(", "));
    text
}

/// The last `solid_status`, reused while the model (`Model::cache_key`), the body and the
/// selection are the same (`docs/specs/m6-panel-speed.md`: the check is not redone every
/// frame).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidCache {
    key: Option<((u64, u64), BodyId, Vec<Entity>)>,
    status: Option<SolidStatus>,
    computed: u64,
}

impl SolidCache {
    /// `solid_status(model, body, selection)`, computed only when one of them changed since
    /// the last call.
    pub fn get(
        &mut self,
        model: &Model,
        body: BodyId,
        selection: &[Entity],
    ) -> Option<&SolidStatus> {
        let key = (model.cache_key(), body, selection.to_vec());
        if !self.key.as_ref().is_some_and(|current| *current == key) {
            self.status = solid_status(model, body, selection);
            self.key = Some(key);
            self.computed += 1;
        }
        self.status.as_ref()
    }

    /// How many times `get` computed a status.
    pub fn computed(&self) -> u64 {
        self.computed
    }
}

/// Which problem marks the viewport shows, and what they were computed for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarkTracker {
    key: Option<(EditPath, Vec<Entity>, u64)>,
    marks: Option<SolidMarks>,
}

impl MarkTracker {
    /// Bring the marks up to date with the edit `path`, the `selection` and the undo stack's
    /// `undo_state`. When all three are what the last call saw, nothing is redone. Otherwise
    /// the marks become `solid_marks` of the selected instance (`solid_status` in the body
    /// `path` opens, placed by `path.transform`), or none when the selection isn't one
    /// instance, the path doesn't resolve or the marks are empty. Returns true when the marks
    /// shown are now different from before, so the scene must be uploaded again.
    pub fn update(
        &mut self,
        model: &Model,
        path: &EditPath,
        selection: &[Entity],
        undo_state: u64,
    ) -> bool {
        let key = (path.clone(), selection.to_vec(), undo_state);
        if self.key.as_ref().is_some_and(|current| *current == key) {
            return false;
        }
        let marks = (|| {
            let body = path.body(model)?;
            let outer = path.transform(model)?;
            let status = solid_status(model, body, selection)?;
            let marks = solid_marks(model, body, status.instance, outer);
            (!marks.is_empty()).then_some(marks)
        })();
        let changed = self.marks != marks;
        self.marks = marks;
        self.key = Some(key);
        changed
    }

    /// Drop the marks and what they were for (a new or reopened document). Returns true
    /// when marks were showing.
    pub fn clear(&mut self) -> bool {
        let had = self.marks.is_some();
        self.marks = None;
        self.key = None;
        had
    }

    /// The marks to draw, if any; never `Some` of empty marks.
    pub fn marks(&self) -> Option<&SolidMarks> {
        self.marks.as_ref()
    }
}

/// One `Primitive::Line` in `Style::Problem` per entry of `marks.segments`, same order, ends
/// as given (already world space). Empty marks give an empty list.
pub fn mark_primitives(marks: &SolidMarks) -> Vec<crate::overlay::Primitive> {
    marks
        .segments
        .iter()
        .map(|&[a, b]| crate::overlay::Primitive::Line {
            a,
            b,
            style: crate::overlay::Style::Problem,
        })
        .collect()
}

/// `scene` with the problem tint when `marks` is `Some` (`tint_problems`); else `scene` itself,
/// not a copy.
pub fn marked_scene<'a>(scene: &'a FlatScene, marks: Option<&SolidMarks>) -> Cow<'a, FlatScene> {
    match marks {
        Some(marks) if !marks.is_empty() => Cow::Owned(tint_problems(scene, marks)),
        _ => Cow::Borrowed(scene),
    }
}
