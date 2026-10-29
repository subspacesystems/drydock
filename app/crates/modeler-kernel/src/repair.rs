// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Solid repair for 3D printing: remove internal, stray and loose geometry, fill flat holes
//! and turn reversed faces round, in one undo step.
//!
//! Behaviour: `docs/specs/m7-solid-repair.md`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use glam::DVec3;

use crate::geom::{self, Containment};
use crate::model::{BodyId, FaceMaterials, Model, ModelEdit, ModelError, UndoStack};
use crate::solid::SolidReport;
use crate::topology::{Context, EdgeId, EdgeUse, Edit, Face, FaceId, Loop, Record, VertexId};

/// Something the repair left as it was. Edge lists are in ascending id order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unfixed {
    /// Connected open edges that don't form one closed loop: some vertex has other than two
    /// of them.
    NotOneLoop(Vec<EdgeId>),
    /// A closed loop of open edges whose vertices don't lie in one plane.
    NotFlat(Vec<EdgeId>),
    /// A flat loop in the same plane as another one, one inside the other (the border of a
    /// ring-shaped hole).
    Nested(Vec<EdgeId>),
    /// The face that would fill the loop isn't a valid face.
    BadFill(Vec<EdgeId>),
    /// The face that would fill the loop overlaps a face that's already there (the border
    /// of a flat sheet, not a hole in a solid).
    Covered(Vec<EdgeId>),
    /// An edge still used by three or more faces.
    SharedEdge(EdgeId),
    /// Groups and components placed inside the body (`repair_body` only).
    NestedInstances(usize),
}

/// What a repair did and what it couldn't do. Id lists are in ascending order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RepairReport {
    /// Internal faces deleted (step 1).
    pub removed_internal: Vec<FaceId>,
    /// Stray faces deleted (step 2).
    pub removed_stray: Vec<FaceId>,
    /// Edges deleted because no face used them (step 3), whether loose before the repair or
    /// left loose by steps 1 and 2.
    pub removed_edges: Vec<EdgeId>,
    /// Faces added to fill holes (step 4).
    pub filled: Vec<FaceId>,
    /// Faces turned round (step 5); they keep their ids.
    pub reversed: Vec<FaceId>,
    /// What's left, in the order of the spec.
    pub unfixed: Vec<Unfixed>,
    /// The solid check of the result.
    pub after: SolidReport,
}

impl RepairReport {
    /// True when the repair removed, added or turned anything.
    pub fn changed(&self) -> bool {
        !(self.removed_internal.is_empty()
            && self.removed_stray.is_empty()
            && self.removed_edges.is_empty()
            && self.filled.is_empty()
            && self.reversed.is_empty())
    }
}

/// Why `repair_body` was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepairError {
    /// The model refused a record (also: the body doesn't exist).
    Model(ModelError),
}

enum BorderKind {
    NotOneLoop,
    NotFlat,
    Loop(Loop, Vec<DVec3>),
}

struct Border {
    edges: Vec<EdgeId>,
    kind: BorderKind,
}

fn is_stray(ctx: &Context, f: FaceId) -> bool {
    let Some(face) = ctx.face(f) else {
        return false;
    };
    let mut open = false;
    let mut two = false;
    for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
        for u in &l.uses {
            match ctx.faces_of_edge(u.edge).len() {
                1 => open = true,
                2 => two = true,
                _ => {}
            }
        }
    }
    open && !two
}

fn find_use(ctx: &Context, f: FaceId, e: EdgeId) -> Option<EdgeUse> {
    let face = ctx.face(f)?;
    std::iter::once(&face.outer)
        .chain(face.inner.iter())
        .flat_map(|l| l.uses.iter())
        .find(|u| u.edge == e)
        .copied()
}

fn use_end(ctx: &Context, u: EdgeUse) -> Option<VertexId> {
    let edge = ctx.edge(u.edge)?;
    Some(if u.reversed {
        edge.ends[0]
    } else {
        edge.ends[1]
    })
}

fn undo_since(ctx: &mut Context, log: &mut Edit, len: usize) {
    for record in log.records[len..].iter().rev() {
        let _ = ctx.apply(&record.inverse());
    }
    log.records.truncate(len);
}

fn turn_loop(l: &Loop) -> Loop {
    Loop {
        uses: l
            .uses
            .iter()
            .rev()
            .map(|u| EdgeUse {
                edge: u.edge,
                reversed: !u.reversed,
            })
            .collect(),
    }
}

fn turn_face(face: &Face) -> Face {
    Face {
        outer: turn_loop(&face.outer),
        inner: face.inner.iter().map(turn_loop).collect(),
    }
}

