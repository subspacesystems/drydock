// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Sticky edges: an edge drawn into a context joins the geometry already there.
//!
//! Behaviour: `docs/specs/m1-sticky-edges.md`. Every change goes through the `topology`
//! primitives, so it is recorded in the caller's `Edit` and can be undone.

use std::collections::BTreeSet;

use glam::DVec3;

use crate::geom::{self, LINEAR_TOL, SegmentHit};
use crate::topology::{
    Context, Edge, EdgeId, EdgeUse, Edit, Face, FaceId, Loop, Record, TopoError, VertexId,
};

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

fn finite(p: DVec3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

fn closest_on_segment(p: DVec3, a: DVec3, b: DVec3) -> DVec3 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 == 0.0 {
        return a;
    }
    a + ab * ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
}

fn topo_error(error: TopoError) -> StickyError {
    match error {
        TopoError::MissingEdge(e) => StickyError::MissingEdge(e),
        _ => StickyError::Degenerate,
    }
}

fn swap_loop(loop_: &Loop, e: EdgeId, piece1: EdgeId, piece2: EdgeId) -> Loop {
    let mut uses = Vec::with_capacity(loop_.uses.len() + 1);
    for u in &loop_.uses {
        if u.edge == e {
            if u.reversed {
                uses.push(EdgeUse::backward(piece2));
                uses.push(EdgeUse::backward(piece1));
            } else {
                uses.push(EdgeUse::forward(piece1));
                uses.push(EdgeUse::forward(piece2));
            }
        } else {
            uses.push(*u);
        }
    }
    Loop { uses }
}

