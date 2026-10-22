// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Scale tool: grab one of the handles on the selection's bounding box, then click (or type
//! factors) to scale the selection about the opposite handle or the box's center.
//!
//! Behaviour: `docs/specs/m3-tool-scale.md`.

use glam::{DMat4, DVec3};

use crate::model::{BodyId, Model, UndoStack};
use crate::select::Entity;
use crate::transform::TransformError;
use crate::units::ParseError;

use super::{Input, Target};

/// Factors this close to 1 on every axis scale nothing.
pub const FACTOR_TOL: f64 = 1e-9;
/// Factors smaller than this (in absolute value) would squash the selection flat.
pub const MIN_FACTOR: f64 = 1e-3;

/// Why the tool refused an input. The tool, the model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum ScaleError {
    /// The typed text is not a list of factors.
    Parse(ParseError),
    /// The typed list has the wrong number of factors for the grabbed handle.
    WrongCount,
    /// A factor is smaller than `MIN_FACTOR` in absolute value.
    ZeroFactor,
    /// Typed factors need a scale in progress, or one just finished.
    NotStarted,
    /// Nothing is selected, or the selection has no points.
    Empty,
    /// The handle is not one of `handles(bounds)`.
    NoHandle,
    /// A clicked point is not finite.
    NotFinite,
    /// The kernel refused the scale.
    Transform(TransformError),
}

/// What the tool is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleStage {
    /// Waiting for a handle to be grabbed.
    Pick,
    /// Scaling: the next click finishes.
    Drag,
}

/// The axis-aligned box around a selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: DVec3,
    pub max: DVec3,
}

impl Bounds {
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }

    /// The point of `handle` on this box: on each axis the min side for -1, the middle for 0,
    /// the max side for 1.
    pub fn point(&self, handle: Handle) -> DVec3 {
        let c = self.center();
        let half = (self.max - self.min) * 0.5;
        c + DVec3::new(
            half.x * f64::from(handle.x),
            half.y * f64::from(handle.y),
            half.z * f64::from(handle.z),
        )
    }
}

/// A handle on a bounding box: each component is -1, 0 or 1 (see `Bounds::point`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Handle {
    pub x: i8,
    pub y: i8,
    pub z: i8,
}

impl Handle {
    pub const fn new(x: i8, y: i8, z: i8) -> Self {
        Self { x, y, z }
    }

    /// The handle on the other side of the box's center.
    pub const fn opposite(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

/// The box around every point `transform` would move for `selection` in `body` (see
/// `transform::moved_vertices`) and every segment end of each selected instance (see
/// `select::instance_segments`). `None` when there are no such points.
pub fn bounds(model: &Model, body: BodyId, selection: &[Entity]) -> Option<Bounds> {
    let _ = (model, body, selection);
    unimplemented!("m3-tool-scale")
}

/// The handles a box offers, sorted (`Handle`'s `Ord`).
pub fn handles(bounds: &Bounds) -> Vec<Handle> {
    let _ = bounds;
    unimplemented!("m3-tool-scale")
}

/// Factors typed into the measurement box: `2`, `2,3` or `2, 3, 0.5`.
pub fn parse_factors(text: &str) -> Result<Vec<f64>, ParseError> {
    let _ = text;
    unimplemented!("m3-tool-scale")
}

#[derive(Clone, Debug)]
pub struct ScaleTool {}

impl Default for ScaleTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ScaleTool {
    pub fn new() -> Self {
        unimplemented!("m3-tool-scale")
    }

    pub fn stage(&self) -> ScaleStage {
        unimplemented!("m3-tool-scale")
    }

    /// Scale about the box's center instead of the opposite handle. May change mid-drag.
    pub fn set_about_center(&mut self, on: bool) {
        let _ = on;
        unimplemented!("m3-tool-scale")
    }

    /// Swap the handle's default between uniform and per-axis scaling. May change mid-drag.
    pub fn set_invert_uniform(&mut self, on: bool) {
        let _ = on;
        unimplemented!("m3-tool-scale")
    }

    /// Start scaling `selection` from `handle` of its bounding box.
    pub fn grab(
        &mut self,
        handle: Handle,
        model: &Model,
        body: BodyId,
        selection: &[Entity],
    ) -> Result<(), ScaleError> {
        let _ = (handle, model, body, selection);
        unimplemented!("m3-tool-scale")
    }

    /// The grabbed handle, while scaling.
    pub fn handle_grabbed(&self) -> Option<Handle> {
        unimplemented!("m3-tool-scale")
    }

    /// Whether the current drag scales all three axes by one factor, while scaling.
    pub fn uniform(&self) -> Option<bool> {
        unimplemented!("m3-tool-scale")
    }

    /// The point that stays put, while scaling.
    pub fn fixed_point(&self) -> Option<DVec3> {
        unimplemented!("m3-tool-scale")
    }

    /// The factors along x, y and z the next click would apply, while scaling.
    pub fn factors(&self) -> Option<DVec3> {
        unimplemented!("m3-tool-scale")
    }

    /// The factors of the last finished scale, while typed factors can still change it
    /// (`undo` still shows it on top).
    pub fn last_factors(&self, undo: &UndoStack) -> Option<DVec3> {
        let _ = undo;
        unimplemented!("m3-tool-scale")
    }

    /// The scale the next click would apply, while scaling.
    pub fn matrix(&self) -> Option<DMat4> {
        unimplemented!("m3-tool-scale")
    }

    /// The grabbed box after the current scale, while scaling.
    pub fn preview(&self) -> Option<Bounds> {
        unimplemented!("m3-tool-scale")
    }

    /// Scales go through `transform::transform` on `selection`, one undo step each.
    pub fn handle(
        &mut self,
        input: Input,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), ScaleError> {
        let _ = (input, selection, target);
        unimplemented!("m3-tool-scale")
    }
}
