// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Sweep paths for Follow Me: a set of edges, or a face's outer loop, turned into one ordered
//! chain of vertices that a profile can be swept along.
//!
//! Behaviour: `docs/specs/m9-sweep-path.md`. The sweep itself (side faces, mitred joints, end
//! caps) builds on this in `docs/specs/m9-sweep-face.md`.

use std::collections::{BTreeMap, BTreeSet};

use glam::DVec3;

use crate::curve;
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
        let mut vertices = self.vertices.clone();
        let mut points = self.points.clone();
        let mut edges = self.edges.clone();
        if self.closed {
            if vertices.len() > 1 {
                vertices[1..].reverse();
                points[1..].reverse();
            }
            edges.reverse();
        } else {
            vertices.reverse();
            points.reverse();
            edges.reverse();
        }
        SweepPath {
            vertices,
            points,
            edges,
            closed: self.closed,
        }
    }

    /// Total length of the path's edges, in meters.
    pub fn length(&self) -> f64 {
        let n = self.points.len();
        if n < 2 {
            return 0.0;
        }
        let steps = if self.closed { n } else { n - 1 };
        (0..steps)
            .map(|i| self.points[i].distance(self.points[(i + 1) % n]))
            .sum()
    }
}

fn position(ctx: &Context, id: VertexId) -> DVec3 {
    ctx.vertex(id).map_or(DVec3::ZERO, |v| v.position)
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        parent[rb] = ra;
    }
}

/// Order `edges` into one path (spec: "Path from edges"). The order and repeats of `edges`
/// don't matter. Every curve with an edge in `edges` brings in all of its step edges.
pub fn path_from_edges(ctx: &Context, edges: &[EdgeId]) -> Result<SweepPath, PathError> {
    for &edge in edges {
        if ctx.edge(edge).is_none() {
            return Err(PathError::UnknownEdge(edge));
        }
    }
    if edges.is_empty() {
        return Err(PathError::Empty);
    }

    let given: BTreeSet<EdgeId> = edges.iter().copied().collect();
    let mut set = given.clone();
    for (id, _) in ctx.curves() {
        let steps = curve::curve_edges(ctx, id);
        if steps.iter().any(|edge| given.contains(edge)) {
            set.extend(steps);
        }
    }

    let mut degree: BTreeMap<VertexId, usize> = BTreeMap::new();
    for &id in &set {
        let ends = ctx.edge(id).expect("set edges are in the context").ends;
        for vertex in ends {
            *degree.entry(vertex).or_insert(0) += 1;
        }
    }
    for (&vertex, &count) in &degree {
        if count >= 3 {
            return Err(PathError::Branch(vertex));
        }
    }

    let edge_ids: Vec<EdgeId> = set.iter().copied().collect();
    let mut parent: Vec<usize> = (0..edge_ids.len()).collect();
    let mut seen_vertex: BTreeMap<VertexId, usize> = BTreeMap::new();
    for (i, &id) in edge_ids.iter().enumerate() {
        let ends = ctx.edge(id).expect("set edges are in the context").ends;
        for vertex in ends {
            if let Some(&j) = seen_vertex.get(&vertex) {
                union(&mut parent, i, j);
            } else {
                seen_vertex.insert(vertex, i);
            }
        }
    }
    let pieces: BTreeSet<usize> = (0..edge_ids.len()).map(|i| find(&mut parent, i)).collect();
    if pieces.len() > 1 {
        return Err(PathError::Gap {
            pieces: pieces.len(),
        });
    }

    let mut incident: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for &id in &edge_ids {
        let ends = ctx.edge(id).expect("set edges are in the context").ends;
        incident.entry(ends[0]).or_default().push(id);
        incident.entry(ends[1]).or_default().push(id);
    }

    let ends: Vec<VertexId> = incident
        .iter()
        .filter(|(_, edges)| edges.len() == 1)
        .map(|(&vertex, _)| vertex)
        .collect();

    let mut vertices;
    let mut walk_edges;
    let closed;
    if let Some(&start) = ends.first() {
        closed = false;
        vertices = vec![start];
        walk_edges = Vec::new();
        let mut current = start;
        let mut previous: Option<EdgeId> = None;
        loop {
            let next_edge = incident[&current]
                .iter()
                .copied()
                .find(|edge| Some(*edge) != previous)
                .expect("a chain end continues");
            let ends = ctx
                .edge(next_edge)
                .expect("set edges are in the context")
                .ends;
            let next = if ends[0] == current { ends[1] } else { ends[0] };
            walk_edges.push(next_edge);
            vertices.push(next);
            if incident[&next].len() == 1 {
                break;
            }
            previous = Some(next_edge);
            current = next;
        }
    } else {
        closed = true;
        let start = *incident.keys().next().expect("the set is not empty");
        let first_edge = *incident[&start]
            .iter()
            .min()
            .expect("a closed path vertex has two edges");
        vertices = vec![start];
        walk_edges = Vec::new();
        let mut current = start;
        let mut edge_to_take = first_edge;
        loop {
            let ends = ctx
                .edge(edge_to_take)
                .expect("set edges are in the context")
                .ends;
            let next = if ends[0] == current { ends[1] } else { ends[0] };
            walk_edges.push(edge_to_take);
            if next == start {
                break;
            }
            vertices.push(next);
            let next_edge = *incident[&next]
                .iter()
                .find(|edge| **edge != edge_to_take)
                .expect("a closed path vertex has two edges");
            current = next;
            edge_to_take = next_edge;
        }
    }

    let points = vertices.iter().map(|&id| position(ctx, id)).collect();
    Ok(SweepPath {
        vertices,
        points,
        edges: walk_edges,
        closed,
    })
}

/// The outer loop of `face` as a closed path, in the loop's own direction (counter-clockwise
/// seen from the front), starting at its lowest-id vertex (spec: "Path around a face").
pub fn face_perimeter(ctx: &Context, face: FaceId) -> Result<SweepPath, PathError> {
    let face = ctx.face(face).ok_or(PathError::UnknownFace(face))?;
    let mut steps: Vec<(VertexId, EdgeId)> = face
        .outer
        .uses
        .iter()
        .filter_map(|use_| ctx.use_start(*use_).map(|vertex| (vertex, use_.edge)))
        .collect();
    let start = steps
        .iter()
        .enumerate()
        .min_by_key(|&(_, &(vertex, _))| vertex)
        .map(|(i, _)| i)
        .unwrap_or(0);
    steps.rotate_left(start);
    let vertices: Vec<VertexId> = steps.iter().map(|&(vertex, _)| vertex).collect();
    let edges: Vec<EdgeId> = steps.iter().map(|&(_, edge)| edge).collect();
    let points = vertices.iter().map(|&id| position(ctx, id)).collect();
    Ok(SweepPath {
        vertices,
        points,
        edges,
        closed: true,
    })
}