fn border_kind(ctx: &Context, edges: &[EdgeId]) -> BorderKind {
    let mut incident: BTreeMap<VertexId, usize> = BTreeMap::new();
    for &e in edges {
        if let Some(edge) = ctx.edge(e) {
            for end in edge.ends {
                *incident.entry(end).or_insert(0) += 1;
            }
        }
    }
    if incident.values().any(|&n| n != 2) {
        return BorderKind::NotOneLoop;
    }

    let e0 = edges[0];
    let Some(face) = ctx.faces_of_edge(e0).first().copied() else {
        return BorderKind::NotOneLoop;
    };
    let Some(existing) = find_use(ctx, face, e0) else {
        return BorderKind::NotOneLoop;
    };
    let mut current = EdgeUse {
        edge: e0,
        reversed: !existing.reversed,
    };
    let Some(start) = ctx.use_start(current) else {
        return BorderKind::NotOneLoop;
    };

    let mut adjacency: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for &e in edges {
        if let Some(edge) = ctx.edge(e) {
            for end in edge.ends {
                adjacency.entry(end).or_default().push(e);
            }
        }
    }

    let mut uses = Vec::with_capacity(edges.len());
    loop {
        let Some(end) = use_end(ctx, current) else {
            return BorderKind::NotOneLoop;
        };
        uses.push(current);
        if uses.len() == edges.len() {
            if end == start {
                break;
            }
            return BorderKind::NotOneLoop;
        }
        let Some(next) = adjacency
            .get(&end)
            .and_then(|list| list.iter().copied().find(|&e| e != current.edge))
        else {
            return BorderKind::NotOneLoop;
        };
        let Some(edge) = ctx.edge(next) else {
            return BorderKind::NotOneLoop;
        };
        current = EdgeUse {
            edge: next,
            reversed: edge.ends[0] != end,
        };
    }

    let l = Loop { uses };
    let points = ctx.loop_points(&l);
    if geom::is_planar(&points, geom::LINEAR_TOL) {
        BorderKind::Loop(l, points)
    } else {
        BorderKind::NotFlat
    }
}

fn borders(ctx: &Context) -> Vec<Border> {
    let open: Vec<EdgeId> = ctx
        .edges()
        .filter(|(e, _)| ctx.faces_of_edge(*e).len() == 1)
        .map(|(e, _)| e)
        .collect();

    let mut adjacency: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for &e in &open {
        if let Some(edge) = ctx.edge(e) {
            for end in edge.ends {
                adjacency.entry(end).or_default().push(e);
            }
        }
    }

    let mut visited: BTreeSet<EdgeId> = BTreeSet::new();
    let mut out = Vec::new();
    for &first in &open {
        if !visited.insert(first) {
            continue;
        }
        let mut group = Vec::new();
        let mut queue = VecDeque::from([first]);
        while let Some(e) = queue.pop_front() {
            group.push(e);
            if let Some(edge) = ctx.edge(e) {
                for end in edge.ends {
                    if let Some(list) = adjacency.get(&end) {
                        for &other in list {
                            if visited.insert(other) {
                                queue.push_back(other);
                            }
                        }
                    }
                }
            }
        }
        group.sort_unstable();
        let kind = border_kind(ctx, &group);
        out.push(Border { edges: group, kind });
    }
    out.sort_by_key(|b| b.edges[0]);
    out
}

fn loop_inside(a: &[DVec3], b: &[DVec3]) -> bool {
    let Some(plane) = geom::fit_plane(a) else {
        return false;
    };
    if !b
        .iter()
        .all(|p| plane.signed_distance(*p).abs() <= geom::LINEAR_TOL)
    {
        return false;
    }
    let a2 = geom::to_plane_2d(a, plane.normal);
    let b2 = geom::to_plane_2d(b, plane.normal);
    b2.iter()
        .any(|p| geom::point_in_polygon_2d(*p, &a2, geom::LINEAR_TOL) != Containment::Outside)
}

fn loops_nested(a: &[DVec3], b: &[DVec3]) -> bool {
    loop_inside(a, b) || loop_inside(b, a)
}

fn fill_border(ctx: &mut Context, log: &mut Edit, border: &Border) -> Result<FaceId, Unfixed> {
    let BorderKind::Loop(outer, _) = &border.kind else {
        return Err(Unfixed::BadFill(border.edges.clone()));
    };
    let start = log.records.len();
    let Ok(new) = ctx.insert_face(
        Face {
            outer: outer.clone(),
            inner: vec![],
        },
        log,
    ) else {
        return Err(Unfixed::BadFill(border.edges.clone()));
    };
    let bad = ctx.face_violation(new).is_some();
    let covered = !bad
        && ctx
            .faces()
            .any(|(other, _)| other != new && crate::faces::faces_overlap(ctx, new, other));
    if bad || covered {
        undo_since(ctx, log, start);
        return Err(if bad {
            Unfixed::BadFill(border.edges.clone())
        } else {
            Unfixed::Covered(border.edges.clone())
        });
    }
    Ok(new)
}

