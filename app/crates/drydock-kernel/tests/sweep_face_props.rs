// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Property tests for `sweep_face` (`docs/specs/m9-sweep-face.md`).

use drydock_kernel::model::*;
use drydock_kernel::pushpull::push_pull;
use drydock_kernel::sweepface::sweep_sections;
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

fn point() -> impl Strategy<Value = DVec3> {
    (-5.0f64..5.0, -5.0f64..5.0, -5.0f64..5.0).prop_map(|(x, y, z)| v(x, y, z))
}

fn rel_close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

fn clean_volume(m: &Model) -> f64 {
    let r = solid(ctx(m));
    assert!(r.is_clean(), "{r:?}");
    r.volume.unwrap()
}

/// A regular polygon fronting `n` (with a concentric hole when `hole` is given, as a
/// fraction of the radius) at `c`.
fn polygon(m: &mut Model, c: DVec3, n: DVec3, r: f64, sides: usize, hole: Option<f64>) {
    let mut pts = ngon(c, n, r, sides, 0.3);
    let outer: Vec<usize> = (0..sides).collect();
    let mut loops: Vec<Vec<usize>> = vec![outer];
    if let Some(k) = hole {
        pts.extend(ngon(c, n, r * k, sides, 0.3));
        loops.push((sides..2 * sides).rev().collect());
    }
    let refs: Vec<&[usize]> = loops.iter().map(|l| l.as_slice()).collect();
    mesh(m, &pts, &[&refs]);
}

