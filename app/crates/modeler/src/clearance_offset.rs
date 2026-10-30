// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Grow or shrink the selected groups and components by the clearance: what the selection
//! holds, the scale of an instance, what the status bar says. No window needed. The edit
//! itself is `Editor::offset_instances`. See `docs/specs/m7-clearance-offset-ui.md`.

use glam::DMat4;
use modeler_kernel::model::InstanceId;
use modeler_kernel::offset::OffsetError;
use modeler_kernel::select::Entity;

/// Which way the parts move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Grow,
    Shrink,
}

impl Direction {
    /// The undo step's label: "Grow by Clearance" or "Shrink by Clearance".
    pub fn label(self) -> &'static str {
        unimplemented!("m7-clearance-offset-ui")
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
        unimplemented!("m7-clearance-offset-ui")
    }
}

/// The instances in `selection` (`Entity::Instance` entries), in selection order, each once.
pub fn selected_instances(_selection: &[Entity]) -> Vec<InstanceId> {
    unimplemented!("m7-clearance-offset-ui")
}

/// The uniform scale of `world`, when it is a similarity (rotation, mirror and one scale
/// factor, plus a move): its three axis columns have the same length `s > 0` and are at
/// right angles, each within `1e-9` relative (lengths compare as `|l - s| <= 1e-9 * s`,
/// angles as `|dot| <= 1e-9 * s * s`). `None` otherwise (uneven scale, shear, zero, NaN).
pub fn uniform_scale(_world: &DMat4) -> Option<f64> {
    unimplemented!("m7-clearance-offset-ui")
}

/// The short reason a part was refused, for the status bar:
/// - `NotSolid` -> `"not a solid (try Repair)"`;
/// - `Curved` -> `"has arcs or circles"`;
/// - `Unsupported(v)` -> `"N corners where the faces don't meet"` (`"1 corner where ..."`);
/// - `Collapse` -> shrink: `"too thin for this clearance"`, grow: `"a hole or cavity would close"`;
/// - `SelfIntersect` -> `"a gap would close"`;
/// - `BadDistance` -> `"the clearance is not usable"`;
/// - `MissingBody` -> `"not found"`.
pub fn reason(_error: &OffsetError, _direction: Direction) -> String {
    unimplemented!("m7-clearance-offset-ui")
}

/// The reason for an instance whose transform is not a similarity.
pub const UNEVEN_SCALE: &str = "scaled unevenly";
