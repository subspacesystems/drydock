// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Property tests for curves carried through `sweep_face` (`docs/specs/m9-sweep-curves.md`).

use drydock_kernel::curve::{self, Curve};
use drydock_kernel::model::*;
use drydock_kernel::shapes;
use drydock_kernel::sweep::SweepPath;
use drydock_kernel::topology::*;
use glam::{DQuat, DVec3};
use proptest::prelude::*;

mod pushpull_common;
use pushpull_common::*;
mod sweep_face_common;
use sweep_face_common::*;

fn rotation() -> impl Strategy<Value = DQuat> {
    (
        -1.0f64..1.0,
        -1.0f64..1.0,
        -1.0f64..1.0,
        0.0f64..std::f64::consts::TAU,
    )
        .prop_filter_map("axis", |(x, y, z, a)| {
            let axis = v(x, y, z);
            (axis.length() > 0.1).then(|| DQuat::from_axis_angle(axis.normalize(), a))
        })
}

fn add_curve(m: &mut Model, pts: &[DVec3], closed: bool) -> CurveId {
    let mut edit = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut edit, |c, log| {
        curve::add_curve(c, pts, closed, log)
    })
    .unwrap()
    .expect("the points make a curve")
}

/// Loose edges through `pts` in the root body, as a path of the body's own ids.
fn body_path(m: &mut Model, pts: &[DVec3], closed: bool) -> SweepPath {
    let mut edit = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut edit, |c, log| {
        let vs: Vec<VertexId> = pts.iter().map(|p| c.insert_vertex(*p, log)).collect();
        let n = vs.len();
        let count = if closed { n } else { n - 1 };
        let edges = (0..count)
            .map(|i| c.insert_edge(vs[i], vs[(i + 1) % n], log).unwrap())
            .collect();
        SweepPath {
            vertices: vs,
            points: pts.to_vec(),
            edges,
            closed,
        }
    })
    .unwrap()
}

fn curves(c: &Context) -> Vec<(CurveId, Curve)> {
    c.curves().map(|(id, cv)| (id, cv.clone())).collect()
}

fn assert_curves_sound(c: &Context) {
    let mut seen = std::collections::BTreeSet::new();
    for (id, cv) in c.curves() {
        assert!(curve::fits(c, cv), "curve {id:?} doesn't fit");
        let edges = curve::curve_edges(c, id);
        assert_eq!(edges.len(), cv.vertices.len() - 1);
        for e in edges {
            assert!(seen.insert(e), "edge {e:?} is on two curves");
        }
    }
}

