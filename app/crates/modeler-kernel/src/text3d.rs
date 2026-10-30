// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3D text: flat letter loops drawn on a face, then raised out of it (emboss) or sunk into it
//! (engrave) with push/pull, in one undo step.
//!
//! Behaviour: `docs/specs/m7-text-3d.md`.

use std::collections::BTreeSet;
use std::fmt;

use glam::{DVec2, DVec3};

use crate::geom::{self, Containment, SegmentHit};
use crate::model::{BodyId, Model, ModelEdit, ModelError, UndoStack};
use crate::offset::edge_pierces;
use crate::pushpull::{PushPullError, push_pull_into};
use crate::repair::{RepairReport, repair_geometry};
use crate::solid::{SolidReport, check_geometry};
use crate::sticky::StickyError;
use crate::topology::{Context, EdgeId, FaceId};

/// Which way the letters go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextMode {
    /// Raised out of the face, away from the material.
    Emboss,
    /// Sunk into the material behind the face.
    Engrave,
}

/// Why [`text_3d`] or [`check_text`] refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum Text3dError {
    MissingBody(BodyId),
    /// No such face, or the face has no plane.
    MissingFace(FaceId),
    /// The height is not finite or not longer than `LINEAR_TOL`.
    BadHeight,
    /// No loops.
    NoText,
    /// Loop `i` has fewer than 3 points or is not a simple polygon.
    BadLoop(usize),
    /// A point of loop `i` is off the face's plane.
    OffPlane(usize),
    /// Loops `i < j` cross or touch.
    LoopsCross(usize, usize),
    /// The text is longer than the face however it is turned.
    TooSmall,
    /// Loop `i` leaves the face or meets its border.
    CrossesBorder(usize),
    /// The engrave is at least as deep as the part is thick under the text.
    TooDeep {
        thickness: f64,
    },
    /// The letters would cut into other geometry of the body.
    Intersects,
    /// Drawing the loops on the face failed.
    Draw(StickyError),
    /// Push/pull refused a letter face.
    PushPull(PushPullError),
    /// The body was a solid before and repair couldn't make the result one.
    NotSolid(Box<SolidReport>),
    Model(ModelError),
}

impl fmt::Display for Text3dError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Text3dError::MissingBody(_) => write!(f, "the part is gone"),
            Text3dError::MissingFace(_) => write!(f, "pick a flat face to put the text on"),
            Text3dError::BadHeight => write!(f, "the height must be a length above zero"),
            Text3dError::NoText => write!(f, "there is no text to place"),
            Text3dError::BadLoop(_) => write!(f, "a letter outline crosses itself"),
            Text3dError::OffPlane(_) => write!(f, "the text is not flat on the face"),
            Text3dError::LoopsCross(_, _) => write!(f, "two letters touch or overlap"),
            Text3dError::TooSmall => write!(f, "the face is too small for the text"),
            Text3dError::CrossesBorder(_) => write!(f, "the text runs over the edge of the face"),
            Text3dError::TooDeep { .. } => {
                write!(f, "the engraving is as deep as the part is thick")
            }
            Text3dError::Intersects => write!(f, "the letters would cut into other geometry"),
            Text3dError::Draw(_) => write!(f, "the letters could not be drawn on the face"),
            Text3dError::PushPull(_) => write!(f, "push/pull refused a letter"),
            Text3dError::NotSolid(_) => write!(f, "the part would no longer be a solid"),
            Text3dError::Model(_) => write!(f, "the model could not be changed"),
        }
    }
}

impl std::error::Error for Text3dError {}

/// What [`text_3d`] made.
#[derive(Clone, Debug, PartialEq)]
pub struct TextFaces {
    /// Each letter face at its new place (top of a raised letter, floor of a sunk one), in
    /// loop order.
    pub caps: Vec<FaceId>,
    /// The new side faces, letter after letter, each in push/pull's order.
    pub sides: Vec<FaceId>,
    /// What repair did, when the result needed it.
    pub repaired: Option<RepairReport>,
}

