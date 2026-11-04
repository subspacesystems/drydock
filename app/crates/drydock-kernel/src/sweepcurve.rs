// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Curves carried through a sweep: a profile's arcs and circles reappear on the sections the
//! sweep builds when those are still circles, and a profile swept along an arc of the path
//! leaves an arc through each profile vertex.
//!
//! Behaviour: `docs/specs/m9-sweep-curves.md`. `sweepface::sweep_face` reads a `CurveCarry`
//! before it changes the body and calls `add_curves` once the sides are built and merged.

use std::collections::{BTreeMap, BTreeSet};
use std::iter::once;

use crate::curve::{self, Curve};
use crate::sweep::SweepPath;
use crate::topology::{Context, CurveId, EdgeId, Edit, FaceId, VertexId};

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
        let face = ctx.face(profile).cloned();
        let mut loops = Vec::new();
        let mut profile_edges = BTreeSet::new();
        if let Some(face) = &face {
            for loop_ in once(&face.outer).chain(face.inner.iter()) {
                loops.push(ctx.loop_vertices(loop_));
                for use_ in &loop_.uses {
                    profile_edges.insert(use_.edge);
                }
            }
        }

        let curves: Vec<Curve> = ctx
            .curves()
            .filter_map(|(id, curve)| {
                let edges = curve::curve_edges(ctx, id);
                (!edges.is_empty() && edges.iter().all(|e| profile_edges.contains(e)))
                    .then(|| curve.clone())
            })
            .collect();

        let section_count = at.len();
        let step_count = if path.closed {
            section_count
        } else {
            section_count.saturating_sub(1)
        };
        let path_count = path.points.len();
        let mut steps = Vec::with_capacity(step_count);
        for step in 0..step_count {
            let a = at[step];
            let b = at[(step + 1) % section_count];
            let seg = if path.closed { a } else { a.min(b) };
            let mut found = None;
            if let (Some(&edge), Some(&v0), Some(&v1)) = (
                path.edges.get(seg),
                path.vertices.get(seg),
                path.vertices.get((seg + 1) % path_count),
            ) && let Some(data) = ctx.edge(edge)
                && ((data.ends[0] == v0 && data.ends[1] == v1)
                    || (data.ends[0] == v1 && data.ends[1] == v0))
            {
                found = curve::curve_of_edge(ctx, edge);
            }
            steps.push(found);
        }

        CurveCarry {
            profile: loops,
            curves,
            steps,
            closed: path.closed,
        }
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
        if self.curves.is_empty() && self.steps.iter().all(Option::is_none) {
            return Vec::new();
        }

        let mut used = BTreeSet::new();
        for (id, _) in ctx.curves() {
            used.extend(curve::curve_edges(ctx, id));
        }
        let mut out = Vec::new();

        let mut vertex_at = BTreeMap::new();
        for (l, loop_) in self.profile.iter().enumerate() {
            for (p, vertex) in loop_.iter().enumerate() {
                vertex_at.insert(*vertex, (l, p));
            }
        }

        for section in sections {
            for curve in &self.curves {
                let mut chain = Vec::with_capacity(curve.vertices.len());
                let mut complete = true;
                for vertex in &curve.vertices {
                    match vertex_at.get(vertex) {
                        Some(&(l, p)) => chain.push(section[l][p]),
                        None => {
                            complete = false;
                            break;
                        }
                    }
                }
                if complete {
                    try_add(ctx, log, &chain, curve.start, &mut used, &mut out);
                }
            }
        }

        let step_count = self.steps.len();
        let mut runs = Vec::new();
        let mut whole_ring = false;
        if self.closed && step_count > 0 {
            let first = self.steps[0];
            if first.is_some() && self.steps.iter().all(|step| *step == first) {
                whole_ring = true;
                runs.push((0, step_count));
            } else {
                let boundary = (0..step_count)
                    .find(|&i| self.steps[i] != self.steps[(i + 1) % step_count])
                    .unwrap_or(0);
                let mut i = 0;
                while i < step_count {
                    let index = (boundary + 1 + i) % step_count;
                    match self.steps[index] {
                        Some(found) => {
                            let mut length = 0;
                            while i + length < step_count
                                && self.steps[(boundary + 1 + i + length) % step_count]
                                    == Some(found)
                            {
                                length += 1;
                            }
                            runs.push((index, length));
                            i += length;
                        }
                        None => i += 1,
                    }
                }
            }
        } else {
            let mut i = 0;
            while i < step_count {
                match self.steps[i] {
                    Some(found) => {
                        let start = i;
                        while i < step_count && self.steps[i] == Some(found) {
                            i += 1;
                        }
                        runs.push((start, i - start));
                    }
                    None => i += 1,
                }
            }
        }
        runs.sort_by_key(|(start, _)| *start);

        let section_count = sections.len();
        for (start, length) in runs {
            if length < 2 {
                continue;
            }
            for (l, loop_) in self.profile.iter().enumerate() {
                for p in 0..loop_.len() {
                    if whole_ring {
                        let mut chain = Vec::with_capacity(section_count + 1);
                        for section in sections {
                            chain.push(section[l][p]);
                        }
                        if section_count > 0 {
                            chain.push(sections[0][l][p]);
                        }
                        try_add(ctx, log, &chain, 0.0, &mut used, &mut out);
                        continue;
                    }

                    let mut chain = Vec::with_capacity(length + 1);
                    for k in 0..=length {
                        let index = if self.closed {
                            (start + k) % section_count
                        } else {
                            start + k
                        };
                        chain.push(sections[index][l][p]);
                    }

                    let mut tries = Vec::with_capacity(4);
                    tries.push(chain.clone());
                    if chain.len() > 1 {
                        tries.push(chain[..chain.len() - 1].to_vec());
                        tries.push(chain[1..].to_vec());
                    }
                    if chain.len() > 2 {
                        tries.push(chain[1..chain.len() - 1].to_vec());
                    }
                    for candidate in tries {
                        if candidate.len() < 3 {
                            continue;
                        }
                        if let Some(curve) = curve::fit(ctx, &candidate, 0.0) {
                            commit(ctx, log, &candidate, curve, &mut used, &mut out);
                            break;
                        }
                    }
                }
            }
        }

        out
    }
}

fn try_add(
    ctx: &mut Context,
    log: &mut Edit,
    chain: &[VertexId],
    start: f64,
    used: &mut BTreeSet<EdgeId>,
    out: &mut Vec<CurveId>,
) {
    let Some(curve) = curve::fit(ctx, chain, start) else {
        return;
    };
    commit(ctx, log, chain, curve, used, out);
}

fn commit(
    ctx: &mut Context,
    log: &mut Edit,
    chain: &[VertexId],
    curve: Curve,
    used: &mut BTreeSet<EdgeId>,
    out: &mut Vec<CurveId>,
) {
    let mut edges = Vec::new();
    for pair in chain.windows(2) {
        match curve::step_edges(ctx, pair[0], pair[1]) {
            Some(mut step) => edges.append(&mut step),
            None => return,
        }
    }
    if edges.iter().any(|edge| used.contains(edge)) {
        return;
    }
    let id = ctx.insert_curve(curve, log);
    used.extend(edges);
    out.push(id);
}
