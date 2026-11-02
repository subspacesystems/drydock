// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Property tests for `push_pull` (`docs/specs/m3-pushpull.md`).

use drydock_kernel::model::*;
use drydock_kernel::pushpull::*;
use glam::{DQuat, DVec3};
use proptest::prelude::*;

mod pushpull_common;
use pushpull_common::*;

/// A regular `n`-gon of radius `r`, rotated by `q` and moved to `c`, fronting `q * +z`.
fn polygon(model: &mut Model, n: usize, r: f64, q: DQuat, c: DVec3) {
    let pts: Vec<DVec3> = (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            c + q * v(r * a.cos(), r * a.sin(), 0.0)
        })
        .collect();
    let order: Vec<usize> = (0..n).collect();
    mesh(model, &pts, &[&[&order]]);
}

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

fn distance() -> impl Strategy<Value = f64> {
    prop_oneof![0.01f64..10.0, -10.0f64..-0.01]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn a_lone_polygon_extrudes_to_a_closed_prism(
        n in 3usize..12,
        r in 0.1f64..5.0,
        q in rotation(),
        cx in -10.0f64..10.0,
        cy in -10.0f64..10.0,
        cz in -10.0f64..10.0,
        d in distance(),
    ) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        polygon(&mut model, n, r, q, v(cx, cy, cz));
        let f = ctx(&model).faces().next().unwrap().0;
        let area = 0.5 * n as f64 * r * r * (std::f64::consts::TAU / n as f64).sin();
        let before = model.clone();

        let res = run_ok(&mut model, &mut undo, f, d);
        let c = ctx(&model);
        prop_assert!(c.validate().is_empty(), "{:?}", c.validate());
        prop_assert_eq!(c.vertex_count(), 2 * n);
        prop_assert_eq!(c.edge_count(), 3 * n);
        prop_assert_eq!(c.face_count(), n + 2);
        prop_assert_eq!(res.sides.len(), n);
        prop_assert!(res.base.is_some());
        prop_assert!(closed(c));
        prop_assert!(normals_outward(c));
        let vol = volume(c);
        prop_assert!((vol - area * d.abs()).abs() < 1e-6 * (1.0 + area * d.abs()), "{} vs {}", vol, area * d.abs());
        let shift = centroid(&face_points(c, res.cap)) - (v(cx, cy, cz));
        prop_assert!((shift - q * DVec3::Z * d).length() < 1e-6);

        undo.undo(&mut model).unwrap();
        prop_assert_eq!(&model, &before);
    }

    #[test]
    fn a_box_top_slides_to_any_height_above_the_bottom(d in prop_oneof![0.01f64..10.0, -0.99f64..-0.01]) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let faces = cube(&mut model);
        let before = model.clone();
        let res = run_ok(&mut model, &mut undo, faces[1], d);
        prop_assert_eq!(res.cap, faces[1]);
        let c = ctx(&model);
        prop_assert!(c.validate().is_empty());
        prop_assert!((volume(c) - (1.0 + d)).abs() < 1e-9);
        undo.undo(&mut model).unwrap();
        prop_assert_eq!(&model, &before);
    }
}

fn run_ok(
    model: &mut Model,
    undo: &mut UndoStack,
    f: drydock_kernel::topology::FaceId,
    d: f64,
) -> PushPulled {
    push_pull(model, BodyId::Root, f, d, undo, "Push/Pull").unwrap()
}