/// Checks that `loops` can be embossed on or engraved into `face` of `ctx` by `height`,
/// without changing anything.
pub fn check_text(
    ctx: &Context,
    face: FaceId,
    loops: &[Vec<DVec3>],
    mode: TextMode,
    height: f64,
) -> Result<(), Text3dError> {
    let Some(face_data) = ctx.face(face) else {
        return Err(Text3dError::MissingFace(face));
    };
    let Some(plane) = ctx.face_plane(face) else {
        return Err(Text3dError::MissingFace(face));
    };
    if !height.is_finite() || height <= geom::LINEAR_TOL {
        return Err(Text3dError::BadHeight);
    }
    if loops.is_empty() {
        return Err(Text3dError::NoText);
    }

    let n = plane.normal;
    let projected: Vec<Vec<DVec2>> = loops.iter().map(|l| geom::to_plane_2d(l, n)).collect();

    for (i, l) in loops.iter().enumerate() {
        if l.len() < 3 {
            return Err(Text3dError::BadLoop(i));
        }
        if l.iter()
            .any(|p| plane.signed_distance(*p).abs() > geom::LINEAR_TOL)
        {
            return Err(Text3dError::OffPlane(i));
        }
        if !geom::polygon_is_simple_2d(&projected[i], geom::LINEAR_TOL) {
            return Err(Text3dError::BadLoop(i));
        }
    }

    for i in 0..loops.len() {
        for j in i + 1..loops.len() {
            if loops_touch(&projected[i], &projected[j]) {
                return Err(Text3dError::LoopsCross(i, j));
            }
        }
    }

    let text_points: Vec<DVec3> = loops.iter().flatten().copied().collect();
    let face_outer = ctx.loop_points(&face_data.outer);
    if diameter(&text_points) >= diameter(&face_outer) - geom::LINEAR_TOL {
        return Err(Text3dError::TooSmall);
    }

    let face_loops_3d: Vec<Vec<DVec3>> = std::iter::once(&face_data.outer)
        .chain(face_data.inner.iter())
        .map(|l| ctx.loop_points(l))
        .collect();
    let face_loops: Vec<Vec<DVec2>> = face_loops_3d
        .iter()
        .map(|l| geom::to_plane_2d(l, n))
        .collect();
    let Some(outer) = face_loops.first() else {
        return Err(Text3dError::MissingFace(face));
    };
    for (i, l) in loops.iter().enumerate() {
        for p in l {
            let p = geom::to_plane_2d(&[*p], n)[0];
            if geom::point_in_polygon_2d(p, outer, geom::LINEAR_TOL) != Containment::Inside {
                return Err(Text3dError::CrossesBorder(i));
            }
            let in_hole = face_loops[1..]
                .iter()
                .any(|h| geom::point_in_polygon_2d(p, h, geom::LINEAR_TOL) != Containment::Outside);
            if in_hole {
                return Err(Text3dError::CrossesBorder(i));
            }
        }
        if face_loops.iter().any(|f| loops_touch(&projected[i], f)) {
            return Err(Text3dError::CrossesBorder(i));
        }
    }

    if mode == TextMode::Engrave {
        let out = if check_geometry(ctx).reversed_faces.contains(&face) {
            -n
        } else {
            n
        };
        let others: Vec<(geom::Plane, Vec<DVec2>, Vec<Vec<DVec2>>)> = ctx
            .faces()
            .filter(|(g, _)| *g != face)
            .filter_map(|(g, data)| {
                let plane = ctx.face_plane(g)?;
                let outer = geom::to_plane_2d(&ctx.loop_points(&data.outer), plane.normal);
                let holes = data
                    .inner
                    .iter()
                    .map(|h| geom::to_plane_2d(&ctx.loop_points(h), plane.normal))
                    .collect();
                Some((plane, outer, holes))
            })
            .collect();
        let mut thickness = f64::INFINITY;
        for l in loops {
            for p in l {
                for (plane, outer, holes) in &others {
                    let denom = plane.normal.dot(out);
                    if denom.abs() < 1e-9 {
                        continue;
                    }
                    let t = plane.normal.dot(*p - plane.point) / denom;
                    if t <= geom::LINEAR_TOL {
                        continue;
                    }
                    let q = *p - out * t;
                    let q = geom::to_plane_2d(&[q], plane.normal)[0];
                    if geom::point_in_polygon_2d(q, outer, geom::LINEAR_TOL) == Containment::Outside
                    {
                        continue;
                    }
                    if holes.iter().any(|h| {
                        geom::point_in_polygon_2d(q, h, geom::LINEAR_TOL) == Containment::Inside
                    }) {
                        continue;
                    }
                    thickness = thickness.min(t);
                }
            }
        }
        if height >= thickness - geom::LINEAR_TOL {
            return Err(Text3dError::TooDeep { thickness });
        }
    }

    Ok(())
}

