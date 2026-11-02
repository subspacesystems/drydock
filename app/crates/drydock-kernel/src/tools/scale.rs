// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Scale tool: grab one of the handles on the selection's bounding box, then click (or type
//! factors) to scale the selection about the opposite handle or the box's center.
//!
//! Behaviour: `docs/specs/m3-tool-scale.md`.

use glam::{DMat4, DVec3};

use crate::geom::LINEAR_TOL;
use crate::model::{BodyId, Model, UndoStack};
use crate::select::{self, Entity};
use crate::transform::{self, TransformError};
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
    let target = model.body(body)?;
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    let mut any = false;
    for id in transform::moved_vertices(target.geometry(), selection) {
        if let Some(vertex) = target.geometry().vertex(id) {
            min = min.min(vertex.position);
            max = max.max(vertex.position);
            any = true;
        }
    }
    for entity in selection {
        let Entity::Instance(id) = *entity else {
            continue;
        };
        for (a, b) in select::instance_segments(model, body, id) {
            min = min.min(a).min(b);
            max = max.max(a).max(b);
            any = true;
        }
    }
    any.then_some(Bounds { min, max })
}

fn live_axis(bounds: &Bounds, axis: usize) -> bool {
    match axis {
        0 => bounds.max.x - bounds.min.x > LINEAR_TOL,
        1 => bounds.max.y - bounds.min.y > LINEAR_TOL,
        _ => bounds.max.z - bounds.min.z > LINEAR_TOL,
    }
}

/// The handles a box offers, sorted (`Handle`'s `Ord`).
pub fn handles(bounds: &Bounds) -> Vec<Handle> {
    let mut out = Vec::new();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                if (x, y, z) == (0, 0, 0) {
                    continue;
                }
                let components = [x, y, z];
                let live = components
                    .iter()
                    .enumerate()
                    .all(|(axis, &c)| c == 0 || live_axis(bounds, axis));
                if live {
                    out.push(Handle::new(x, y, z));
                }
            }
        }
    }
    out.sort();
    out
}

/// Factors typed into the measurement box: `2`, `2,3` or `2, 3, 0.5`.
pub fn parse_factors(text: &str) -> Result<Vec<f64>, ParseError> {
    if text.trim().is_empty() {
        return Err(ParseError::Empty);
    }
    let mut out = Vec::new();
    for piece in text.split(',') {
        let piece = piece.trim();
        if piece.is_empty() {
            return Err(ParseError::Invalid);
        }
        let value: f64 = piece.parse().map_err(|_| ParseError::Invalid)?;
        if !value.is_finite() {
            return Err(ParseError::Invalid);
        }
        out.push(value);
    }
    Ok(out)
}

#[derive(Clone, Debug)]
struct Drag {
    bounds: Bounds,
    handle: Handle,
    factors: DVec3,
    cursor: Option<DVec3>,
}

#[derive(Clone, Debug)]
struct LastScale {
    fixed: DVec3,
    handle: Handle,
    uniform: bool,
    factors: DVec3,
    count: usize,
}

#[derive(Clone, Debug)]
pub struct ScaleTool {
    stage: ScaleStage,
    about_center: bool,
    invert_uniform: bool,
    drag: Option<Drag>,
    last: Option<LastScale>,
}