fn parallel(a: DVec3, b: DVec3) -> bool {
    a.normalize().cross(b.normalize()).length() < 1e-6
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// A profile swept round an arc (or a whole circle) of the path, standing in the plane
    /// through the arc's axis at its first vertex: one curve round the axis through each
    /// profile vertex, and for a round profile one circle per section that is a circle. Undo
    /// takes them all away again, redo brings back the same.
    #[test]
    fn sweeps_round_an_arc_carry_curves_and_undo_cleanly(
        q in rotation(),
        radius in 1.0f64..3.0,
        segments in 3usize..13,
        step in 0.2f64..0.9,
        closed in any::<bool>(),
        round in any::<bool>(),
        half_sides in 3usize..7,
        size in 0.05f64..0.15,
        out in 0.2f64..0.6,
        height in 0.2f64..0.6,
    ) {
        let n = if closed { segments + 3 } else { segments };
        prop_assume!(closed || n as f64 * step <= 5.0);
        let o = q * v(1.0, -2.0, 0.5);
        let axis = q * DVec3::Z;
        let x = q * DVec3::X;
        let y = q * DVec3::Y;
        let pts = if closed {
            shapes::circle(o, axis, x, radius, n)
        } else {
            shapes::arc(o, axis, x, radius, 0.0, n as f64 * step, n)
        };
        let mut m = Model::new();
        let mut undo = UndoStack::new();
        let p = body_path(&mut m, &pts, closed);
        let path_curve = add_curve(&mut m, &pts, closed);
        let old_path_curve = ctx(&m).curve(path_curve).unwrap().clone();

        // The profile, in the plane through the axis and the path's first vertex.
        let centre = o + x * (radius + out) + axis * height;
        // An even number of sides from the top, so no side is level: level sides of
        // neighbouring steps would merge and split the section circles into arcs.
        let profile_pts = if round {
            shapes::circle(centre, y, axis, size, 2 * half_sides)
        } else {
            rect_points(centre, x, axis, -size, size, -size, size).to_vec()
        };
        let idx: Vec<usize> = (0..profile_pts.len()).collect();
        let f = mesh(&mut m, &profile_pts, &[&[&idx]])[0];
        if round {
            add_curve(&mut m, &profile_pts, true);
        }

        let before = m.clone();
        let before_curves = curves(ctx(&m));
        sweep(&mut m, &mut undo, f, &p).unwrap();
        let after = m.clone();
        let c = ctx(&m);
        assert_curves_sound(c);
        prop_assert_eq!(c.curve(path_curve), Some(&old_path_curve));

        let new: Vec<Curve> = curves(c)
            .into_iter()
            .filter(|(id, _)| before_curves.iter().all(|(b, _)| b != id))
            .map(|(_, cv)| cv)
            .collect();
        let along: Vec<&Curve> = new
            .iter()
            .filter(|cv| parallel(cv.normal, axis) && cv.radius > radius)
            .collect();
        prop_assert_eq!(along.len(), profile_pts.len());
        for cv in &along {
            prop_assert_eq!(cv.is_closed(), closed);
            // Open: every section but the end cap, which is square to the last step.
            prop_assert_eq!(cv.vertices.len() - 1, if closed { n } else { n - 1 });
            let to_centre = cv.center - o;
            prop_assert!(to_centre.cross(axis).length() < 1e-6);
        }
        // Round: every section but the profile's own and (open) the end cap is a circle.
        let section_curves = new.len() - along.len();
        prop_assert_eq!(section_curves, if round { n - 1 } else { 0 });

        undo.undo(&mut m).unwrap();
        prop_assert_eq!(&m, &before);
        prop_assert_eq!(curves(ctx(&m)), before_curves);
        undo.redo(&mut m).unwrap();
        prop_assert_eq!(&m, &after);
    }

    /// A round profile swept straight, square to the path: the end cap's loop is the same
    /// circle moved along the path, and nothing else is a curve.
    #[test]
    fn a_straight_sweep_moves_the_circle_to_the_end_cap(
        q in rotation(),
        r in 0.05f64..2.0,
        segments in 3usize..25,
        len in 0.1f64..5.0,
        start in -3.0f64..3.0,
    ) {
        let n = q * DVec3::X;
        let o = q * v(0.3, -0.2, 0.1);
        let pts = shapes::circle(o, n, q * DVec3::Y, r, segments);
        let mut m = Model::new();
        let mut undo = UndoStack::new();
        let idx: Vec<usize> = (0..segments).collect();
        let f = mesh(&mut m, &pts, &[&[&idx]])[0];
        let mut edit = ModelEdit::new();
        let id = m
            .edit_geometry(BodyId::Root, &mut edit, |c, log| {
                let vs: Vec<VertexId> =
                    pts.iter().map(|p| c.find_vertex(*p, 1e-9).unwrap()).collect();
                let mut chain = vs.clone();
                chain.push(vs[0]);
                let cv = curve::fit(c, &chain, start).unwrap();
                c.insert_curve(cv, log)
            })
            .unwrap();
        let old = ctx(&m).curve(id).unwrap().clone();
        let p = path(&[o, o + n * len], false);
        let s = sweep(&mut m, &mut undo, f, &p).unwrap();
        let c = ctx(&m);
        assert_curves_sound(c);
        prop_assert_eq!(c.curve_count(), 2);
        let (nid, cv) = c.curves().find(|(i, _)| *i != id).unwrap();
        prop_assert!(nid > id);
        prop_assert!(cv.center.distance(old.center + n * len) < 1e-7);
        prop_assert!((cv.radius - old.radius).abs() < 1e-7);
        prop_assert!(parallel(cv.normal, old.normal));
        prop_assert_eq!(cv.start, start);
        prop_assert_eq!(cv.vertices.len(), segments + 1);
        let cap: Vec<EdgeId> = c
            .face(s.end.unwrap())
            .unwrap()
            .outer
            .uses
            .iter()
            .map(|u| u.edge)
            .collect();
        prop_assert!(curve::curve_edges(c, nid).iter().all(|e| cap.contains(e)));
    }
}
