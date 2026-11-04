// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Properties of offsetting a face in its plane (`docs/specs/m9-offset-face.md`).

use std::f64::consts::TAU;

use drydock_kernel::faceoffset::*;
use drydock_kernel::geom;
use drydock_kernel::model::*;
use drydock_kernel::topology::*;
use glam::{DMat4, DVec3};
use proptest::prelude::*;

/// One face through `pts` (counter-clockwise about its front normal) at the root.
fn polygon(model: &mut Model, pts: &[DVec3]) -> FaceId {
    let mut edit = ModelEdit::new();
    let f = model
        .edit_geometry(BodyId::Root, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = pts.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            let mut uses = Vec::new();
            for i in 0..vs.len() {
                let e = ctx.insert_edge(vs[i], vs[(i + 1) % vs.len()], log).unwrap();
                uses.push(EdgeUse::forward(e));
            }
            ctx.insert_face(
                Face {
                    outer: Loop { uses },
                    inner: Vec::new(),
                },
                log,
            )
            .unwrap()
        })
        .unwrap();
    assert!(model.root().geometry().validate().is_empty());
    f
}

fn area(pts: &[DVec3]) -> f64 {
    geom::area_vector(pts).length()
}

fn face_area(c: &Context, f: FaceId) -> f64 {
    let face = c.face(f).unwrap();
    area(&c.loop_points(&face.outer))
        - face
            .inner
            .iter()
            .map(|l| area(&c.loop_points(l)))
            .sum::<f64>()
}

/// A convex polygon: 5..=10 points on a circle of radius `r`, the gaps between them weighted
/// 1..2 (so each gap is at most 120 degrees and each corner at least 60), placed in a plane
/// turned by `tilt` about x and `turn` about z and moved by `shift`.
fn convex() -> impl Strategy<Value = (Vec<DVec3>, f64)> {
    (
        prop::collection::vec(1.0f64..2.0, 5..=10),
        0.5f64..5.0,
        0.0f64..TAU,
        -1.5f64..1.5,
        0.0f64..TAU,
        prop::array::uniform3(-5.0f64..5.0),
    )
        .prop_map(|(weights, r, phase, tilt, turn, shift)| {
            let total: f64 = weights.iter().sum();
            let m = DMat4::from_translation(DVec3::from(shift))
                * DMat4::from_rotation_z(turn)
                * DMat4::from_rotation_x(tilt);
            let mut a = phase;
            let mut pts = Vec::new();
            for w in &weights {
                pts.push(m.transform_point3(DVec3::new(r * a.cos(), r * a.sin(), 0.0)));
                a += w / total * TAU;
            }
            (pts, r)
        })
}

/// A distance from -0.08 r to 0.5 r, at least 0.001 r from zero. Inward 0.08 r is safe for
/// `convex`: its shortest edge is over 0.33 r and an edge shrinks by at most 3.47 d.
fn distance(r: f64) -> impl Strategy<Value = f64> {
    (-0.08f64..0.5).prop_filter_map("too small", move |t| (t.abs() >= 0.001).then_some(t * r))
}

