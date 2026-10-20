// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Inference: where the cursor's point lands while a drawing tool runs, what it snapped to,
//! and what entity is under the cursor.
//!
//! Behaviour: `docs/specs/m2-inference.md`. Works on one context, in its own coordinates;
//! instances are not entered.

use crate::select::Entity;
use crate::topology::{Context, EdgeId, FaceId, VertexId};
use glam::{DVec2, DVec3};

/// The cursor, the view and what the running tool knows. Points are in the context's
/// coordinates.
pub struct Probe<'a> {
    /// Cursor, screen pixels.
    pub cursor: DVec2,
    /// Start of the ray through the cursor (the eye for a perspective view).
    pub origin: DVec3,
    /// Direction of the ray through the cursor. Need not be unit length.
    pub dir: DVec3,
    /// True for a parallel view: every pixel's ray has direction `dir`.
    pub parallel: bool,
    /// Maps a point to screen pixels; `None` when it can't be seen.
    pub project: &'a dyn Fn(DVec3) -> Option<DVec2>,
    /// Snap radius, pixels.
    pub radius: f64,
    /// The tool's last placed point, for axis inference and the direction lock.
    pub anchor: Option<DVec3>,
    /// Axis directions in red, green, blue order (unit length, orthogonal).
    pub axes: [DVec3; 3],
    /// Extra points that snap as centers (arc and circle centers known to the tool).
    pub centers: &'a [DVec3],
    /// Direction lock: the result stays on the line through `anchor` along this direction.
    pub lock: Option<DVec3>,
}

/// What the inferred point snapped to, highest priority first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Snap {
    Endpoint(VertexId),
    Midpoint(EdgeId),
    /// Index into `Probe::centers`.
    Center(usize),
    OnEdge(EdgeId),
    /// Index into `Probe::axes`.
    OnAxis(usize),
    OnFace(FaceId),
    /// On the ground plane, or the horizontal plane through the anchor.
    Ground,
    /// On the lock line, nothing else snapped.
    Along,
}

/// The point a tool should use, and the hint to draw for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inference {
    pub point: DVec3,
    pub snap: Snap,
    /// Index into `Probe::axes` when `point - anchor` runs along that axis.
    pub axis: Option<usize>,
}

/// The nearest face the ray from `origin` along `dir` passes through, and the distance to
/// it along the unit ray.
pub fn ray_face(ctx: &Context, origin: DVec3, dir: DVec3) -> Option<(FaceId, f64)> {
    let _ = (ctx, origin, dir);
    unimplemented!()
}

/// The inferred point for the cursor.
pub fn infer(ctx: &Context, probe: &Probe) -> Option<Inference> {
    let _ = (ctx, probe);
    unimplemented!()
}

/// The entity under the cursor: a visible edge within the snap radius, else the face the
/// ray hits first.
pub fn pick(ctx: &Context, probe: &Probe) -> Option<Entity> {
    let _ = (ctx, probe);
    unimplemented!()
}