/// Draws `loops` (closed polygons on `face`'s plane, outer loops and holes told apart by
/// nesting) on `face` of `body` and pushes the letter faces out by `height` (emboss) or in
/// (engrave), as one undo step labelled `label`.
#[allow(clippy::too_many_arguments)]
pub fn text_3d(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    loops: &[Vec<DVec3>],
    mode: TextMode,
    height: f64,
    undo: &mut UndoStack,
    label: &str,
) -> Result<TextFaces, Text3dError> {
    let mut edit = ModelEdit::new();
    match text_3d_inner(model, body, face, loops, mode, height, &mut edit) {
        Ok(made) => {
            undo.push(label, edit);
            Ok(made)
        }
        Err(e) => {
            if !edit.is_empty() {
                let _ = model.apply_edit(&edit.inverse());
            }
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn text_3d_inner(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    loops: &[Vec<DVec3>],
    mode: TextMode,
    height: f64,
    edit: &mut ModelEdit,
) -> Result<TextFaces, Text3dError> {
    let ctx = model
        .body(body)
        .ok_or(Text3dError::MissingBody(body))?
        .geometry();
    check_text(ctx, face, loops, mode, height)?;

    let before = check_geometry(ctx);
    let before_edges: BTreeSet<EdgeId> = ctx.edges().map(|(e, _)| e).collect();
    let before_faces: BTreeSet<FaceId> = ctx.faces().map(|(f, _)| f).collect();
    let n = ctx
        .face_plane(face)
        .ok_or(Text3dError::MissingFace(face))?
        .normal;
    let mut distance = if mode == TextMode::Engrave {
        -height
    } else {
        height
    };
    if before.reversed_faces.contains(&face) {
        distance = -distance;
    }

    let projected: Vec<Vec<DVec2>> = loops.iter().map(|l| geom::to_plane_2d(l, n)).collect();
    let depths: Vec<usize> = (0..loops.len())
        .map(|i| {
            let p = projected[i][0];
            (0..loops.len())
                .filter(|&j| {
                    j != i
                        && geom::point_in_polygon_2d(p, &projected[j], geom::LINEAR_TOL)
                            == Containment::Inside
                })
                .count()
        })
        .collect();

    let drawn = model.edit_geometry(body, edit, |ctx, log| {
        let mut edges = Vec::with_capacity(loops.len());
        for l in loops {
            let mut set = BTreeSet::new();
            for i in 0..l.len() {
                let chain = ctx.add_edge(l[i], l[(i + 1) % l.len()], log)?;
                set.extend(chain.edges);
            }
            edges.push(set);
        }
        Ok::<_, StickyError>(edges)
    });
    let loop_edges = match drawn {
        Ok(Ok(edges)) => edges,
        Ok(Err(e)) => return Err(Text3dError::Draw(e)),
        Err(e) => return Err(Text3dError::Model(e)),
    };

    let mut letter_faces = Vec::new();
    {
        let ctx = model
            .body(body)
            .ok_or(Text3dError::MissingBody(body))?
            .geometry();
        for (i, depth) in depths.iter().enumerate() {
            if depth % 2 != 0 {
                continue;
            }
            let mut found = None;
            for (f, data) in ctx.faces() {
                let outer: BTreeSet<EdgeId> = data.outer.uses.iter().map(|u| u.edge).collect();
                if outer == loop_edges[i] {
                    found = Some(f);
                    break;
                }
            }
            match found {
                Some(f) => letter_faces.push(f),
                None => return Err(Text3dError::BadLoop(i)),
            }
        }
    }

    let mut caps = Vec::new();
    let mut sides = Vec::new();
    for f in letter_faces {
        let pulled =
            push_pull_into(model, body, f, distance, edit).map_err(Text3dError::PushPull)?;
        caps.push(pulled.cap);
        sides.extend(pulled.sides);
    }

    {
        let ctx = model
            .body(body)
            .ok_or(Text3dError::MissingBody(body))?
            .geometry();
        let edges: Vec<EdgeId> = ctx.edges().map(|(e, _)| e).collect();
        let faces: Vec<FaceId> = ctx.faces().map(|(f, _)| f).collect();
        let new_edges: Vec<EdgeId> = edges
            .iter()
            .copied()
            .filter(|e| !before_edges.contains(e))
            .collect();
        let new_faces: Vec<FaceId> = faces
            .iter()
            .copied()
            .filter(|f| !before_faces.contains(f))
            .collect();
        let pierces = new_edges
            .iter()
            .any(|&e| faces.iter().any(|&f| edge_pierces(ctx, e, f)))
            || edges
                .iter()
                .any(|&e| new_faces.iter().any(|&f| edge_pierces(ctx, e, f)));
        if pierces {
            return Err(Text3dError::Intersects);
        }
    }

    let after = check_geometry(
        model
            .body(body)
            .ok_or(Text3dError::MissingBody(body))?
            .geometry(),
    );
    let good = if before.is_clean() {
        after.is_clean()
    } else if before.is_solid() {
        after.is_solid()
    } else {
        true
    };
    let repaired = if good {
        None
    } else {
        let report = model
            .edit_geometry(body, edit, repair_geometry)
            .map_err(Text3dError::Model)?;
        let fixed = if before.is_clean() {
            report.after.is_clean()
        } else {
            report.after.is_solid()
        };
        if fixed {
            Some(report)
        } else {
            return Err(Text3dError::NotSolid(Box::new(report.after)));
        }
    };

    Ok(TextFaces {
        caps,
        sides,
        repaired,
    })
}

fn poly_bounds(poly: &[DVec2]) -> (DVec2, DVec2) {
    let mut lo = DVec2::splat(f64::INFINITY);
    let mut hi = DVec2::splat(f64::NEG_INFINITY);
    for p in poly {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    (lo, hi)
}

fn loops_touch(a: &[DVec2], b: &[DVec2]) -> bool {
    if a.len() < 2 || b.len() < 2 {
        return false;
    }
    let (alo, ahi) = poly_bounds(a);
    let (blo, bhi) = poly_bounds(b);
    let tol = geom::LINEAR_TOL;
    if alo.x > bhi.x + tol || ahi.x < blo.x - tol || alo.y > bhi.y + tol || ahi.y < blo.y - tol {
        return false;
    }
    let at = |p: DVec2| DVec3::new(p.x, p.y, 0.0);
    for i in 0..a.len() {
        let a0 = at(a[i]);
        let a1 = at(a[(i + 1) % a.len()]);
        for j in 0..b.len() {
            let b0 = at(b[j]);
            let b1 = at(b[(j + 1) % b.len()]);
            if geom::segment_intersection(a0, a1, b0, b1, tol) != SegmentHit::None {
                return true;
            }
        }
    }
    false
}

fn diameter(points: &[DVec3]) -> f64 {
    let mut best = 0.0f64;
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            best = best.max(points[i].distance_squared(points[j]));
        }
    }
    best.sqrt()
}
