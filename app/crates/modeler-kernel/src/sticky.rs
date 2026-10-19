// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Sticky edges: an edge drawn into a context joins the geometry already there.
//!
//! Behaviour: `docs/specs/m1-sticky-edges.md`. Every change goes through the `topology`
//! primitives, so it is recorded in the caller's `Edit` and can be undone.

use glam::DVec3;

use crate::topology::{Context, EdgeId, Edit, VertexId};

/// Why a sticky operation did nothing. The context and the log are unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StickyError {
    /// A coordinate is NaN or infinite.
    NotFinite,
    /// The two ends are the same point (within `LINEAR_TOL`, or snapped to one vertex).
    Degenerate,
    MissingEdge(EdgeId),
}

/// The edges that cover a drawn segment, in order from its first point to its last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeChain {
    /// `edges.len() + 1` vertices: `edges[i]` joins `vertices[i]` and `vertices[i + 1]`.
    pub vertices: Vec<VertexId>,
    pub edges: Vec<EdgeId>,
}

impl Context {
    /// Draw the segment `a`-`b`: snap its ends to nearby vertices or edges, split it and the
    /// existing edges where they cross, touch or overlap, and reuse edges it runs along.
    pub fn add_edge(
        &mut self,
        a: DVec3,
        b: DVec3,
        log: &mut Edit,
    ) -> Result<EdgeChain, StickyError> {
        let _ = (a, b, log);
        unimplemented!("m1-sticky-edges")
    }

    /// Split `e` at the point on it closest to `p`. Returns the vertex there: a new one, or
    /// an end of `e` (and no change) when that point is within `LINEAR_TOL` of the end.
    /// Faces using `e` keep their ids and get both pieces in their loop.
    pub fn split_edge(
        &mut self,
        e: EdgeId,
        p: DVec3,
        log: &mut Edit,
    ) -> Result<VertexId, StickyError> {
        let _ = (e, p, log);
        unimplemented!("m1-sticky-edges")
    }

    /// Erase `e`: faces bounded by it go, a hole it borders closes, and end vertices left
    /// without edges go.
    pub fn erase_edge(&mut self, e: EdgeId, log: &mut Edit) -> Result<(), StickyError> {
        let _ = (e, log);
        unimplemented!("m1-sticky-edges")
    }
}
