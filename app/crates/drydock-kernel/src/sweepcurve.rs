// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Curves carried through a sweep: a profile's arcs and circles reappear on the sections the
//! sweep builds when those are still circles, and a profile swept along an arc of the path
//! leaves an arc through each profile vertex.
//!
//! Behaviour: `docs/specs/m9-sweep-curves.md`. `sweepface::sweep_face` reads a `CurveCarry`
//! before it changes the body and calls `add_curves` once the sides are built and merged.

use crate::curve::Curve;
use crate::sweep::SweepPath;
use crate::topology::{Context, CurveId, Edit, FaceId, VertexId};

/// What a sweep needs to carry curves, read from the body before the sweep changes it
/// (spec: "Reading the curves").
#[derive(Clone, Debug, PartialEq)]
pub struct CurveCarry {
    /// The profile's vertices, loop by loop (outer first, then each hole in the face's
    /// order), each loop as `Context::loop_vertices` gives it: the order of the sections'
    /// points.
    pub profile: Vec<Vec<VertexId>>,
    /// The profile's curves, ascending id.
    pub curves: Vec<Curve>,
    /// One entry per sweep step, in sweep order (an open sweep's step `s` runs from section
    /// `s` to `s + 1`; a ring's last step runs from its last section back to section 0): the
    /// path curve that step runs along, if any.
    pub steps: Vec<Option<CurveId>>,
    /// The path is closed (the sweep is a ring).
    pub closed: bool,
}

impl CurveCarry {
    /// Read the profile's curves and the path's curves from `ctx`, the body before the sweep.
    /// `at[s]` is the index into `path.points` of section `s`, in sweep order.
    pub fn new(ctx: &Context, profile: FaceId, path: &SweepPath, at: &[usize]) -> CurveCarry {
        let _ = (ctx, profile, path, at);
        unimplemented!("m9-sweep-curves")
    }

    /// Add the carried curves to `ctx` (spec: "Section curves", "Curves along the path").
    /// `sections[s][l][p]` is the vertex the sweep put at section `s` for the profile's point
    /// `p` of loop `l` (the same indexing as `profile`). Returns the new curves' ids in the
    /// order they were added.
    pub fn add_curves(
        &self,
        ctx: &mut Context,
        sections: &[Vec<Vec<VertexId>>],
        log: &mut Edit,
    ) -> Vec<CurveId> {
        let _ = (ctx, sections, log);
        unimplemented!("m9-sweep-curves")
    }
}
