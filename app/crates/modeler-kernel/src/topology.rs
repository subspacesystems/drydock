// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Vertices, edges and faces of one context, addressed by ids that are never reused.
//!
//! Behaviour: `docs/specs/m1-topology.md`. The `insert_*`, `delete_*` and
//! `set_vertex_position` primitives only keep references consistent; `validate` checks the
//! geometric invariants of ADR-004. Every mutation is a `Record` appended to an `Edit`, and
//! `Edit::inverse` undoes it.

use std::collections::{BTreeMap, BTreeSet};

use glam::DVec3;

use crate::geom::{self, Containment, Plane, SegmentHit};

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
        match self {
            Record::AddVertex(id, v) => Record::RemoveVertex(*id, *v),
            Record::RemoveVertex(id, v) => Record::AddVertex(*id, *v),
            Record::AddEdge(id, e) => Record::RemoveEdge(*id, *e),
            Record::RemoveEdge(id, e) => Record::AddEdge(*id, *e),
            Record::AddFace(id, f) => Record::RemoveFace(*id, f.clone()),
            Record::RemoveFace(id, f) => Record::AddFace(*id, f.clone()),
            Record::MoveVertex { id, from, to } => Record::MoveVertex {
                id: *id,
                from: *to,
                to: *from,
            },
        }
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
        Edit {
            records: self.records.iter().rev().map(Record::inverse).collect(),
        }
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
    next_vertex: u64,
    next_edge: u64,
    next_face: u64,
    vertex_edges: BTreeMap<VertexId, BTreeSet<EdgeId>>,
    edge_faces: BTreeMap<EdgeId, BTreeSet<FaceId>>,
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
        self.vertex_edges
            .get(&v)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Faces whose loops use the edge, ascending, each once. Empty for an unknown id.
    pub fn faces_of_edge(&self, e: EdgeId) -> Vec<FaceId> {
        self.edge_faces
            .get(&e)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// The lowest-id edge joining `a` and `b` (either order).
    pub fn edge_between(&self, a: VertexId, b: VertexId) -> Option<EdgeId> {
        self.edges
            .iter()
            .find(|(_, e)| (e.ends[0] == a && e.ends[1] == b) || (e.ends[0] == b && e.ends[1] == a))
            .map(|(id, _)| *id)
    }

    /// The vertex closest to `p` within `tol` (lowest id on a tie).
    pub fn find_vertex(&self, p: DVec3, tol: f64) -> Option<VertexId> {
        let mut best: Option<(f64, VertexId)> = None;
        for (id, v) in &self.vertices {
            let d = v.position.distance(p);
            if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, *id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// The vertex a use starts at; `None` if its edge is unknown.
    pub fn use_start(&self, u: EdgeUse) -> Option<VertexId> {
        let e = self.edges.get(&u.edge)?;
        Some(if u.reversed { e.ends[1] } else { e.ends[0] })
    }

    /// The vertex a use ends at; `None` if its edge is unknown.
    fn use_end(&self, u: EdgeUse) -> Option<VertexId> {
        let e = self.edges.get(&u.edge)?;
        Some(if u.reversed { e.ends[0] } else { e.ends[1] })
    }

    /// The start vertex of each use, in order (uses of unknown edges are skipped).
    pub fn loop_vertices(&self, l: &Loop) -> Vec<VertexId> {
        l.uses.iter().filter_map(|u| self.use_start(*u)).collect()
    }

    /// Positions of `loop_vertices`.
    pub fn loop_points(&self, l: &Loop) -> Vec<DVec3> {
        self.loop_vertices(l)
            .into_iter()
            .filter_map(|v| self.vertices.get(&v).map(|v| v.position))
            .collect()
    }

    /// `geom::fit_plane` of the outer loop's points; `None` for an unknown or degenerate face.
    pub fn face_plane(&self, f: FaceId) -> Option<Plane> {
        let face = self.faces.get(&f)?;
        geom::fit_plane(&self.loop_points(&face.outer))
    }

    fn loop_points_of(&self, l: &Loop) -> Vec<DVec3> {
        self.loop_points(l)
    }

    /// Every broken invariant, in a deterministic order. Empty when the context is valid.
    pub fn validate(&self) -> Vec<Violation> {
        let mut out = Vec::new();

        // 1. Stray vertices.
        for v in self.vertices.keys() {
            if self.edges_of_vertex(*v).is_empty() {
                out.push(Violation::StrayVertex(*v));
            }
        }

        // 2. Duplicate vertices.
        let vids: Vec<_> = self.vertices.keys().copied().collect();
        for i in 0..vids.len() {
            for j in i + 1..vids.len() {
                let a = &self.vertices[&vids[i]];
                let b = &self.vertices[&vids[j]];
                if a.position.distance(b.position) <= geom::LINEAR_TOL {
                    out.push(Violation::DuplicateVertex(vids[i], vids[j]));
                }
            }
        }

        // 3. Duplicate edges.
        let mut duplicate_pairs = BTreeSet::new();
        let eids: Vec<_> = self.edges.keys().copied().collect();
        let mut duplicate = Vec::new();
        for i in 0..eids.len() {
            for j in i + 1..eids.len() {
                let a = &self.edges[&eids[i]];
                let b = &self.edges[&eids[j]];
                let same = (a.ends[0] == b.ends[0] && a.ends[1] == b.ends[1])
                    || (a.ends[0] == b.ends[1] && a.ends[1] == b.ends[0]);
                if same {
                    duplicate_pairs.insert((eids[i], eids[j]));
                    duplicate.push(Violation::DuplicateEdge(eids[i], eids[j]));
                }
            }
        }
        out.extend(duplicate);

        // 4. Edges that cross, touch or overlap.
        for i in 0..eids.len() {
            for j in i + 1..eids.len() {
                if duplicate_pairs.contains(&(eids[i], eids[j])) {
                    continue;
                }
                let ea = &self.edges[&eids[i]];
                let eb = &self.edges[&eids[j]];
                let (Some(a0), Some(a1), Some(b0), Some(b1)) = (
                    self.vertices.get(&ea.ends[0]).map(|v| v.position),
                    self.vertices.get(&ea.ends[1]).map(|v| v.position),
                    self.vertices.get(&eb.ends[0]).map(|v| v.position),
                    self.vertices.get(&eb.ends[1]).map(|v| v.position),
                ) else {
                    continue;
                };
                let shared: Vec<VertexId> = ea
                    .ends
                    .iter()
                    .copied()
                    .filter(|v| eb.ends.contains(v))
                    .collect();
                let hit = geom::segment_intersection(a0, a1, b0, b1, geom::LINEAR_TOL);
                let bad = match shared.as_slice() {
                    [only] => {
                        let expected_a = if ea.ends[0] == *only { 0.0 } else { 1.0 };
                        let expected_b = if eb.ends[0] == *only { 0.0 } else { 1.0 };
                        match hit {
                            SegmentHit::None => false,
                            SegmentHit::Point { ta, tb } => !(ta == expected_a && tb == expected_b),
                            SegmentHit::Overlap { .. } => true,
                        }
                    }
                    [] => !matches!(hit, SegmentHit::None),
                    _ => false,
                };
                if bad {
                    out.push(Violation::EdgesIntersect(eids[i], eids[j]));
                }
            }
        }

        // 5. Face checks, at most one violation each.
        for (f, face) in &self.faces {
            if let Some(v) = self.check_face(*f, face) {
                out.push(v);
            }
        }

        // 6. Duplicate faces: same set of outer-loop edges.
        let fids: Vec<_> = self.faces.keys().copied().collect();
        let sets: Vec<BTreeSet<EdgeId>> = fids
            .iter()
            .map(|f| self.faces[f].outer.uses.iter().map(|u| u.edge).collect())
            .collect();
        for i in 0..fids.len() {
            for j in i + 1..fids.len() {
                if sets[i] == sets[j] {
                    out.push(Violation::DuplicateFace(fids[i], fids[j]));
                }
            }
        }

        out
    }

    /// Pairs of coplanar faces whose areas overlap, lower id first, sorted. Faces may share
    /// edges and vertices. Not part of `validate`: imported models can hold overlapping
    /// decals. Behaviour: `docs/specs/m1-sticky-faces.md`.
    pub fn face_overlaps(&self) -> Vec<(FaceId, FaceId)> {
        unimplemented!("m1-sticky-faces")
    }

    /// The face check `validate` would report for this face (open or repeated loop, not
    /// planar, not simple, hole winding, hole outside, holes touching), or `None` when the
    /// face passes or is unknown. Doesn't look at other faces or edges.
    pub fn face_violation(&self, f: FaceId) -> Option<Violation> {
        self.faces.get(&f).and_then(|face| self.check_face(f, face))
    }

    fn check_face(&self, f: FaceId, face: &Face) -> Option<Violation> {
        let loops = std::iter::once(&face.outer).chain(face.inner.iter());

        // 5.1 Open loops.
        for l in loops.clone() {
            if l.uses.len() < 3 {
                return Some(Violation::OpenLoop(f));
            }
            for i in 0..l.uses.len() {
                let end = self.use_end(l.uses[i]);
                let start = self.use_start(l.uses[(i + 1) % l.uses.len()]);
                match (end, start) {
                    (Some(a), Some(b)) if a == b => {}
                    _ => return Some(Violation::OpenLoop(f)),
                }
            }
        }

        // 5.2 Repeated vertices or edges.
        let mut seen_edges = BTreeSet::new();
        for l in loops.clone() {
            let mut seen_vertices = BTreeSet::new();
            for v in self.loop_vertices(l) {
                if !seen_vertices.insert(v) {
                    return Some(Violation::LoopNotSimple(f));
                }
            }
            for u in &l.uses {
                if !seen_edges.insert(u.edge) {
                    return Some(Violation::LoopNotSimple(f));
                }
            }
        }

        // 5.3 Planarity.
        let Some(plane) = self.face_plane(f) else {
            return Some(Violation::NonPlanarFace(f));
        };
        for l in loops.clone() {
            for p in self.loop_points_of(l) {
                if plane.signed_distance(p).abs() > geom::LINEAR_TOL {
                    return Some(Violation::NonPlanarFace(f));
                }
            }
        }

        // 5.4 Simple polygons in the face plane.
        for l in loops.clone() {
            let pts = self.loop_points_of(l);
            let projected = geom::to_plane_2d(&pts, plane.normal);
            if !geom::polygon_is_simple_2d(&projected, geom::LINEAR_TOL) {
                return Some(Violation::LoopNotSimple(f));
            }
        }

        // 5.5 Hole winding.
        let outer_pts = self.loop_points_of(&face.outer);
        let outer_2d = geom::to_plane_2d(&outer_pts, plane.normal);
        let mut inner_2d = Vec::with_capacity(face.inner.len());
        for l in &face.inner {
            let pts = self.loop_points_of(l);
            let p2d = geom::to_plane_2d(&pts, plane.normal);
            if geom::signed_area_2d(&p2d) >= 0.0 {
                return Some(Violation::HoleWinding(f));
            }
            inner_2d.push(p2d);
        }

        // 5.6 Holes inside the outer loop.
        for p2d in &inner_2d {
            for p in p2d {
                if geom::point_in_polygon_2d(*p, &outer_2d, geom::LINEAR_TOL) != Containment::Inside
                {
                    return Some(Violation::HoleOutside(f));
                }
            }
        }

        // 5.7 Holes separate from each other.
        for i in 0..inner_2d.len() {
            for j in i + 1..inner_2d.len() {
                let a = &inner_2d[i];
                let b = &inner_2d[j];
                if a.iter().any(|p| {
                    geom::point_in_polygon_2d(*p, b, geom::LINEAR_TOL) != Containment::Outside
                }) || b.iter().any(|p| {
                    geom::point_in_polygon_2d(*p, a, geom::LINEAR_TOL) != Containment::Outside
                }) {
                    return Some(Violation::HolesTouch(f));
                }
            }
        }

        None
    }

    // -- primitives (each appends its record to `log`) -----------------------------------

    pub fn insert_vertex(&mut self, position: DVec3, log: &mut Edit) -> VertexId {
        let id = VertexId(self.next_vertex);
        let record = Record::AddVertex(id, Vertex { position });
        if self.apply(&record).is_ok() {
            log.records.push(record);
        }
        id
    }

    pub fn insert_edge(
        &mut self,
        a: VertexId,
        b: VertexId,
        log: &mut Edit,
    ) -> Result<EdgeId, TopoError> {
        let id = EdgeId(self.next_edge);
        let record = Record::AddEdge(id, Edge { ends: [a, b] });
        self.apply(&record)?;
        log.records.push(record);
        Ok(id)
    }

    pub fn insert_face(&mut self, face: Face, log: &mut Edit) -> Result<FaceId, TopoError> {
        let id = FaceId(self.next_face);
        let record = Record::AddFace(id, face);
        self.apply(&record)?;
        log.records.push(record);
        Ok(id)
    }

    pub fn delete_vertex(&mut self, v: VertexId, log: &mut Edit) -> Result<(), TopoError> {
        let data = *self.vertices.get(&v).ok_or(TopoError::MissingVertex(v))?;
        let record = Record::RemoveVertex(v, data);
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    pub fn delete_edge(&mut self, e: EdgeId, log: &mut Edit) -> Result<(), TopoError> {
        let data = *self.edges.get(&e).ok_or(TopoError::MissingEdge(e))?;
        let record = Record::RemoveEdge(e, data);
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    pub fn delete_face(&mut self, f: FaceId, log: &mut Edit) -> Result<(), TopoError> {
        let data = self
            .faces
            .get(&f)
            .cloned()
            .ok_or(TopoError::MissingFace(f))?;
        let record = Record::RemoveFace(f, data);
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    pub fn set_vertex_position(
        &mut self,
        v: VertexId,
        position: DVec3,
        log: &mut Edit,
    ) -> Result<(), TopoError> {
        let current = self
            .vertices
            .get(&v)
            .ok_or(TopoError::MissingVertex(v))?
            .position;
        let record = Record::MoveVertex {
            id: v,
            from: current,
            to: position,
        };
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    // -- records ------------------------------------------------------------------------

    /// Apply one record (checks references and `Mismatch`, not geometry).
    pub fn apply(&mut self, record: &Record) -> Result<(), TopoError> {
        match record {
            Record::AddVertex(id, data) => {
                if self.vertices.contains_key(id) {
                    return Err(TopoError::IdInUse);
                }
                self.vertices.insert(*id, *data);
                self.next_vertex = self.next_vertex.max(id.0.saturating_add(1));
                Ok(())
            }
            Record::RemoveVertex(id, data) => {
                let Some(stored) = self.vertices.get(id) else {
                    return Err(TopoError::MissingVertex(*id));
                };
                if stored != data {
                    return Err(TopoError::Mismatch);
                }
                if !self.edges_of_vertex(*id).is_empty() {
                    return Err(TopoError::VertexInUse(*id));
                }
                self.vertices.remove(id);
                self.vertex_edges.remove(id);
                Ok(())
            }
            Record::AddEdge(id, data) => {
                if self.edges.contains_key(id) {
                    return Err(TopoError::IdInUse);
                }
                for end in data.ends {
                    if !self.vertices.contains_key(&end) {
                        return Err(TopoError::MissingVertex(end));
                    }
                }
                if data.ends[0] == data.ends[1] {
                    return Err(TopoError::SameVertex(data.ends[0]));
                }
                self.edges.insert(*id, *data);
                for end in data.ends {
                    self.vertex_edges.entry(end).or_default().insert(*id);
                }
                self.next_edge = self.next_edge.max(id.0.saturating_add(1));
                Ok(())
            }
            Record::RemoveEdge(id, data) => {
                let Some(stored) = self.edges.get(id) else {
                    return Err(TopoError::MissingEdge(*id));
                };
                if stored != data {
                    return Err(TopoError::Mismatch);
                }
                if !self.faces_of_edge(*id).is_empty() {
                    return Err(TopoError::EdgeInUse(*id));
                }
                self.edges.remove(id);
                for end in data.ends {
                    if let Some(set) = self.vertex_edges.get_mut(&end) {
                        set.remove(id);
                    }
                    if self.vertex_edges.get(&end).is_some_and(BTreeSet::is_empty) {
                        self.vertex_edges.remove(&end);
                    }
                }
                self.edge_faces.remove(id);
                Ok(())
            }
            Record::AddFace(id, face) => {
                if self.faces.contains_key(id) {
                    return Err(TopoError::IdInUse);
                }
                for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                    for u in &l.uses {
                        if !self.edges.contains_key(&u.edge) {
                            return Err(TopoError::MissingEdge(u.edge));
                        }
                    }
                }
                self.faces.insert(*id, face.clone());
                for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                    for u in &l.uses {
                        self.edge_faces.entry(u.edge).or_default().insert(*id);
                    }
                }
                self.next_face = self.next_face.max(id.0.saturating_add(1));
                Ok(())
            }
            Record::RemoveFace(id, data) => {
                let Some(stored) = self.faces.get(id) else {
                    return Err(TopoError::MissingFace(*id));
                };
                if stored != data {
                    return Err(TopoError::Mismatch);
                }
                self.faces.remove(id);
                for l in std::iter::once(&data.outer).chain(data.inner.iter()) {
                    for u in &l.uses {
                        if let Some(set) = self.edge_faces.get_mut(&u.edge) {
                            set.remove(id);
                        }
                        if self.edge_faces.get(&u.edge).is_some_and(BTreeSet::is_empty) {
                            self.edge_faces.remove(&u.edge);
                        }
                    }
                }
                Ok(())
            }
            Record::MoveVertex { id, from, to } => {
                let Some(stored) = self.vertices.get(id) else {
                    return Err(TopoError::MissingVertex(*id));
                };
                if stored.position != *from {
                    return Err(TopoError::Mismatch);
                }
                if let Some(v) = self.vertices.get_mut(id) {
                    v.position = *to;
                }
                Ok(())
            }
        }
    }

    /// Apply every record in order. On the first error, undo the records already applied
    /// and return it, leaving the context as it was.
    pub fn apply_edit(&mut self, edit: &Edit) -> Result<(), TopoError> {
        for (applied, record) in edit.records.iter().enumerate() {
            if let Err(error) = self.apply(record) {
                for done in edit.records[..applied].iter().rev() {
                    let _ = self.apply(&done.inverse());
                }
                return Err(error);
            }
        }
        Ok(())
    }
}
