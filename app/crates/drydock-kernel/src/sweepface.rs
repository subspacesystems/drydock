// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Follow Me: sweep a planar profile face along a path, with mitred joints. An open path gives
//! a solid with the profile as its start cap and a new end cap; a closed path gives a ring.
//!
//! Behaviour: `docs/specs/m9-sweep-face.md`. The path comes from `sweep.rs`.

use std::collections::BTreeSet;
use std::iter::once;

use glam::DVec3;

use crate::faces;
use crate::geom::{self, LINEAR_TOL};
use crate::model::{BodyId, FaceMaterials, Model, ModelEdit, ModelError, UndoStack};
use crate::sweep::SweepPath;
use crate::topology::{
    Context, EdgeId, EdgeUse, Face, FaceId, Loop, Record, TopoError, VertexId, Violation,
};

/// Smallest angle, in degrees, between the profile's plane and the path segment the sweep
/// starts along. A profile closer to parallel than this is refused (`ProfileParallel`).
pub const MIN_PROFILE_ANGLE_DEG: f64 = 1.0;

/// Largest turn, in degrees, between two consecutive path segments. A sharper turn (the path
/// nearly doubling back) has no usable mitre plane (`DegenerateMitre`).
pub const MAX_TURN_DEG: f64 = 179.0;

/// Why a sweep was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum SweepError {
    MissingBody(BodyId),
    /// The profile face isn't in the body.
    MissingFace(FaceId),
    /// Too few points (open: 2, closed: 3), `vertices` and `points` of different lengths, a
    /// point that is not finite, or a segment (a closed path's closing one too) no longer
    /// than `LINEAR_TOL`.
    BadPath,
    /// The lowest-id edge of the profile that another face also uses.
    ProfileShared(EdgeId),
    /// The path turns by more than `MAX_TURN_DEG` at this vertex.
    DegenerateMitre(VertexId),
    /// Closed path: the profile's plane doesn't meet the path.
    NotOnPath,
    /// Closed path: the profile's plane meets the path at this vertex, but the profile doesn't
    /// lie in the vertex's mitre plane.
    MitreMismatch(VertexId),
    /// The profile's plane is within `MIN_PROFILE_ANGLE_DEG` of parallel to the segment the
    /// sweep starts along.
    ProfileParallel,
    /// Some profile point would not move forward (by more than `LINEAR_TOL`) on the way to
    /// the section plane at this path vertex: the profile is too big for the turn, or lies
    /// beyond the vertex.
    Folded(VertexId),
    /// Closed path: carried once round, the profile doesn't come back onto itself (a path
    /// that isn't flat twists it).
    Twisted,
    /// The result would break a `validate` invariant that held before.
    Invalid(Violation),
    /// The result would make two coplanar faces overlap that didn't before (lower id first).
    Overlap(FaceId, FaceId),
    Model(ModelError),
}

/// The faces a sweep left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Swept {
    /// Open path: the profile face (same id), fronting away from the sweep. Closed: `None`.
    pub start: Option<FaceId>,
    /// Open path: the new face at the end of the path. Closed: `None`.
    pub end: Option<FaceId>,
    /// The side faces left after coplanar ones merged, ascending.
    pub sides: Vec<FaceId>,
}

struct Plan {
    sections: Vec<Vec<Vec<DVec3>>>,
    loops: Vec<Vec<DVec3>>,
    normal: DVec3,
    start_d: DVec3,
    closed: bool,
    dirs: Vec<DVec3>,
}

enum CandidateKind {
    Vertex(usize),
    Segment(usize),
}

struct Candidate {
    order: f64,
    kind: CandidateKind,
    distance: f64,
}

struct BuildOut {
    sides: Vec<FaceId>,
    end: Option<FaceId>,
}

fn segment_dir(a: DVec3, b: DVec3) -> DVec3 {
    (b - a).normalize()
}

fn turn_degrees(a: DVec3, b: DVec3) -> f64 {
    a.dot(b).clamp(-1.0, 1.0).acos().to_degrees()
}

fn reverse_loop(loop_: &Loop) -> Loop {
    let mut uses: Vec<EdgeUse> = loop_.uses.iter().rev().copied().collect();
    for use_ in &mut uses {
        use_.reversed = !use_.reversed;
    }
    Loop { uses }
}

