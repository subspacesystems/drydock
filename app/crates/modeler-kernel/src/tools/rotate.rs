// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Rotate tool: click the protractor's center, click the start of the angle, click again
//! (or type an angle) to rotate the selection about the protractor's axis.
//!
//! Behaviour: `docs/specs/m3-tool-rotate.md`.

use glam::{DMat4, DVec3};

use crate::geom::LINEAR_TOL;
use crate::model::UndoStack;
use crate::select::Entity;
use crate::transform::{self, TransformError};
use crate::units::ParseError;

use super::plane::DrawPlane;
use super::{Input, Target};

/// Rotation angles snap to whole multiples of this many degrees...
pub const SNAP_STEP_DEG: f64 = 15.0;
/// ...when the cursor's angle is within this many degrees of one.
pub const SNAP_WINDOW_DEG: f64 = 2.0;
/// Angles (in degrees) this close to zero rotate nothing.
pub const ANGLE_TOL_DEG: f64 = 1e-6;

/// Why the tool refused an input. The tool, the model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum RotateError {
    /// The typed text is not an angle.
    Parse(ParseError),
    /// A typed angle needs a rotation in progress, or one just finished.
    NotStarted,
    /// Nothing is selected.
    Empty,
    /// A clicked point is not finite.
    NotFinite,
    /// The kernel refused the rotation.
    Transform(TransformError),
}

/// What the next click does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotateStage {
    /// Place the protractor's center.
    Center,
    /// Place the start of the angle.
    Start,
    /// Rotate: the next click finishes.
    Angle,
}

/// An angle typed into the measurement box, in degrees.
pub fn parse_angle(text: &str) -> Result<f64, ParseError> {
    let t = text.trim();
    let t = if let Some(t) = t.strip_suffix('°') {
        t
    } else if let Some(t) = t.strip_suffix("deg") {
        t
    } else {
        t
    };
    let t = t.trim();
    if t.is_empty() {
        return Err(ParseError::Empty);
    }
    let angle: f64 = t.parse().map_err(|_| ParseError::Invalid)?;
    if !angle.is_finite() {
        return Err(ParseError::Invalid);
    }
    Ok(angle)
}

#[derive(Clone, Debug)]
struct LastRotation {
    center: DVec3,
    axis: DVec3,
    angle: f64,
    count: usize,
}

#[derive(Clone, Debug)]
pub struct RotateTool {
    plane: DrawPlane,
    stage: RotateStage,
    center: Option<DVec3>,
    start: Option<DVec3>,
    radius: f64,
    cursor: Option<DVec3>,
    angle: Option<f64>,
    last: Option<LastRotation>,
}

