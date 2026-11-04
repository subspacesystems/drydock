// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Sweep paths for Follow Me: a set of edges, or a face's outer loop, turned into one ordered
//! chain of vertices that a profile can be swept along.
//!
//! Behaviour: `docs/specs/m9-sweep-path.md`. The sweep itself (side faces, mitred joints, end
//! caps) builds on this in `docs/specs/m9-sweep-face.md`.

use glam::DVec3;

use crate::topology::{Context, EdgeId, FaceId, VertexId};

/// An ordered chain of edges, open or closed.
///
/// `edges[i]` joins `vertices[i]` and `vertices[i + 1]` (for a closed path the last edge joins
/// the last vertex back to `vertices[0]`). So an open path has one more vertex than edges, and
/// a closed path has as many vertices as edges and does not repeat its first vertex.
/// `points[i]` is the position of `vertices[i]`.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepPath {
    pub vertices: Vec<VertexId>,
    pub points: Vec<DVec3>,
    pub edges: Vec<EdgeId>,
    pub closed: bool,
}

/// Why a set of edges or a face gives no path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    /// No edges were given.
    Empty,
    /// The first given edge, in input order, that the context doesn't have.
    UnknownEdge(EdgeId),
    /// The face isn't in the context.
    UnknownFace(FaceId),
    /// The lowest-id vertex where three or more of the path's edges meet.
    Branch(VertexId),
    /// The edges fall into this many separate connected pieces (two or more).
    Gap { pieces: usize },
}

impl SweepPath {
    /// The same path run the other way. An open path's vertices and edges in reverse order;
    /// a closed path keeps `vertices[0]` first and then visits the rest in reverse order.
    pub fn reversed(&self) -> SweepPath {
        unimplemented!("m9-sweep-path")
    }

    /// Total length of the path's edges, in meters.
    pub fn length(&self) -> f64 {
        unimplemented!("m9-sweep-path")
    }
}

/// Order `edges` into one path (spec: "Path from edges"). The order and repeats of `edges`
/// don't matter. Every curve with an edge in `edges` brings in all of its step edges.
pub fn path_from_edges(ctx: &Context, edges: &[EdgeId]) -> Result<SweepPath, PathError> {
    let _ = (ctx, edges);
    unimplemented!("m9-sweep-path")
}

/// The outer loop of `face` as a closed path, in the loop's own direction (counter-clockwise
/// seen from the front), starting at its lowest-id vertex (spec: "Path around a face").
pub fn face_perimeter(ctx: &Context, face: FaceId) -> Result<SweepPath, PathError> {
    let _ = (ctx, face);
    unimplemented!("m9-sweep-path")
}