fn use_between(
    ctx: &mut Context,
    a: VertexId,
    b: VertexId,
    inserted: &mut Vec<EdgeId>,
    log: &mut crate::topology::Edit,
) -> Result<EdgeUse, TopoError> {
    if let Some(e) = ctx.edge_between(a, b) {
        let ends = ctx.edge(e).ok_or(TopoError::MissingEdge(e))?.ends;
        Ok(if ends[0] == a && ends[1] == b {
            EdgeUse::forward(e)
        } else {
            EdgeUse::backward(e)
        })
    } else {
        let e = ctx.insert_edge(a, b, log)?;
        inserted.push(e);
        Ok(EdgeUse::forward(e))
    }
}

fn flipped(use_: EdgeUse) -> EdgeUse {
    EdgeUse {
        edge: use_.edge,
        reversed: !use_.reversed,
    }
}

fn assign_vertices(
    ctx: &mut Context,
    sections: &[Vec<Vec<DVec3>>],
    log: &mut crate::topology::Edit,
) -> Vec<Vec<Vec<VertexId>>> {
    sections
        .iter()
        .map(|section| {
            section
                .iter()
                .map(|loop_| {
                    loop_
                        .iter()
                        .map(|&p| {
                            ctx.find_vertex(p, LINEAR_TOL)
                                .unwrap_or_else(|| ctx.insert_vertex(p, log))
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

fn carry_section(
    section: &[Vec<DVec3>],
    d: DVec3,
    q: DVec3,
    m: DVec3,
    vertex: VertexId,
) -> Result<Vec<Vec<DVec3>>, SweepError> {
    let denom = d.dot(m);
    let mut out = Vec::with_capacity(section.len());
    for loop_ in section {
        let mut carried = Vec::with_capacity(loop_.len());
        for &x in loop_ {
            let t = (q - x).dot(m) / denom;
            let moved = x + d * t;
            if (moved - x).dot(d) <= LINEAR_TOL {
                return Err(SweepError::Folded(vertex));
            }
            carried.push(moved);
        }
        out.push(carried);
    }
    Ok(out)
}

fn same_section(a: &[Vec<DVec3>], b: &[Vec<DVec3>]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(la, lb)| {
            la.len() == lb.len() && la.iter().zip(lb).all(|(p, q)| p.distance(*q) <= LINEAR_TOL)
        })
}

fn plan_sweep(ctx: &Context, profile: FaceId, path: &SweepPath) -> Result<Plan, SweepError> {
    let face = ctx
        .face(profile)
        .cloned()
        .ok_or(SweepError::MissingFace(profile))?;

    let k = path.points.len();
    if path.vertices.len() != k
        || (path.closed && k < 3)
        || (!path.closed && k < 2)
        || path.points.iter().any(|p| !p.is_finite())
    {
        return Err(SweepError::BadPath);
    }
    let segments = if path.closed { k } else { k - 1 };
    for i in 0..segments {
        let a = path.points[i];
        let b = path.points[(i + 1) % k];
        if (b - a).length() <= LINEAR_TOL {
            return Err(SweepError::BadPath);
        }
    }

    let plane = ctx
        .face_plane(profile)
        .ok_or(SweepError::Invalid(Violation::NonPlanarFace(profile)))?;
    let n = plane.normal;

    let loops: Vec<Loop> = once(&face.outer)
        .chain(face.inner.iter())
        .cloned()
        .collect();
    let loop_points: Vec<Vec<DVec3>> = loops.iter().map(|l| ctx.loop_points(l)).collect();
    let centroid = loop_points[0].iter().copied().sum::<DVec3>() / loop_points[0].len() as f64;

    let mut profile_edges = BTreeSet::new();
    for loop_ in &loops {
        for use_ in &loop_.uses {
            profile_edges.insert(use_.edge);
        }
    }
    for e in &profile_edges {
        if ctx.faces_of_edge(*e).iter().any(|f| *f != profile) {
            return Err(SweepError::ProfileShared(*e));
        }
    }

    if path.closed {
        let dirs: Vec<DVec3> = (0..k)
            .map(|i| segment_dir(path.points[i], path.points[(i + 1) % k]))
            .collect();
        for j in 0..k {
            let arriving = dirs[(j + k - 1) % k];
            let leaving = dirs[j];
            if turn_degrees(arriving, leaving) > MAX_TURN_DEG {
                return Err(SweepError::DegenerateMitre(path.vertices[j]));
            }
        }
        plan_closed(path, plane, n, loop_points, centroid, dirs)
    } else {
        for j in 1..k - 1 {
            let arriving = segment_dir(path.points[j - 1], path.points[j]);
            let leaving = segment_dir(path.points[j], path.points[j + 1]);
            if turn_degrees(arriving, leaving) > MAX_TURN_DEG {
                return Err(SweepError::DegenerateMitre(path.vertices[j]));
            }
        }
        plan_open(path, n, loop_points, centroid)
    }
}

fn plan_open(
    path: &SweepPath,
    n: DVec3,
    loops: Vec<Vec<DVec3>>,
    centroid: DVec3,
) -> Result<Plan, SweepError> {
    let k = path.points.len();
    let mut points = path.points.clone();
    let mut vertices = path.vertices.clone();
    if (points[k - 1] - centroid).length() < (points[0] - centroid).length() - LINEAR_TOL {
        points.reverse();
        vertices.reverse();
    }
    let dirs: Vec<DVec3> = (0..k - 1)
        .map(|i| segment_dir(points[i], points[i + 1]))
        .collect();
    let start_d = dirs[0];
    if n.dot(start_d).abs() < MIN_PROFILE_ANGLE_DEG.to_radians().sin() {
        return Err(SweepError::ProfileParallel);
    }

    let mut sections = vec![loops.clone()];
    let mut previous = loops.clone();
    for i in 1..k - 1 {
        let m = (dirs[i - 1] + dirs[i]).normalize();
        let next = carry_section(&previous, dirs[i - 1], points[i], m, vertices[i])?;
        sections.push(next.clone());
        previous = next;
    }
    let last_d = dirs[k - 2];
    let last = carry_section(&previous, last_d, points[k - 1], last_d, vertices[k - 1])?;
    sections.push(last);

    Ok(Plan {
        sections,
        loops,
        normal: n,
        start_d,
        closed: false,
        dirs,
    })
}

fn plan_closed(
    path: &SweepPath,
    plane: geom::Plane,
    n: DVec3,
    loops: Vec<Vec<DVec3>>,
    centroid: DVec3,
    dirs: Vec<DVec3>,
) -> Result<Plan, SweepError> {
    let k = path.points.len();
    let signed: Vec<f64> = path
        .points
        .iter()
        .map(|p| plane.signed_distance(*p))
        .collect();

    let mut candidates = Vec::new();
    for (j, &s) in signed.iter().enumerate() {
        if s.abs() <= LINEAR_TOL {
            candidates.push(Candidate {
                order: j as f64,
                kind: CandidateKind::Vertex(j),
                distance: path.points[j].distance(centroid),
            });
        }
    }
    for i in 0..k {
        let j = (i + 1) % k;
        if signed[i].abs() > LINEAR_TOL
            && signed[j].abs() > LINEAR_TOL
            && signed[i] * signed[j] < 0.0
        {
            let t = signed[i] / (signed[i] - signed[j]);
            let point = path.points[i] + (path.points[j] - path.points[i]) * t;
            candidates.push(Candidate {
                order: i as f64 + 0.5,
                kind: CandidateKind::Segment(i),
                distance: point.distance(centroid),
            });
        }
    }
    if candidates.is_empty() {
        return Err(SweepError::NotOnPath);
    }
    candidates.sort_by(|a, b| a.order.total_cmp(&b.order));
    let mut best = &candidates[0];
    for candidate in &candidates[1..] {
        if candidate.distance < best.distance - LINEAR_TOL {
            best = candidate;
        }
    }

    match best.kind {
        CandidateKind::Vertex(j) => {
            let arriving = dirs[(j + k - 1) % k];
            let leaving = dirs[j];
            let mitre = (arriving + leaving).normalize();
            for loop_ in &loops {
                for &p in loop_ {
                    if (p - path.points[j]).dot(mitre).abs() > LINEAR_TOL {
                        return Err(SweepError::MitreMismatch(path.vertices[j]));
                    }
                }
            }
            let start_d = leaving;
            if n.dot(start_d).abs() < MIN_PROFILE_ANGLE_DEG.to_radians().sin() {
                return Err(SweepError::ProfileParallel);
            }

            let mut sections = vec![loops.clone()];
            let mut previous = loops.clone();
            let mut previous_index = j;
            for _ in 0..k - 1 {
                let next_index = (previous_index + 1) % k;
                let segment = dirs[previous_index];
                let next_mitre = (dirs[(next_index + k - 1) % k] + dirs[next_index]).normalize();
                let next = carry_section(
                    &previous,
                    segment,
                    path.points[next_index],
                    next_mitre,
                    path.vertices[next_index],
                )?;
                sections.push(next.clone());
                previous = next;
                previous_index = next_index;
            }
            let closed = carry_section(
                &previous,
                dirs[previous_index],
                path.points[j],
                mitre,
                path.vertices[j],
            )?;
            if !same_section(&closed, &sections[0]) {
                return Err(SweepError::Twisted);
            }

            Ok(Plan {
                sections,
                loops,
                normal: n,
                start_d,
                closed: true,
                dirs,
            })
        }
        CandidateKind::Segment(i) => {
            let far = (i + 1) % k;
            let far_mitre = (dirs[i] + dirs[far]).normalize();
            let start_d = dirs[i];
            if n.dot(start_d).abs() < MIN_PROFILE_ANGLE_DEG.to_radians().sin() {
                return Err(SweepError::ProfileParallel);
            }

            let first = carry_section(
                &loops,
                dirs[i],
                path.points[far],
                far_mitre,
                path.vertices[far],
            )?;
            let mut sections = vec![first.clone()];
            let mut previous = first;
            let mut previous_index = far;
            for _ in 0..k - 1 {
                let next_index = (previous_index + 1) % k;
                let segment = dirs[previous_index];
                let next_mitre = (dirs[(next_index + k - 1) % k] + dirs[next_index]).normalize();
                let next = carry_section(
                    &previous,
                    segment,
                    path.points[next_index],
                    next_mitre,
                    path.vertices[next_index],
                )?;
                sections.push(next.clone());
                previous = next;
                previous_index = next_index;
            }
            let closed = carry_section(
                &previous,
                dirs[previous_index],
                path.points[far],
                far_mitre,
                path.vertices[far],
            )?;
            if !same_section(&closed, &sections[0]) {
                return Err(SweepError::Twisted);
            }

            Ok(Plan {
                sections,
                loops,
                normal: n,
                start_d,
                closed: true,
                dirs,
            })
        }
    }
}

fn build_sides(
    ctx: &mut Context,
    log: &mut crate::topology::Edit,
    plan: &Plan,
    section_vertices: &[Vec<Vec<VertexId>>],
) -> Result<(Vec<FaceId>, Vec<EdgeId>), TopoError> {
    let count = plan.sections.len();
    let steps: Vec<(usize, usize)> = if plan.closed {
        (0..count).map(|i| (i, (i + 1) % count)).collect()
    } else {
        (0..count.saturating_sub(1)).map(|i| (i, i + 1)).collect()
    };
    let reverse_loops = plan.normal.dot(plan.start_d) < 0.0;
    let mut sides = Vec::new();
    let mut inserted = Vec::new();

    for (loop_index, loop_points) in plan.loops.iter().enumerate() {
        let len = loop_points.len();
        let order: Vec<usize> = if reverse_loops {
            (0..len).rev().collect()
        } else {
            (0..len).collect()
        };
        for w in 0..len {
            let a_index = order[w];
            let b_index = order[(w + 1) % len];
            for &(i, j) in &steps {
                let a_i = section_vertices[i][loop_index][a_index];
                let b_i = section_vertices[i][loop_index][b_index];
                let a_j = section_vertices[j][loop_index][a_index];
                let b_j = section_vertices[j][loop_index][b_index];

                let a_i_b_i = use_between(ctx, a_i, b_i, &mut inserted, log)?;
                let b_i_b_j = use_between(ctx, b_i, b_j, &mut inserted, log)?;
                let a_j_b_j = use_between(ctx, a_j, b_j, &mut inserted, log)?;
                let a_i_a_j = use_between(ctx, a_i, a_j, &mut inserted, log)?;

                let face = ctx.insert_face(
                    Face {
                        outer: Loop {
                            uses: vec![a_i_b_i, b_i_b_j, flipped(a_j_b_j), flipped(a_i_a_j)],
                        },
                        inner: Vec::new(),
                    },
                    log,
                )?;
                sides.push(face);
            }
        }
    }
    Ok((sides, inserted))
}

fn build_cap(
    ctx: &mut Context,
    log: &mut crate::topology::Edit,
    section: &[Vec<DVec3>],
    section_vertices: &[Vec<VertexId>],
    normal: DVec3,
    inserted: &mut Vec<EdgeId>,
) -> Result<FaceId, TopoError> {
    let reverse_loops = geom::area_vector(&section[0]).dot(normal) < 0.0;
    let mut loops = Vec::new();
    for (loop_index, points) in section.iter().enumerate() {
        let len = points.len();
        let order: Vec<usize> = if reverse_loops {
            (0..len).rev().collect()
        } else {
            (0..len).collect()
        };
        let mut uses = Vec::with_capacity(len);
        for w in 0..len {
            let a = section_vertices[loop_index][order[w]];
            let b = section_vertices[loop_index][order[(w + 1) % len]];
            uses.push(use_between(ctx, a, b, inserted, log)?);
        }
        loops.push(Loop { uses });
    }
    let mut loops = loops.into_iter();
    let outer = loops.next().expect("profile has an outer loop");
    ctx.insert_face(
        Face {
            outer,
            inner: loops.collect(),
        },
        log,
    )
}

fn merge_sides(
    ctx: &mut Context,
    log: &mut crate::topology::Edit,
    mut sides: Vec<FaceId>,
    candidates: &[EdgeId],
) -> Vec<FaceId> {
    let side_set: BTreeSet<FaceId> = sides.iter().copied().collect();
    let mut candidates = candidates.to_vec();
    candidates.sort_unstable();
    candidates.dedup();

    for &e in &candidates {
        if ctx.edge(e).is_none() {
            continue;
        }
        let faces = ctx.faces_of_edge(e);
        if faces.len() != 2 || !faces.iter().all(|f| side_set.contains(f)) {
            continue;
        }
        let (Some(a), Some(b)) = (ctx.face_plane(faces[0]), ctx.face_plane(faces[1])) else {
            continue;
        };
        if a.normal.dot(b.normal) > 0.0 && faces::same_plane(&a, &b) {
            let _ = ctx.erase_edge(e, log);
        }
    }

    for &e in &candidates {
        if ctx.edge(e).is_some() && ctx.faces_of_edge(e).is_empty() {
            let _ = ctx.delete_edge(e, log);
        }
    }

    sides.retain(|f| ctx.face(*f).is_some());
    sides.sort_unstable();
    sides
}

fn build_open(
    ctx: &mut Context,
    log: &mut crate::topology::Edit,
    plan: &Plan,
    profile: FaceId,
) -> Result<BuildOut, TopoError> {
    let section_vertices = assign_vertices(ctx, &plan.sections, log);

    if plan.normal.dot(plan.start_d) > 0.0
        && let Some(face) = ctx.face(profile).cloned()
    {
        let mut turned = face.clone();
        turned.outer = reverse_loop(&face.outer);
        turned.inner = face.inner.iter().map(reverse_loop).collect();
        let remove = Record::RemoveFace(profile, face);
        ctx.apply(&remove)?;
        log.records.push(remove);
        let add = Record::AddFace(profile, turned);
        ctx.apply(&add)?;
        log.records.push(add);
    }

    let (sides, mut inserted) = build_sides(ctx, log, plan, &section_vertices)?;
    let last_d = *plan.dirs.last().expect("open path has a segment");
    let end = build_cap(
        ctx,
        log,
        plan.sections.last().expect("open path has a section"),
        section_vertices.last().expect("open path has a section"),
        last_d,
        &mut inserted,
    )?;
    let sides = merge_sides(ctx, log, sides, &inserted);
    Ok(BuildOut {
        sides,
        end: Some(end),
    })
}

fn build_closed(
    ctx: &mut Context,
    log: &mut crate::topology::Edit,
    plan: &Plan,
    profile: FaceId,
) -> Result<BuildOut, TopoError> {
    let section_vertices = assign_vertices(ctx, &plan.sections, log);
    let (sides, inserted) = build_sides(ctx, log, plan, &section_vertices)?;

    let mut profile_edges = BTreeSet::new();
    let mut profile_vertices = BTreeSet::new();
    if let Some(face) = ctx.face(profile).cloned() {
        for loop_ in once(&face.outer).chain(face.inner.iter()) {
            for use_ in &loop_.uses {
                profile_edges.insert(use_.edge);
            }
            for v in ctx.loop_vertices(loop_) {
                profile_vertices.insert(v);
            }
        }
        ctx.delete_face(profile, log)?;
    }
    for &e in &profile_edges {
        if ctx.edge(e).is_some() && ctx.faces_of_edge(e).is_empty() {
            ctx.delete_edge(e, log)?;
        }
    }
    for &v in &profile_vertices {
        if ctx.vertex(v).is_some() && ctx.edges_of_vertex(v).is_empty() {
            ctx.delete_vertex(v, log)?;
        }
    }

    let mut candidates = inserted;
    candidates.extend(
        profile_edges
            .iter()
            .copied()
            .filter(|e| ctx.edge(*e).is_some()),
    );
    let sides = merge_sides(ctx, log, sides, &candidates);
    Ok(BuildOut { sides, end: None })
}

fn roll_back(model: &mut Model, edit: &ModelEdit) {
    let _ = model.apply_edit(&edit.inverse());
}

/// The profile's cross-sections along the path, without changing anything (for the tool's
/// preview): one per section plane in sweep order; each holds the profile's loops (outer
/// first, then each hole in the face's order), each loop's points in the profile loop's own
/// order starting at its first use's start vertex. Fails as [`sweep_face`] does, up to and
/// including `Twisted`.
pub fn sweep_sections(
    ctx: &Context,
    profile: FaceId,
    path: &SweepPath,
) -> Result<Vec<Vec<Vec<DVec3>>>, SweepError> {
    plan_sweep(ctx, profile, path).map(|plan| plan.sections)
}

/// Sweep `profile` of `body` along `path` as one undo step labelled `label`. Only
/// `path.points`, `path.closed` and (for errors) `path.vertices` are used; the path's
/// vertices and edges are left as they are.
pub fn sweep_face(
    model: &mut Model,
    body: BodyId,
    profile: FaceId,
    path: &SweepPath,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Swept, SweepError> {
    let plan = {
        let body_ref = model.body(body).ok_or(SweepError::MissingBody(body))?;
        plan_sweep(body_ref.geometry(), profile, path)?
    };
    let (old_materials, before_violations, before_overlaps) = {
        let body_ref = model.body(body).ok_or(SweepError::MissingBody(body))?;
        let ctx = body_ref.geometry();
        (
            body_ref.face_materials(profile),
            ctx.validate(),
            ctx.face_overlaps(),
        )
    };

    let mut edit = ModelEdit::new();
    if plan.closed {
        model
            .set_face_materials(body, profile, FaceMaterials::default(), &mut edit)
            .map_err(SweepError::Model)?;
    }

    let built = model.edit_geometry(body, &mut edit, |ctx, log| {
        if plan.closed {
            build_closed(ctx, log, &plan, profile)
        } else {
            build_open(ctx, log, &plan, profile)
        }
    });
    let built = match built {
        Ok(Ok(out)) => out,
        Ok(Err(error)) => {
            roll_back(model, &edit);
            return Err(SweepError::Model(ModelError::Topo(error)));
        }
        Err(error) => {
            roll_back(model, &edit);
            return Err(SweepError::Model(error));
        }
    };

    if !plan.closed
        && let Some(end) = built.end
    {
        let materials = FaceMaterials {
            front: old_materials.front,
            back: old_materials.back,
            front_map: None,
            back_map: None,
        };
        if materials != FaceMaterials::default()
            && let Err(error) = model.set_face_materials(body, end, materials, &mut edit)
        {
            roll_back(model, &edit);
            return Err(SweepError::Model(error));
        }
    }

    let (new_violation, new_overlap) = {
        let body_ref = model.body(body).ok_or(SweepError::MissingBody(body))?;
        let ctx = body_ref.geometry();
        (
            ctx.validate()
                .into_iter()
                .find(|v| !before_violations.contains(v)),
            ctx.face_overlaps()
                .into_iter()
                .find(|pair| !before_overlaps.contains(pair)),
        )
    };

    if let Some(violation) = new_violation {
        roll_back(model, &edit);
        return Err(SweepError::Invalid(violation));
    }
    if let Some((a, b)) = new_overlap {
        roll_back(model, &edit);
        return Err(SweepError::Overlap(a, b));
    }

    undo.push(label, edit);
    Ok(Swept {
        start: if plan.closed { None } else { Some(profile) },
        end: built.end,
        sides: built.sides,
    })
}