/// Repair the geometry of one context, appending every change to `log`.
pub fn repair_geometry(ctx: &mut Context, log: &mut Edit) -> RepairReport {
    let mut report = RepairReport::default();

    // 1. Internal faces.
    for f in crate::solid::check_geometry(ctx).internal_faces {
        if ctx.face(f).is_some() {
            ctx.delete_face(f, log)
                .expect("internal face can be deleted");
            report.removed_internal.push(f);
        }
    }

    // 2. Stray faces.
    let mut stray = Vec::new();
    let mut any_kept = false;
    for (f, _) in ctx.faces() {
        if is_stray(ctx, f) {
            stray.push(f);
        } else {
            any_kept = true;
        }
    }
    if any_kept {
        for f in stray {
            if ctx.face(f).is_some() {
                ctx.delete_face(f, log).expect("stray face can be deleted");
                report.removed_stray.push(f);
            }
        }
    }

    // 3. Loose edges, then vertices no edge uses.
    let loose: Vec<EdgeId> = ctx
        .edges()
        .filter(|(e, _)| ctx.faces_of_edge(*e).is_empty())
        .map(|(e, _)| e)
        .collect();
    for e in loose {
        ctx.delete_edge(e, log).expect("loose edge can be deleted");
        report.removed_edges.push(e);
    }
    let free: Vec<VertexId> = ctx
        .vertices()
        .filter(|(v, _)| ctx.edges_of_vertex(*v).is_empty())
        .map(|(v, _)| v)
        .collect();
    for v in free {
        ctx.delete_vertex(v, log)
            .expect("vertex without edges can be deleted");
    }

    // 4. Holes.
    let borders = borders(ctx);
    let mut nested = vec![false; borders.len()];
    for i in 0..borders.len() {
        for j in i + 1..borders.len() {
            let pair_nested = match (&borders[i].kind, &borders[j].kind) {
                (BorderKind::Loop(_, a), BorderKind::Loop(_, b)) => loops_nested(a, b),
                _ => false,
            };
            if pair_nested {
                nested[i] = true;
                nested[j] = true;
            }
        }
    }
    for (i, border) in borders.iter().enumerate() {
        match &border.kind {
            BorderKind::NotOneLoop => {
                report
                    .unfixed
                    .push(Unfixed::NotOneLoop(border.edges.clone()));
            }
            BorderKind::NotFlat => {
                report.unfixed.push(Unfixed::NotFlat(border.edges.clone()));
            }
            BorderKind::Loop(_, _) => {
                if nested[i] {
                    report.unfixed.push(Unfixed::Nested(border.edges.clone()));
                } else {
                    match fill_border(ctx, log, border) {
                        Ok(new) => report.filled.push(new),
                        Err(unfixed) => report.unfixed.push(unfixed),
                    }
                }
            }
        }
    }

    // 5. Reversed faces.
    for f in crate::solid::check_geometry(ctx).reversed_faces {
        let Some(old) = ctx.face(f).cloned() else {
            continue;
        };
        let turned = turn_face(&old);
        let remove = Record::RemoveFace(f, old);
        ctx.apply(&remove).expect("face can be removed");
        log.records.push(remove);
        let add = Record::AddFace(f, turned);
        ctx.apply(&add).expect("face can be added back");
        log.records.push(add);
        report.reversed.push(f);
    }

    // 6. What's left.
    let after = crate::solid::check_geometry(ctx);
    for &e in &after.shared_edges {
        report.unfixed.push(Unfixed::SharedEdge(e));
    }
    report.after = after;
    report
}

/// Repair the body's geometry, carry the paint of turned faces to their new sides and push
/// one undo step named `label`. Pushes nothing and leaves the model alone when nothing
/// changes.
pub fn repair_body(
    model: &mut Model,
    body: BodyId,
    undo: &mut UndoStack,
    label: &str,
) -> Result<RepairReport, RepairError> {
    let Some(current) = model.body(body) else {
        return Err(RepairError::Model(ModelError::MissingBody(body)));
    };
    let instances = current.instances().count();
    let mut probe = current.geometry().clone();

    let mut scratch = Edit::new();
    let mut report = repair_geometry(&mut probe, &mut scratch);
    if scratch.is_empty() {
        if instances > 0 {
            report.unfixed.push(Unfixed::NestedInstances(instances));
            report.after.nested_instances = instances;
            report.after.volume = None;
        }
        return Ok(report);
    }

    let mut log = ModelEdit::new();
    let mut report = model
        .edit_geometry(body, &mut log, repair_geometry)
        .map_err(RepairError::Model)?;

    for &f in &report.reversed {
        let Some(current) = model.body(body) else {
            let _ = model.apply_edit(&log.inverse());
            return Err(RepairError::Model(ModelError::MissingBody(body)));
        };
        let materials = current.face_materials(f);
        if materials == FaceMaterials::default() {
            continue;
        }
        let swapped = FaceMaterials {
            front: materials.back,
            back: materials.front,
            front_map: materials.back_map,
            back_map: materials.front_map,
        };
        if let Err(error) = model.set_face_materials(body, f, swapped, &mut log) {
            let _ = model.apply_edit(&log.inverse());
            return Err(RepairError::Model(error));
        }
    }

    if instances > 0 {
        report.unfixed.push(Unfixed::NestedInstances(instances));
        report.after.nested_instances = instances;
        report.after.volume = None;
    }

    undo.push(label, log);
    Ok(report)
}
