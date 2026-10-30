// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Grow or shrink the selected groups and components by the clearance: what the selection
//! holds, the scale of an instance, what the status bar says. No window needed. The edit
//! itself is `Editor::offset_instances`. See `docs/specs/m7-clearance-offset-ui.md`.

use glam::DMat4;
use modeler_kernel::model::InstanceId;
use modeler_kernel::offset::OffsetError;
use modeler_kernel::select::Entity;

use crate::fit_check::format_mm;

/// Which way the parts move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Grow,
    Shrink,
}

impl Direction {
    /// The undo step's label: "Grow by Clearance" or "Shrink by Clearance".
    pub fn label(self) -> &'static str {
        match self {
            Direction::Grow => "Grow by Clearance",
            Direction::Shrink => "Shrink by Clearance",
        }
    }
}

/// A part that was left alone, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Refused {
    /// The instance's name; when that is empty, its definition's name; when that is empty
    /// too, "part".
    pub name: String,
    /// The short reason from `reason` (or the scale reason below).
    pub reason: String,
}

/// What `Editor::offset_instances` did.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetOutcome {
    pub direction: Direction,
    /// The clearance asked for, in meters (positive for both directions).
    pub clearance_m: f64,
    /// How many parts (distinct definitions) changed.
    pub changed: usize,
    /// The parts that did not, in selection order.
    pub refused: Vec<Refused>,
}

impl OffsetOutcome {
    /// The status bar line. `format_mm` supplies the number.
    /// - Some changed: `"Grew 2 parts by 0.2 mm"` / `"Shrank 1 part by 0.2 mm"`;
    /// - then, when some were refused, `". Not changed: "` and each refused part as
    ///   `"NAME: REASON"` joined by `"; "`;
    /// - none changed: `"Nothing changed: "` and the same list instead.
    pub fn message(&self) -> String {
        let refused = self
            .refused
            .iter()
            .map(|part| format!("{}: {}", part.name, part.reason))
            .collect::<Vec<_>>()
            .join("; ");
        if self.changed == 0 {
            return format!("Nothing changed: {refused}");
        }
        let verb = match self.direction {
            Direction::Grow => "Grew",
            Direction::Shrink => "Shrank",
        };
        let part = if self.changed == 1 { "part" } else { "parts" };
        let mut message = format!(
            "{verb} {} {part} by {} mm",
            self.changed,
            format_mm(self.clearance_m)
        );
        if !refused.is_empty() {
            message.push_str(". Not changed: ");
            message.push_str(&refused);
        }
        message
    }
}

/// The instances in `selection` (`Entity::Instance` entries), in selection order, each once.
pub fn selected_instances(selection: &[Entity]) -> Vec<InstanceId> {
    let mut seen = std::collections::BTreeSet::new();
    let mut ids = Vec::new();
    for entity in selection {
        if let Entity::Instance(id) = *entity
            && seen.insert(id)
        {
            ids.push(id);
        }
    }
    ids
}

/// The uniform scale of `world`, when it is a similarity (rotation, mirror and one scale
/// factor, plus a move): its three axis columns have the same length `s > 0` and are at
/// right angles, each within `1e-9` relative (lengths compare as `|l - s| <= 1e-9 * s`,
/// angles as `|dot| <= 1e-9 * s * s`). `None` otherwise (uneven scale, shear, zero, NaN).
pub fn uniform_scale(world: &DMat4) -> Option<f64> {
    let axes = [
        world.x_axis.truncate(),
        world.y_axis.truncate(),
        world.z_axis.truncate(),
    ];
    let s = axes[0].length();
    if !s.is_finite() || s <= 0.0 {
        return None;
    }
    let tolerance = 1e-9 * s;
    if axes
        .iter()
        .any(|axis| !axis.is_finite() || (axis.length() - s).abs() > tolerance)
    {
        return None;
    }
    let angle_tolerance = 1e-9 * s * s;
    for i in 0..axes.len() {
        for j in (i + 1)..axes.len() {
            if axes[i].dot(axes[j]).abs() > angle_tolerance {
                return None;
            }
        }
    }
    Some(s)
}

/// The short reason a part was refused, for the status bar:
/// - `NotSolid` -> `"not a solid (try Repair)"`;
/// - `Curved` -> `"has arcs or circles"`;
/// - `Unsupported(v)` -> `"N corners where the faces don't meet"` (`"1 corner where ..."`);
/// - `Collapse` -> shrink: `"too thin for this clearance"`, grow: `"a hole or cavity would close"`;
/// - `SelfIntersect` -> `"a gap would close"`;
/// - `BadDistance` -> `"the clearance is not usable"`;
/// - `MissingBody` -> `"not found"`.
pub fn reason(error: &OffsetError, direction: Direction) -> String {
    match error {
        OffsetError::NotSolid(_) => "not a solid (try Repair)".to_owned(),
        OffsetError::Curved(_) => "has arcs or circles".to_owned(),
        OffsetError::Unsupported(vertices) => {
            let count = vertices.len();
            let corner = if count == 1 { "corner" } else { "corners" };
            format!("{count} {corner} where the faces don't meet")
        }
        OffsetError::Collapse { .. } => match direction {
            Direction::Shrink => "too thin for this clearance".to_owned(),
            Direction::Grow => "a hole or cavity would close".to_owned(),
        },
        OffsetError::SelfIntersect => "a gap would close".to_owned(),
        OffsetError::BadDistance => "the clearance is not usable".to_owned(),
        OffsetError::MissingBody(_) => "not found".to_owned(),
    }
}

/// The reason for an instance whose transform is not a similarity.
pub const UNEVEN_SCALE: &str = "scaled unevenly";