proptest! {
    /// Along a straight path square to the profile, a sweep is a push/pull.
    #[test]
    fn a_straight_sweep_is_a_push_pull(
        sides in 3usize..9,
        r in 0.1f64..2.0,
        hole in proptest::option::of(0.2f64..0.7),
        q in rotation(),
        c in point(),
        d in prop_oneof![0.05f64..5.0, -5.0f64..-0.05],
    ) {
        let n = q * DVec3::Z;
        let mut swept = Model::new();
        let mut undo = UndoStack::new();
        polygon(&mut swept, c, n, r, sides, hole);
        let f = ctx(&swept).faces().next().unwrap().0;
        let s = sweep(&mut swept, &mut undo, f, &path(&[c, c + n * d], false)).unwrap();
        prop_assert_eq!(s.start, Some(f));
        prop_assert_eq!(s.sides.len(), sides * if hole.is_some() { 2 } else { 1 });

        let mut pulled = Model::new();
        polygon(&mut pulled, c, n, r, sides, hole);
        let g = ctx(&pulled).faces().next().unwrap().0;
        push_pull(&mut pulled, BodyId::Root, g, d, &mut undo, "Push/Pull").unwrap();
        prop_assert!(same_faces(ctx(&swept), ctx(&pulled), 1e-9));
        prop_assert!(ctx(&swept).validate().is_empty());
    }

    /// A rectangle round an L: its area times the path length, less what the mitre takes
    /// off the inside of the turn (`e` is how far the profile's centre sits toward it).
    #[test]
    fn an_l_sweep_has_the_mitred_volume(
        a in 1.0f64..3.0,
        b in 1.0f64..3.0,
        turn in 10.0f64..150.0,
        w in 0.05f64..0.3,
        h in 0.05f64..0.3,
        e in -0.3f64..0.3,
        lift in -0.2f64..0.2,
        front_along in any::<bool>(),
        q in rotation(),
        t in point(),
    ) {
        let half = (turn.to_radians() / 2.0).tan();
        prop_assume!((e.abs() + w / 2.0) * half < 0.4 * a.min(b));
        let place = |p: DVec3| q * p + t;
        let (u, wv) = if front_along {
            (q * DVec3::Y, q * DVec3::Z)
        } else {
            (q * DVec3::Z, q * DVec3::Y)
        };
        let pts = if front_along {
            rect_points(t, u, wv, e - w / 2.0, e + w / 2.0, lift - h / 2.0, lift + h / 2.0)
        } else {
            rect_points(t, u, wv, lift - h / 2.0, lift + h / 2.0, e - w / 2.0, e + w / 2.0)
        };
        let mut m = Model::new();
        let mut undo = UndoStack::new();
        let f = mesh(&mut m, &pts, &[&[&[0, 1, 2, 3]]])[0];
        let th = turn.to_radians();
        let p = path(
            &[
                place(v(0.0, 0.0, 0.0)),
                place(v(a, 0.0, 0.0)),
                place(v(a + b * th.cos(), b * th.sin(), 0.0)),
            ],
            false,
        );
        let sections = sweep_sections(ctx(&m), f, &p).unwrap();
        prop_assert_eq!(sections.len(), 3);
        let s = sweep(&mut m, &mut undo, f, &p).unwrap();
        // Walls stay apart; the top and bottom of the two legs share planes and merge.
        prop_assert_eq!(s.sides.len(), 6);
        let expected = w * h * (a + b - 2.0 * e * half);
        prop_assert!(rel_close(clean_volume(&m), expected), "{} vs {}", clean_volume(&m), expected);

        let mut back = Model::new();
        let g = mesh(&mut back, &pts, &[&[&[0, 1, 2, 3]]])[0];
        sweep(&mut back, &mut undo, g, &reversed(&p)).unwrap();
        prop_assert!(same_faces(ctx(&m), ctx(&back), 1e-9));
    }

    /// A rectangle swept round a flat convex ring from part way along a side: its area times
    /// the length its centre travels, which grows by 2 e tan(turn / 2) at every corner.
    #[test]
    fn a_ring_has_the_mitred_volume(
        gaps in proptest::collection::vec(0.7f64..1.3, 3..8),
        radius in 2.0f64..4.0,
        side in any::<proptest::sample::Index>(),
        along in 0.3f64..0.7,
        out in 0.05f64..0.3,
        w in 0.05f64..0.3,
        h in 0.05f64..0.3,
        q in rotation(),
        t in point(),
    ) {
        let n = gaps.len();
        let total: f64 = gaps.iter().sum();
        let mut angle = 0.0f64;
        let mut local = Vec::new();
        for g in &gaps {
            local.push(v(radius * angle.cos(), radius * angle.sin(), 0.0));
            angle += g / total * std::f64::consts::TAU;
        }
        let place = |p: DVec3| q * p + t;
        let pts: Vec<DVec3> = local.iter().map(|p| place(*p)).collect();
        let p = path(&pts, true);

        let s0 = side.index(n);
        let (a, b) = (local[s0], local[(s0 + 1) % n]);
        let d = (b - a).normalize();
        let outward = d.cross(DVec3::Z);
        let at = a + (b - a) * along;
        let profile = rect_points(place(at), q * outward, q * DVec3::Z, out, out + w, 0.0, h);
        let mut m = Model::new();
        let mut undo = UndoStack::new();
        let f = mesh(&mut m, &profile, &[&[&[0, 1, 2, 3]]])[0];
        let s = sweep(&mut m, &mut undo, f, &p).unwrap();
        prop_assert_eq!((s.start, s.end), (None, None));
        prop_assert!(ctx(&m).face(f).is_none());
        prop_assert_eq!(s.sides.len(), 2 * n + 2);

        let e = out + w / 2.0;
        let mut length = 0.0;
        for i in 0..n {
            let prev = (local[i] - local[(i + n - 1) % n]).normalize();
            let next = (local[(i + 1) % n] - local[i]).normalize();
            let turn = prev.dot(next).clamp(-1.0, 1.0).acos();
            length += (local[(i + 1) % n] - local[i]).length() + 2.0 * e * (turn / 2.0).tan();
        }
        let expected = w * h * length;
        prop_assert!(rel_close(clean_volume(&m), expected), "{} vs {}", clean_volume(&m), expected);

        let mut back = Model::new();
        let g = mesh(&mut back, &profile, &[&[&[0, 1, 2, 3]]])[0];
        sweep(&mut back, &mut undo, g, &reversed(&p)).unwrap();
        prop_assert!(same_faces(ctx(&m), ctx(&back), 1e-9));
    }

    /// A small square centred on the start of any open path, square to its first segment,
    /// sweeps to its area times the path's length, the path need not be flat.
    #[test]
    fn a_centred_profile_sweeps_area_times_length(
        steps in proptest::collection::vec(
            (1.0f64..3.0, -1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0),
            1..5,
        ),
        spin in 0.0f64..std::f64::consts::TAU,
        t in point(),
    ) {
        let mut pts = vec![t];
        let mut prev: Option<DVec3> = None;
        let mut length = 0.0;
        for (len, x, y, z) in &steps {
            let dir = v(*x, *y, *z);
            prop_assume!(dir.length() > 0.2);
            let dir = dir.normalize();
            if let Some(p) = prev {
                prop_assume!(p.dot(dir) > (120.0f64).to_radians().cos());
            }
            prev = Some(dir);
            pts.push(*pts.last().unwrap() + dir * *len);
            length += len;
        }
        prop_assume!(pts.last().unwrap().distance(t) > 0.5);
        let d0 = (pts[1] - pts[0]).normalize();
        let (u0, w0) = basis(d0);
        let (u, w) = (
            u0 * spin.cos() + w0 * spin.sin(),
            -u0 * spin.sin() + w0 * spin.cos(),
        );
        let profile = rect_points(t, u, w, -0.05, 0.05, -0.05, 0.05);
        let p = path(&pts, false);
        let mut m = Model::new();
        let mut undo = UndoStack::new();
        let f = mesh(&mut m, &profile, &[&[&[0, 1, 2, 3]]])[0];
        let s = sweep(&mut m, &mut undo, f, &p).unwrap();
        prop_assert!(s.sides.len() <= 4 * steps.len());
        prop_assert!(rel_close(clean_volume(&m), 0.01 * length));
        prop_assert_eq!(undo.undo_count(), 1);

        let mut back = Model::new();
        let g = mesh(&mut back, &profile, &[&[&[0, 1, 2, 3]]])[0];
        sweep(&mut back, &mut undo, g, &reversed(&p)).unwrap();
        prop_assert!(same_faces(ctx(&m), ctx(&back), 1e-9));
    }
}
