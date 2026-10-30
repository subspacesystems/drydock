// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The solid check in the window: the Solid rows of Entity Info, the Repair message and the
//! problem marks in the viewport. No window needed. See `docs/specs/m7-solid-ui.md`.

use std::borrow::Cow;

use modeler_dae::{FlatScene, SolidMarks};
use modeler_kernel::editpath::EditPath;
use modeler_kernel::model::{BodyId, DefinitionId, InstanceId, Model};
use modeler_kernel::repair::RepairReport;
use modeler_kernel::select::Entity;
use modeler_kernel::solid::SolidReport;
use modeler_kernel::units::LengthFormat;

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
pub fn solid_status(_model: &Model, _body: BodyId, _selection: &[Entity]) -> Option<SolidStatus> {
    unimplemented!("m7-solid-ui")
}

/// Whether Repair has anything to work on: the report is not `is_clean`.
pub fn can_repair(_status: &SolidStatus) -> bool {
    unimplemented!("m7-solid-ui")
}

/// A volume in cubic meters as text in the cube of `format`'s unit: `mm³`, `cm³`, `m³`, `in³`
/// or `ft³`; the number to the format's decimals (2 for architectural, in `ft³`), trailing
/// zeros and point removed, no space before the suffix; 0 for a value that isn't finite.
/// `1e-6` m³ is `"1000mm³"` in millimeters and `"1cm³"` in centimeters.
pub fn format_volume(_cubic_meters: f64, _format: LengthFormat) -> String {
    unimplemented!("m7-solid-ui")
}

/// The label and value rows Entity Info shows for `status`, in order (see the spec).
pub fn status_rows(_status: &SolidStatus, _format: LengthFormat) -> Vec<(String, String)> {
    unimplemented!("m7-solid-ui")
}

/// The one-line status-bar text for a repair (see the spec).
pub fn repair_message(_report: &RepairReport) -> String {
    unimplemented!("m7-solid-ui")
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
        _model: &Model,
        _body: BodyId,
        _selection: &[Entity],
    ) -> Option<&SolidStatus> {
        unimplemented!("m7-solid-ui")
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
        _model: &Model,
        _path: &EditPath,
        _selection: &[Entity],
        _undo_state: u64,
    ) -> bool {
        unimplemented!("m7-solid-ui")
    }

    /// Drop the marks and what they were for (a new or reopened document). Returns true
    /// when marks were showing.
    pub fn clear(&mut self) -> bool {
        unimplemented!("m7-solid-ui")
    }

    /// The marks to draw, if any; never `Some` of empty marks.
    pub fn marks(&self) -> Option<&SolidMarks> {
        self.marks.as_ref()
    }
}

/// `scene` with the problem tint when `marks` is `Some` (`tint_problems`); else `scene` itself,
/// not a copy.
pub fn marked_scene<'a>(_scene: &'a FlatScene, _marks: Option<&SolidMarks>) -> Cow<'a, FlatScene> {
    unimplemented!("m7-solid-ui")
}
