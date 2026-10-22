// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Rotate tool: click the protractor's center, click the start of the angle, click again
//! (or type an angle) to rotate the selection about the protractor's axis.
//!
//! Behaviour: `docs/specs/m3-tool-rotate.md`.

use glam::{DMat4, DVec3};

use crate::select::Entity;
use crate::transform::TransformError;
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
    let _ = text;
    unimplemented!("m3-tool-rotate")
}

#[derive(Clone, Debug)]
pub struct RotateTool {
    plane: DrawPlane,
}

impl RotateTool {
    pub fn new(plane: DrawPlane) -> Self {
        Self { plane }
    }

    /// The protractor's plane; its normal is the rotation axis.
    pub fn plane(&self) -> DrawPlane {
        self.plane
    }

    /// Use `plane` for the next rotation. Ignored once a center is placed.
    pub fn set_plane(&mut self, plane: DrawPlane) {
        let _ = plane;
        unimplemented!("m3-tool-rotate")
    }

    pub fn stage(&self) -> RotateStage {
        unimplemented!("m3-tool-rotate")
    }

    /// The protractor's center once placed. Use it as `Probe::anchor`.
    pub fn center(&self) -> Option<DVec3> {
        unimplemented!("m3-tool-rotate")
    }

    /// The angle in degrees the next click would rotate by, while rotating.
    pub fn angle(&self) -> Option<f64> {
        unimplemented!("m3-tool-rotate")
    }

    /// The angle in degrees of the last finished rotation, while a typed angle can still
    /// change it.
    pub fn last_angle(&self) -> Option<f64> {
        unimplemented!("m3-tool-rotate")
    }

    /// The rotation the next click would apply, while rotating.
    pub fn matrix(&self) -> Option<DMat4> {
        unimplemented!("m3-tool-rotate")
    }

    /// The protractor's arms: center to cursor while placing the start, center to start and
    /// center to the rotated start while rotating.
    pub fn preview(&self) -> Vec<(DVec3, DVec3)> {
        unimplemented!("m3-tool-rotate")
    }

    /// Rotations go through `transform::transform` on `selection`, one undo step each.
    pub fn handle(
        &mut self,
        input: Input,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), RotateError> {
        let _ = (input, selection, target);
        unimplemented!("m3-tool-rotate")
    }
}