/// A star-shaped polygon about the origin on the ground: 5..=12 points at sorted angles with
/// radii 0.6..1.0. May have reflex and sharp corners.
fn star() -> impl Strategy<Value = Vec<DVec3>> {
    prop::collection::vec((1.0f64..2.0, 0.6f64..1.0), 5..=12).prop_map(|spokes| {
        let total: f64 = spokes.iter().map(|(w, _)| w).sum();
        let mut a: f64 = 0.0;
        let mut pts = Vec::new();
        for (w, r) in &spokes {
            pts.push(DVec3::new(r * a.cos(), r * a.sin(), 0.0));
            a += w / total * TAU;
        }
        pts
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn convex_offset_then_back_restores_the_polygon(
        (pts, d) in convex().prop_flat_map(|(pts, r)| (Just(pts), distance(r)))
    ) {
        let mut m = Model::new();
        let f = polygon(&mut m, &pts);
        let out = offset_loops(m.root().geometry(), f, d).unwrap();
        prop_assert_eq!(out.len(), 1);
        prop_assert_eq!(out[0].len(), pts.len());

        let mut back = Model::new();
        let g = polygon(&mut back, &out[0]);
        let again = offset_loops(back.root().geometry(), g, -d).unwrap();
        prop_assert_eq!(again[0].len(), pts.len());
        for (a, b) in again[0].iter().zip(&pts) {
            prop_assert!(a.distance(*b) < 1e-7, "{:?} != {:?}", a, b);
        }
        prop_assert!((area(&again[0]) - area(&pts)).abs() < 1e-7 * area(&pts).max(1.0));
    }

    #[test]
    fn convex_offset_edges_are_parallel_at_the_distance(
        (pts, d) in convex().prop_flat_map(|(pts, r)| (Just(pts), distance(r)))
    ) {
        let mut m = Model::new();
        let f = polygon(&mut m, &pts);
        let n = m.root().geometry().face_plane(f).unwrap().normal;
        let out = &offset_loops(m.root().geometry(), f, d).unwrap()[0];
        let k = pts.len();
        for j in 0..k {
            let (a, b) = (pts[j], pts[(j + 1) % k]);
            let (p, q) = (out[j], out[(j + 1) % k]);
            let e = (b - a).normalize();
            // Same direction.
            prop_assert!((q - p).normalize().dot(e) > 1.0 - 1e-9);
            // Moved by d along the outward side (e x n) of the old edge, in the plane.
            let side = e.cross(n);
            prop_assert!(((p - a).dot(side) - d).abs() < 1e-9);
            prop_assert!(((q - a).dot(side) - d).abs() < 1e-9);
            prop_assert!((p - a).dot(n).abs() < 1e-9);
        }
    }

    #[test]
    fn convex_area_grows_with_the_distance(
        (pts, d1, d2) in convex().prop_flat_map(|(pts, r)| (Just(pts), distance(r), distance(r)))
    ) {
        prop_assume!((d1 - d2).abs() > 1e-6);
        let (lo, hi) = if d1 < d2 { (d1, d2) } else { (d2, d1) };
        let mut m = Model::new();
        let f = polygon(&mut m, &pts);
        let c = m.root().geometry();
        let a_lo = area(&offset_loops(c, f, lo).unwrap()[0]);
        let a_hi = area(&offset_loops(c, f, hi).unwrap()[0]);
        let a0 = area(&pts);
        prop_assert!(a_lo < a_hi);
        prop_assert_eq!(lo < 0.0, a_lo < a0);
        prop_assert_eq!(hi > 0.0, a_hi > a0);
    }

    #[test]
    fn star_area_grows_with_the_distance(pts in star(), t1 in -0.2f64..0.2, t2 in -0.2f64..0.2) {
        prop_assume!(t1.abs() > 1e-3 && t2.abs() > 1e-3 && (t1 - t2).abs() > 1e-4);
        let (lo, hi) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
        let mut m = Model::new();
        let f = polygon(&mut m, &pts);
        let c = m.root().geometry();
        let (Ok(a), Ok(b)) = (offset_loops(c, f, lo), offset_loops(c, f, hi)) else {
            return Ok(());
        };
        prop_assert!(area(&a[0]) < area(&b[0]));
    }

    #[test]
    fn offset_face_splits_area_and_undoes(
        (pts, d) in convex().prop_flat_map(|(pts, r)| (Just(pts), distance(r)))
    ) {
        let mut m = Model::new();
        let f = polygon(&mut m, &pts);
        let n = m.root().geometry().face_plane(f).unwrap().normal;
        let a0 = area(&pts);
        let loops = offset_loops(m.root().geometry(), f, d).unwrap();
        let before = m.clone();
        let mut undo = UndoStack::new();
        let r = offset_face(&mut m, BodyId::Root, f, d, &mut undo, "Offset").unwrap();
        let c = m.root().geometry();
        prop_assert!(c.validate().is_empty(), "{:?}", c.validate());
        prop_assert!(c.face_overlaps().is_empty());
        prop_assert_eq!(c.face_count(), 2);
        prop_assert!(r.hole_rings.is_empty());
        let a1 = area(&loops[0]);
        let tol = 1e-7 * a0.max(a1).max(1.0);
        if d < 0.0 {
            prop_assert_eq!(r.ring, f);
            prop_assert!((face_area(c, r.inner) - a1).abs() < tol);
            prop_assert!((face_area(c, r.inner) + face_area(c, r.ring) - a0).abs() < tol);
        } else {
            prop_assert_eq!(r.inner, f);
            prop_assert!((face_area(c, r.ring) - (a1 - a0)).abs() < tol);
        }
        for g in [r.inner, r.ring] {
            prop_assert!(c.face_plane(g).unwrap().normal.dot(n) > 1.0 - 1e-9);
        }
        let after = m.clone();
        undo.undo(&mut m).unwrap();
        prop_assert!(m == before);
        undo.redo(&mut m).unwrap();
        prop_assert!(m == after);
    }

    #[test]
    fn refusals_change_nothing(pts in star(), t in -1.5f64..1.5) {
        prop_assume!(t.abs() > 1e-3);
        let mut m = Model::new();
        let f = polygon(&mut m, &pts);
        let before = m.clone();
        let mut undo = UndoStack::new();
        match offset_face(&mut m, BodyId::Root, f, t, &mut undo, "Offset") {
            Ok(_) => {
                prop_assert!(m.root().geometry().validate().is_empty());
                prop_assert_eq!(undo.undo_count(), 1);
            }
            Err(_) => {
                prop_assert!(m == before);
                prop_assert_eq!(undo.undo_count(), 0);
            }
        }
    }
}
