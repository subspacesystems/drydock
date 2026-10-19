// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Vertices, edges and faces of one context, addressed by ids that are never reused.
//!
//! Behaviour: `docs/specs/m1-topology.md`. The `insert_*`, `delete_*` and
//! `set_vertex_position` primitives only keep references consistent; `validate` checks the
//! geometric invariants of ADR-004. Every mutation is a `Record` appended to an `Edit`, and
//! `Edit::inverse` undoes it.

use std::collections::BTreeMap;

use glam::DVec3;

use crate::geom::Plane;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VertexId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: DVec3,
}

/// An undirected edge between two vertices, stored in the order given at insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub ends: [VertexId; 2],
}

/// An edge traversed by a loop: from `ends[0]` to `ends[1]`, or the other way if `reversed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeUse {
    pub edge: EdgeId,
    pub reversed: bool,
}

impl EdgeUse {
    pub fn forward(edge: EdgeId) -> Self {
        Self {
            edge,
            reversed: false,
        }
    }

    pub fn backward(edge: EdgeId) -> Self {
        Self {
            edge,
            reversed: true,
        }
    }
}

/// A closed cycle of edge uses: each use ends where the next one starts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loop {
    pub uses: Vec<EdgeUse>,
}

/// A planar face. The outer loop runs counter-clockwise seen from the front (so it defines
/// the normal); inner loops (holes) run clockwise seen from the front.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub outer: Loop,
    pub inner: Vec<Loop>,
}

/// One primitive change. `Remove*` records carry the removed data so they can be inverted.
#[derive(Clone, Debug, PartialEq)]
pub enum Record {
    AddVertex(VertexId, Vertex),
    RemoveVertex(VertexId, Vertex),
    AddEdge(EdgeId, Edge),
    RemoveEdge(EdgeId, Edge),
    AddFace(FaceId, Face),
    RemoveFace(FaceId, Face),
    MoveVertex {
        id: VertexId,
        from: DVec3,
        to: DVec3,
    },
}

impl Record {
    /// The record that undoes this one.
    pub fn inverse(&self) -> Record {
        unimplemented!("m1-topology")
    }
}

/// An ordered list of records, applied first to last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edit {
    pub records: Vec<Record>,
}

impl Edit {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The edit that undoes this one: inverted records in reverse order.
    pub fn inverse(&self) -> Edit {
        unimplemented!("m1-topology")
    }
}

/// Why a primitive or a record could not be applied. The context is unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TopoError {
    /// An `Add*` record names an id that is present.
    IdInUse,
    MissingVertex(VertexId),
    MissingEdge(EdgeId),
    MissingFace(FaceId),
    /// An edge from a vertex to itself.
    SameVertex(VertexId),
    /// The vertex still has edges.
    VertexInUse(VertexId),
    /// The edge still borders faces.
    EdgeInUse(EdgeId),
    /// A `Remove*` or `MoveVertex` record doesn't match the stored data.
    Mismatch,
}

/// A broken invariant found by `Context::validate`. Pairs list the lower id first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Violation {
    /// A vertex no edge uses.
    StrayVertex(VertexId),
    /// Two vertices within `LINEAR_TOL`.
    DuplicateVertex(VertexId, VertexId),
    /// Two edges between the same two vertices.
    DuplicateEdge(EdgeId, EdgeId),
    /// Two edges cross, touch or overlap other than at one shared end vertex.
    EdgesIntersect(EdgeId, EdgeId),
    /// A loop has fewer than 3 uses or doesn't chain end to start.
    OpenLoop(FaceId),
    /// A loop visits a vertex twice, an edge appears twice in the face, or a loop is not a
    /// simple polygon in the face plane.
    LoopNotSimple(FaceId),
    NonPlanarFace(FaceId),
    /// An inner loop is not strictly inside the outer loop.
    HoleOutside(FaceId),
    /// Two inner loops of a face touch or nest.
    HolesTouch(FaceId),
    /// An inner loop runs the same way as the outer loop.
    HoleWinding(FaceId),
    /// Two faces whose outer loops use the same set of edges.
    DuplicateFace(FaceId, FaceId),
}

/// The geometry of one context (the model root, or the inside of a group or component).
///
/// Equality compares vertices, edges and faces only, not the id counters.
#[derive(Clone, Debug, Default)]
pub struct Context {
    vertices: BTreeMap<VertexId, Vertex>,
    edges: BTreeMap<EdgeId, Edge>,
    faces: BTreeMap<FaceId, Face>,
}

impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        self.vertices == other.vertices && self.edges == other.edges && self.faces == other.faces
    }
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    // -- queries --------------------------------------------------------------------

    pub fn vertex(&self, id: VertexId) -> Option<&Vertex> {
        self.vertices.get(&id)
    }

    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges.get(&id)
    }

    pub fn face(&self, id: FaceId) -> Option<&Face> {
        self.faces.get(&id)
    }

    /// All vertices in ascending id order.
    pub fn vertices(&self) -> impl Iterator<Item = (VertexId, &Vertex)> {
        self.vertices.iter().map(|(id, v)| (*id, v))
    }

    /// All edges in ascending id order.
    pub fn edges(&self) -> impl Iterator<Item = (EdgeId, &Edge)> {
        self.edges.iter().map(|(id, e)| (*id, e))
    }

    /// All faces in ascending id order.
    pub fn faces(&self) -> impl Iterator<Item = (FaceId, &Face)> {
        self.faces.iter().map(|(id, f)| (*id, f))
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Edges using the vertex, ascending. Empty for an unknown id.
    pub fn edges_of_vertex(&self, v: VertexId) -> Vec<EdgeId> {
        let _ = v;
        unimplemented!("m1-topology")
    }

    /// Faces whose loops use the edge, ascending, each once. Empty for an unknown id.
    pub fn faces_of_edge(&self, e: EdgeId) -> Vec<FaceId> {
        let _ = e;
        unimplemented!("m1-topology")
    }

    /// The lowest-id edge joining `a` and `b` (either order).
    pub fn edge_between(&self, a: VertexId, b: VertexId) -> Option<EdgeId> {
        let _ = (a, b);
        unimplemented!("m1-topology")
    }

    /// The vertex closest to `p` within `tol` (lowest id on a tie).
    pub fn find_vertex(&self, p: DVec3, tol: f64) -> Option<VertexId> {
        let _ = (p, tol);
        unimplemented!("m1-topology")
    }

    /// The vertex a use starts at; `None` if its edge is unknown.
    pub fn use_start(&self, u: EdgeUse) -> Option<VertexId> {
        let _ = u;
        unimplemented!("m1-topology")
    }

    /// The start vertex of each use, in order (uses of unknown edges are skipped).
    pub fn loop_vertices(&self, l: &Loop) -> Vec<VertexId> {
        let _ = l;
        unimplemented!("m1-topology")
    }

    /// Positions of `loop_vertices`.
    pub fn loop_points(&self, l: &Loop) -> Vec<DVec3> {
        let _ = l;
        unimplemented!("m1-topology")
    }

    /// `geom::fit_plane` of the outer loop's points; `None` for an unknown or degenerate face.
    pub fn face_plane(&self, f: FaceId) -> Option<Plane> {
        let _ = f;
        unimplemented!("m1-topology")
    }

    /// Every broken invariant, in a deterministic order. Empty when the context is valid.
    pub fn validate(&self) -> Vec<Violation> {
        unimplemented!("m1-topology")
    }

    // -- primitives (each appends its record to `log`) -----------------------------------

    pub fn insert_vertex(&mut self, position: DVec3, log: &mut Edit) -> VertexId {
        let _ = (position, log);
        unimplemented!("m1-topology")
    }

    pub fn insert_edge(
        &mut self,
        a: VertexId,
        b: VertexId,
        log: &mut Edit,
    ) -> Result<EdgeId, TopoError> {
        let _ = (a, b, log);
        unimplemented!("m1-topology")
    }

    pub fn insert_face(&mut self, face: Face, log: &mut Edit) -> Result<FaceId, TopoError> {
        let _ = (face, log);
        unimplemented!("m1-topology")
    }

    pub fn delete_vertex(&mut self, v: VertexId, log: &mut Edit) -> Result<(), TopoError> {
        let _ = (v, log);
        unimplemented!("m1-topology")
    }

    pub fn delete_edge(&mut self, e: EdgeId, log: &mut Edit) -> Result<(), TopoError> {
        let _ = (e, log);
        unimplemented!("m1-topology")
    }

    pub fn delete_face(&mut self, f: FaceId, log: &mut Edit) -> Result<(), TopoError> {
        let _ = (f, log);
        unimplemented!("m1-topology")
    }

    pub fn set_vertex_position(
        &mut self,
        v: VertexId,
        position: DVec3,
        log: &mut Edit,
    ) -> Result<(), TopoError> {
        let _ = (v, position, log);
        unimplemented!("m1-topology")
    }

    // -- records ------------------------------------------------------------------------

    /// Apply one record (checks references and `Mismatch`, not geometry).
    pub fn apply(&mut self, record: &Record) -> Result<(), TopoError> {
        let _ = record;
        unimplemented!("m1-topology")
    }

    /// Apply every record in order. On the first error, undo the records already applied
    /// and return it, leaving the context as it was.
    pub fn apply_edit(&mut self, edit: &Edit) -> Result<(), TopoError> {
        let _ = edit;
        unimplemented!("m1-topology")
    }
}