fn finite(p: DVec3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

fn rotation(center: DVec3, axis: DVec3, degrees: f64) -> DMat4 {
    DMat4::from_translation(center)
        * DMat4::from_axis_angle(axis.normalize(), degrees.to_radians())
        * DMat4::from_translation(-center)
}

impl RotateTool {
    pub fn new(plane: DrawPlane) -> Self {
        Self {
            plane,
            stage: RotateStage::Center,
            center: None,
            start: None,
            radius: 0.0,
            cursor: None,
            angle: None,
            last: None,
        }
    }

    /// The protractor's plane; its normal is the rotation axis.
    pub fn plane(&self) -> DrawPlane {
        self.plane
    }

    /// Use `plane` for the next rotation. Ignored once a center is placed.
    pub fn set_plane(&mut self, plane: DrawPlane) {
        if self.stage == RotateStage::Center {
            self.plane = plane;
        }
    }

    pub fn stage(&self) -> RotateStage {
        self.stage
    }

    /// The protractor's center once placed. Use it as `Probe::anchor`.
    pub fn center(&self) -> Option<DVec3> {
        match self.stage {
            RotateStage::Center => None,
            RotateStage::Start | RotateStage::Angle => self.center,
        }
    }

    /// The angle in degrees the next click would rotate by, while rotating.
    pub fn angle(&self) -> Option<f64> {
        match self.stage {
            RotateStage::Angle => self.angle,
            RotateStage::Center | RotateStage::Start => None,
        }
    }

    /// The angle in degrees of the last finished rotation, while a typed angle can still
    /// change it (`undo` still shows it on top).
    pub fn last_angle(&self, undo: &UndoStack) -> Option<f64> {
        let last = self.last.as_ref()?;
        (self.editable(undo)).then_some(last.angle)
    }

    /// The rotation the next click would apply, while rotating.
    pub fn matrix(&self) -> Option<DMat4> {
        if self.stage != RotateStage::Angle {
            return None;
        }
        let center = self.center?;
        let angle = self.angle?;
        Some(rotation(center, self.plane.normal(), angle))
    }

    /// The protractor's arms: center to cursor while placing the start, center to start and
    /// center to the rotated start while rotating.
    pub fn preview(&self) -> Vec<(DVec3, DVec3)> {
        let Some(center) = self.center else {
            return Vec::new();
        };
        match self.stage {
            RotateStage::Center => Vec::new(),
            RotateStage::Start => {
                let Some(cursor) = self.cursor else {
                    return Vec::new();
                };
                if cursor.distance(center) <= LINEAR_TOL {
                    Vec::new()
                } else {
                    vec![(center, cursor)]
                }
            }
            RotateStage::Angle => {
                let Some(start) = self.start else {
                    return Vec::new();
                };
                let Some(angle) = self.angle else {
                    return Vec::new();
                };
                let turned =
                    rotation(DVec3::ZERO, self.plane.normal(), angle).transform_vector3(start);
                vec![
                    (center, center + start * self.radius),
                    (center, center + turned * self.radius),
                ]
            }
        }
    }

    /// Rotations go through `transform::transform` on `selection`, one undo step each.
    pub fn handle(
        &mut self,
        input: Input,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), RotateError> {
        match input {
            Input::Move(p) => self.move_cursor(p),
            Input::Click(p) => self.click(p, selection, target),
            Input::Typed(text) => self.typed(&text, selection, target),
            Input::Cancel => self.cancel(),
        }
    }

    fn editable(&self, undo: &UndoStack) -> bool {
        let Some(last) = self.last.as_ref() else {
            return false;
        };
        self.stage == RotateStage::Center
            && undo.undo_count() == last.count
            && undo.redo_count() == 0
    }

    fn arm(&self, center: DVec3, p: DVec3) -> Option<DVec3> {
        let q = self.plane.project(center, p) - center;
        (q.length() > LINEAR_TOL).then_some(q)
    }

    fn cursor_angle(&self, arm: DVec3) -> Option<f64> {
        let start = self.start?;
        let normal = self.plane.normal().normalize();
        let mut degrees = normal
            .dot(start.cross(arm))
            .atan2(start.dot(arm))
            .to_degrees();
        if degrees <= -180.0 {
            degrees = 180.0;
        }
        let nearest = (degrees / SNAP_STEP_DEG).round() * SNAP_STEP_DEG;
        if (degrees - nearest).abs() <= SNAP_WINDOW_DEG {
            degrees = nearest;
            if degrees <= -180.0 {
                degrees = 180.0;
            }
        }
        Some(degrees)
    }

    fn move_cursor(&mut self, p: DVec3) -> Result<(), RotateError> {
        if !finite(p) || self.stage == RotateStage::Center {
            return Ok(());
        }
        let Some(center) = self.center else {
            return Ok(());
        };
        match self.stage {
            RotateStage::Start => {
                self.cursor = Some(self.plane.project(center, p));
            }
            RotateStage::Angle => {
                if let Some(arm) = self.arm(center, p)
                    && let Some(angle) = self.cursor_angle(arm)
                {
                    self.angle = Some(angle);
                }
            }
            RotateStage::Center => {}
        }
        Ok(())
    }

    fn click(
        &mut self,
        p: DVec3,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), RotateError> {
        if !finite(p) {
            return Err(RotateError::NotFinite);
        }
        match self.stage {
            RotateStage::Center => {
                if selection.is_empty() {
                    return Err(RotateError::Empty);
                }
                self.stage = RotateStage::Start;
                self.center = Some(p);
                self.start = None;
                self.radius = 0.0;
                self.cursor = None;
                self.angle = None;
                self.last = None;
                Ok(())
            }
            RotateStage::Start => {
                let Some(center) = self.center else {
                    return Ok(());
                };
                let Some(arm) = self.arm(center, p) else {
                    return Ok(());
                };
                self.radius = arm.length();
                self.start = Some(arm.normalize());
                self.stage = RotateStage::Angle;
                self.cursor = None;
                self.angle = Some(0.0);
                Ok(())
            }
            RotateStage::Angle => {
                if let Some(center) = self.center
                    && let Some(arm) = self.arm(center, p)
                    && let Some(angle) = self.cursor_angle(arm)
                {
                    self.angle = Some(angle);
                }
                if self.angle.is_some_and(|a| a.abs() <= ANGLE_TOL_DEG) {
                    return Ok(());
                }
                self.commit(selection, target)
            }
        }
    }

    fn typed(
        &mut self,
        text: &str,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), RotateError> {
        match self.stage {
            RotateStage::Center => self.retype_last(text, selection, target),
            RotateStage::Start => Err(RotateError::NotStarted),
            RotateStage::Angle => {
                let value = parse_angle(text).map_err(RotateError::Parse)?;
                let current = self.angle.unwrap_or(0.0);
                let direction = if current < 0.0 { -1.0 } else { 1.0 };
                let sign = if value < 0.0 { -direction } else { direction };
                let angle = value.abs() * sign;
                if angle.abs() <= ANGLE_TOL_DEG {
                    return Ok(());
                }
                self.angle = Some(angle);
                self.commit(selection, target)
            }
        }
    }

    fn retype_last(
        &mut self,
        text: &str,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), RotateError> {
        if !self.editable(target.undo) {
            return Err(RotateError::NotStarted);
        }
        let Some(last) = self.last.clone() else {
            return Err(RotateError::NotStarted);
        };
        let value = parse_angle(text).map_err(RotateError::Parse)?;
        let direction = if last.angle < 0.0 { -1.0 } else { 1.0 };
        let sign = if value < 0.0 { -direction } else { direction };
        let angle = value.abs() * sign;
        if angle.abs() <= ANGLE_TOL_DEG {
            return Ok(());
        }
        if selection.is_empty() {
            return Err(RotateError::Empty);
        }
        target
            .undo
            .undo(target.model)
            .map_err(|e| RotateError::Transform(TransformError::Model(e)))?;
        let matrix = rotation(last.center, last.axis, angle);
        if let Err(error) = transform::transform(
            target.model,
            target.body,
            selection,
            matrix,
            target.undo,
            "Rotate",
        ) {
            let _ = target.undo.redo(target.model);
            return Err(RotateError::Transform(error));
        }
        self.last = Some(LastRotation {
            center: last.center,
            axis: last.axis,
            angle,
            count: target.undo.undo_count(),
        });
        Ok(())
    }

    fn cancel(&mut self) -> Result<(), RotateError> {
        self.stage = RotateStage::Center;
        self.center = None;
        self.start = None;
        self.radius = 0.0;
        self.cursor = None;
        self.angle = None;
        Ok(())
    }

    fn commit(&mut self, selection: &[Entity], target: &mut Target) -> Result<(), RotateError> {
        let Some(matrix) = self.matrix() else {
            return Ok(());
        };
        let Some(center) = self.center else {
            return Ok(());
        };
        let Some(angle) = self.angle else {
            return Ok(());
        };
        let axis = self.plane.normal().normalize();
        transform::transform(
            target.model,
            target.body,
            selection,
            matrix,
            target.undo,
            "Rotate",
        )
        .map_err(RotateError::Transform)?;
        self.stage = RotateStage::Center;
        self.center = None;
        self.start = None;
        self.radius = 0.0;
        self.cursor = None;
        self.angle = None;
        self.last = Some(LastRotation {
            center,
            axis,
            angle,
            count: target.undo.undo_count(),
        });
        Ok(())
    }
}