fn swap_edge(face: &Face, e: EdgeId, piece1: EdgeId, piece2: EdgeId) -> Face {
    Face {
        outer: swap_loop(&face.outer, e, piece1, piece2),
        inner: face
            .inner
            .iter()
            .map(|l| swap_loop(l, e, piece1, piece2))
            .collect(),
    }
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
        if !finite(a) || !finite(b) {
            return Err(StickyError::NotFinite);
        }
        if a.distance(b) <= LINEAR_TOL {
            return Err(StickyError::Degenerate);
        }

        let start = log.records.len();
        let result = self.add_edge_inner(a, b, log);
        if result.is_err() {
            self.rollback_to(log, start);
        }
        result
    }

    fn add_edge_inner(
        &mut self,
        a: DVec3,
        b: DVec3,
        log: &mut Edit,
    ) -> Result<EdgeChain, StickyError> {
        let va = self.snap_end(a, log)?;
        let vb = self.snap_end(b, log)?;
        if va == vb {
            return Err(StickyError::Degenerate);
        }
        let (Some(pa), Some(pb)) = (
            self.vertex(va).map(|v| v.position),
            self.vertex(vb).map(|v| v.position),
        ) else {
            return Err(StickyError::Degenerate);
        };

        let snapshot: Vec<(EdgeId, [VertexId; 2], [DVec3; 2])> = self
            .edges()
            .filter_map(|(id, edge)| {
                let p0 = self.vertex(edge.ends[0])?.position;
                let p1 = self.vertex(edge.ends[1])?.position;
                Some((id, edge.ends, [p0, p1]))
            })
            .collect();

        let mut collected = BTreeSet::new();
        collected.insert(va);
        collected.insert(vb);

        for (eid, ends, [p0, p1]) in snapshot {
            match geom::segment_intersection(pa, pb, p0, p1, LINEAR_TOL) {
                SegmentHit::None => {}
                SegmentHit::Point { tb, .. } => {
                    if tb > 0.0 && tb < 1.0 {
                        let m = self.split_edge(eid, p0.lerp(p1, tb), log)?;
                        collected.insert(m);
                    } else if tb <= 0.0 {
                        collected.insert(ends[0]);
                    } else {
                        collected.insert(ends[1]);
                    }
                }
                SegmentHit::Overlap { .. } => {
                    for (v, p) in [(ends[0], p0), (ends[1], p1)] {
                        if geom::point_segment_distance(p, pa, pb) <= LINEAR_TOL {
                            collected.insert(v);
                        }
                    }
                }
            }
        }

        let mut vertices: Vec<(f64, VertexId)> = collected
            .into_iter()
            .filter_map(|v| {
                let p = self.vertex(v)?.position;
                Some(((p - pa).dot(pb - pa), v))
            })
            .collect();
        vertices.sort_by(|a, b| a.0.total_cmp(&b.0));

        // A valid context has no two vertices within tolerance, so a plain point on the
        // segment only occurs once. Fold any numerically coincident pair anyway.
        let mut chain_vertices: Vec<VertexId> = Vec::with_capacity(vertices.len());
        for (_, v) in vertices {
            let duplicate = chain_vertices.last().is_some_and(|last| {
                let Some(p) = self.vertex(*last).map(|v| v.position) else {
                    return false;
                };
                let Some(q) = self.vertex(v).map(|v| v.position) else {
                    return false;
                };
                p.distance(q) <= LINEAR_TOL
            });
            if !duplicate {
                chain_vertices.push(v);
            }
        }

        let mut chain_edges = Vec::with_capacity(chain_vertices.len().saturating_sub(1));
        for pair in chain_vertices.windows(2) {
            let (u, v) = (pair[0], pair[1]);
            let e = match self.edge_between(u, v) {
                Some(e) => e,
                None => self.insert_edge(u, v, log).map_err(topo_error)?,
            };
            chain_edges.push(e);
        }

        Ok(EdgeChain {
            vertices: chain_vertices,
            edges: chain_edges,
        })
    }

    fn snap_end(&mut self, p: DVec3, log: &mut Edit) -> Result<VertexId, StickyError> {
        if let Some(v) = self.find_vertex(p, LINEAR_TOL) {
            return Ok(v);
        }
        let mut best: Option<(f64, EdgeId)> = None;
        for (id, edge) in self.edges() {
            let (Some(a), Some(b)) = (
                self.vertex(edge.ends[0]).map(|v| v.position),
                self.vertex(edge.ends[1]).map(|v| v.position),
            ) else {
                continue;
            };
            let d = geom::point_segment_distance(p, a, b);
            if d <= LINEAR_TOL && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, id));
            }
        }
        if let Some((_, e)) = best {
            self.split_edge(e, p, log)
        } else {
            Ok(self.insert_vertex(p, log))
        }
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
        let edge = *self.edge(e).ok_or(StickyError::MissingEdge(e))?;
        if !finite(p) {
            return Err(StickyError::NotFinite);
        }
        let start = log.records.len();
        let result = self.split_edge_inner(e, edge, p, log);
        if result.is_err() {
            self.rollback_to(log, start);
        }
        result
    }

    fn split_edge_inner(
        &mut self,
        e: EdgeId,
        edge: Edge,
        p: DVec3,
        log: &mut Edit,
    ) -> Result<VertexId, StickyError> {
        let (Some(a), Some(b)) = (
            self.vertex(edge.ends[0]).map(|v| v.position),
            self.vertex(edge.ends[1]).map(|v| v.position),
        ) else {
            return Err(StickyError::MissingEdge(e));
        };
        let q = closest_on_segment(p, a, b);
        let d0 = q.distance(a);
        let d1 = q.distance(b);
        if d0 <= LINEAR_TOL || d1 <= LINEAR_TOL {
            return Ok(if d0 <= d1 { edge.ends[0] } else { edge.ends[1] });
        }

        let m = self.insert_vertex(q, log);
        let piece1 = self.insert_edge(edge.ends[0], m, log).map_err(topo_error)?;
        let piece2 = self.insert_edge(m, edge.ends[1], log).map_err(topo_error)?;

        for f in self.faces_of_edge(e) {
            let Some(old) = self.face(f).cloned() else {
                continue;
            };
            let new = swap_edge(&old, e, piece1, piece2);
            self.replace_face(f, old, new, log).map_err(topo_error)?;
        }

        self.delete_edge(e, log).map_err(topo_error)?;
        Ok(m)
    }

    /// Erase `e`: faces bounded by it go, a hole it borders closes, and end vertices left
    /// without edges go.
    pub fn erase_edge(&mut self, e: EdgeId, log: &mut Edit) -> Result<(), StickyError> {
        let edge = *self.edge(e).ok_or(StickyError::MissingEdge(e))?;
        let start = log.records.len();
        let result = self.erase_edge_inner(e, edge, log);
        if result.is_err() {
            self.rollback_to(log, start);
        }
        result
    }

    fn erase_edge_inner(
        &mut self,
        e: EdgeId,
        edge: Edge,
        log: &mut Edit,
    ) -> Result<(), StickyError> {
        for f in self.faces_of_edge(e) {
            let Some(old) = self.face(f).cloned() else {
                continue;
            };
            if old.outer.uses.iter().any(|u| u.edge == e) {
                self.delete_face(f, log).map_err(topo_error)?;
            } else {
                let mut new = old.clone();
                new.inner.retain(|l| !l.uses.iter().any(|u| u.edge == e));
                self.replace_face(f, old, new, log).map_err(topo_error)?;
            }
        }

        self.delete_edge(e, log).map_err(topo_error)?;

        for v in edge.ends {
            if self.edges_of_vertex(v).is_empty() {
                self.delete_vertex(v, log).map_err(topo_error)?;
            }
        }
        Ok(())
    }

    fn replace_face(
        &mut self,
        f: FaceId,
        old: Face,
        new: Face,
        log: &mut Edit,
    ) -> Result<(), TopoError> {
        let remove = Record::RemoveFace(f, old);
        self.apply(&remove)?;
        log.records.push(remove);
        let add = Record::AddFace(f, new);
        self.apply(&add)?;
        log.records.push(add);
        Ok(())
    }

    fn rollback_to(&mut self, log: &mut Edit, start: usize) {
        for record in log.records[start..].iter().rev() {
            let _ = self.apply(&record.inverse());
        }
        log.records.truncate(start);
    }
}