fn finite(p: DVec3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

fn near_one(factors: DVec3) -> bool {
    (factors.x - 1.0).abs() <= FACTOR_TOL
        && (factors.y - 1.0).abs() <= FACTOR_TOL
        && (factors.z - 1.0).abs() <= FACTOR_TOL
}

fn axes(handle: Handle) -> impl Iterator<Item = usize> {
    [handle.x, handle.y, handle.z]
        .into_iter()
        .enumerate()
        .filter_map(|(axis, c)| (c != 0).then_some(axis))
}

fn set_axis(factors: &mut DVec3, i: usize, value: f64) {
    match i {
        0 => factors.x = value,
        1 => factors.y = value,
        _ => factors.z = value,
    }
}

fn from_text(text: &str, handle: Handle, uniform: bool) -> Result<DVec3, ScaleError> {
    let values = parse_factors(text).map_err(ScaleError::Parse)?;
    let mut factors = DVec3::ONE;
    if uniform {
        if values.len() != 1 {
            return Err(ScaleError::WrongCount);
        }
        factors = DVec3::splat(values[0]);
    } else {
        let count = axes(handle).count();
        if values.len() == 1 {
            for i in axes(handle) {
                set_axis(&mut factors, i, values[0]);
            }
        } else if values.len() == count {
            for (value, i) in values.iter().zip(axes(handle)) {
                set_axis(&mut factors, i, *value);
            }
        } else {
            return Err(ScaleError::WrongCount);
        }
    }
    if [factors.x, factors.y, factors.z]
        .iter()
        .any(|v| v.abs() < MIN_FACTOR)
    {
        return Err(ScaleError::ZeroFactor);
    }
    Ok(factors)
}

impl Default for ScaleTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ScaleTool {
    pub fn new() -> Self {
        Self {
            stage: ScaleStage::Pick,
            about_center: false,
            invert_uniform: false,
            drag: None,
            last: None,
        }
    }

    pub fn stage(&self) -> ScaleStage {
        self.stage
    }

    /// Scale about the box's center instead of the opposite handle. May change mid-drag.
    pub fn set_about_center(&mut self, on: bool) {
        self.about_center = on;
        self.refresh();
    }

    /// Swap the handle's default between uniform and per-axis scaling. May change mid-drag.
    pub fn set_invert_uniform(&mut self, on: bool) {
        self.invert_uniform = on;
        self.refresh();
    }

    /// Start scaling `selection` from `handle` of its bounding box.
    pub fn grab(
        &mut self,
        handle: Handle,
        model: &Model,
        body: BodyId,
        selection: &[Entity],
    ) -> Result<(), ScaleError> {
        if selection.is_empty() {
            return Err(ScaleError::Empty);
        }
        let bounds = bounds(model, body, selection).ok_or(ScaleError::Empty)?;
        if !handles(&bounds).contains(&handle) {
            return Err(ScaleError::NoHandle);
        }
        self.stage = ScaleStage::Drag;
        self.drag = Some(Drag {
            bounds,
            handle,
            factors: DVec3::ONE,
            cursor: None,
        });
        self.last = None;
        Ok(())
    }

    /// The grabbed handle, while scaling.
    pub fn handle_grabbed(&self) -> Option<Handle> {
        self.drag().map(|drag| drag.handle)
    }

    /// Whether the current drag scales all three axes by one factor, while scaling.
    pub fn uniform(&self) -> Option<bool> {
        self.drag().map(|drag| self.is_uniform(drag))
    }

    /// The point that stays put, while scaling.
    pub fn fixed_point(&self) -> Option<DVec3> {
        self.drag().map(|drag| self.fixed(drag))
    }

    /// The factors along x, y and z the next click would apply, while scaling.
    pub fn factors(&self) -> Option<DVec3> {
        self.drag().map(|drag| drag.factors)
    }

    /// The factors of the last finished scale, while typed factors can still change it
    /// (`undo` still shows it on top).
    pub fn last_factors(&self, undo: &UndoStack) -> Option<DVec3> {
        if !self.editable(undo) {
            return None;
        }
        self.last.as_ref().map(|last| last.factors)
    }

    /// The scale the next click would apply, while scaling.
    pub fn matrix(&self) -> Option<DMat4> {
        let drag = self.drag()?;
        Some(matrix(drag.factors, self.fixed(drag)))
    }

    /// The grabbed box after the current scale, while scaling.
    pub fn preview(&self) -> Option<Bounds> {
        let drag = self.drag()?;
        let matrix = matrix(drag.factors, self.fixed(drag));
        let a = matrix.transform_point3(drag.bounds.min);
        let b = matrix.transform_point3(drag.bounds.max);
        Some(Bounds {
            min: a.min(b),
            max: a.max(b),
        })
    }

    /// Scales go through `transform::transform` on `selection`, one undo step each.
    pub fn handle(
        &mut self,
        input: Input,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), ScaleError> {
        match input {
            Input::Move(p) => self.move_cursor(p),
            Input::Click(p) => self.click(p, selection, target),
            Input::Typed(text) => self.typed(&text, selection, target),
            Input::Cancel => self.cancel(),
        }
    }

    fn drag(&self) -> Option<&Drag> {
        if self.stage == ScaleStage::Drag {
            self.drag.as_ref()
        } else {
            None
        }
    }

    fn is_uniform(&self, drag: &Drag) -> bool {
        let live = (0..3).filter(|&i| live_axis(&drag.bounds, i)).count();
        let handle_axes = axes(drag.handle).count();
        let corner = handle_axes >= 2 && handle_axes == live;
        corner != self.invert_uniform
    }

    fn fixed(&self, drag: &Drag) -> DVec3 {
        if self.about_center {
            drag.bounds.center()
        } else {
            drag.bounds.point(drag.handle.opposite())
        }
    }

    fn cursor_factors(&self, drag: &Drag, p: DVec3) -> DVec3 {
        let f = self.fixed(drag);
        let g = drag.bounds.point(drag.handle);
        let d = g - f;
        if self.is_uniform(drag) {
            let t = (p - f).dot(d) / d.dot(d);
            DVec3::splat(t)
        } else {
            let mut factors = DVec3::ONE;
            for i in axes(drag.handle) {
                let value = match i {
                    0 => (p.x - f.x) / d.x,
                    1 => (p.y - f.y) / d.y,
                    _ => (p.z - f.z) / d.z,
                };
                set_axis(&mut factors, i, value);
            }
            factors
        }
    }

    fn refresh(&mut self) {
        let Some(drag) = self.drag() else {
            return;
        };
        let Some(p) = drag.cursor else {
            return;
        };
        let factors = self.cursor_factors(drag, p);
        if let Some(drag) = self.drag.as_mut() {
            drag.factors = factors;
        }
    }

    fn editable(&self, undo: &UndoStack) -> bool {
        let Some(last) = self.last.as_ref() else {
            return false;
        };
        self.stage == ScaleStage::Pick && undo.undo_count() == last.count && undo.redo_count() == 0
    }

    fn move_cursor(&mut self, p: DVec3) -> Result<(), ScaleError> {
        if !finite(p) {
            return Ok(());
        }
        let Some(drag) = self.drag.as_ref() else {
            return Ok(());
        };
        let factors = self.cursor_factors(drag, p);
        if let Some(drag) = self.drag.as_mut() {
            drag.cursor = Some(p);
            drag.factors = factors;
        }
        Ok(())
    }

    fn click(
        &mut self,
        p: DVec3,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), ScaleError> {
        if !finite(p) {
            return Err(ScaleError::NotFinite);
        }
        let Some(drag) = self.drag.as_ref() else {
            return Ok(());
        };
        let factors = self.cursor_factors(drag, p);
        if let Some(drag) = self.drag.as_mut() {
            drag.cursor = Some(p);
            drag.factors = factors;
        }
        if near_one(factors) {
            return Ok(());
        }
        if [factors.x, factors.y, factors.z]
            .iter()
            .any(|v| v.abs() < MIN_FACTOR)
        {
            return Err(ScaleError::ZeroFactor);
        }
        self.commit(selection, target)
    }

    fn typed(
        &mut self,
        text: &str,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), ScaleError> {
        match self.stage {
            ScaleStage::Pick => self.retype_last(text, selection, target),
            ScaleStage::Drag => {
                let Some(drag) = self.drag.as_ref() else {
                    return Err(ScaleError::NotStarted);
                };
                let factors = from_text(text, drag.handle, self.is_uniform(drag))?;
                if near_one(factors) {
                    return Ok(());
                }
                if let Some(drag) = self.drag.as_mut() {
                    drag.factors = factors;
                }
                self.commit(selection, target)
            }
        }
    }

    fn retype_last(
        &mut self,
        text: &str,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), ScaleError> {
        if !self.editable(target.undo) {
            return Err(ScaleError::NotStarted);
        }
        let Some(last) = self.last.clone() else {
            return Err(ScaleError::NotStarted);
        };
        let factors = from_text(text, last.handle, last.uniform)?;
        if near_one(factors) {
            return Ok(());
        }
        if selection.is_empty() {
            return Err(ScaleError::Empty);
        }
        target
            .undo
            .undo(target.model)
            .map_err(|e| ScaleError::Transform(TransformError::Model(e)))?;
        let matrix = matrix(factors, last.fixed);
        if let Err(error) = transform::transform(
            target.model,
            target.body,
            selection,
            matrix,
            target.undo,
            "Scale",
        ) {
            let _ = target.undo.redo(target.model);
            return Err(ScaleError::Transform(error));
        }
        self.last = Some(LastScale {
            fixed: last.fixed,
            handle: last.handle,
            uniform: last.uniform,
            factors,
            count: target.undo.undo_count(),
        });
        Ok(())
    }

    fn cancel(&mut self) -> Result<(), ScaleError> {
        self.stage = ScaleStage::Pick;
        self.drag = None;
        Ok(())
    }

    fn commit(&mut self, selection: &[Entity], target: &mut Target) -> Result<(), ScaleError> {
        let Some(drag) = self.drag.as_ref() else {
            return Ok(());
        };
        let fixed = self.fixed(drag);
        let handle = drag.handle;
        let uniform = self.is_uniform(drag);
        let factors = drag.factors;
        let matrix = matrix(factors, fixed);
        transform::transform(
            target.model,
            target.body,
            selection,
            matrix,
            target.undo,
            "Scale",
        )
        .map_err(ScaleError::Transform)?;
        self.stage = ScaleStage::Pick;
        self.drag = None;
        self.last = Some(LastScale {
            fixed,
            handle,
            uniform,
            factors,
            count: target.undo.undo_count(),
        });
        Ok(())
    }
}

fn matrix(factors: DVec3, fixed: DVec3) -> DMat4 {
    DMat4::from_translation(fixed) * DMat4::from_scale(factors) * DMat4::from_translation(-fixed)
}
