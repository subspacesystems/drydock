// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Print mesh on random solids (`docs/specs/m7-print-mesh.md`).

use drydock_kernel::model::*;
use drydock_mesh::print_mesh::*;
use glam::{DMat4, DQuat, DVec3};
use proptest::prelude::*;

mod print_common;
use print_common::*;

fn export(model: &Model, tol_mm: f64) -> Vec<Triangle> {
    let out = print_mesh(
        model,
        PrintScope::Model,
        &PrintOptions {
            chord_tolerance_mm: tol_mm,
        },
    );
    assert!(out.skipped.is_empty(), "{:?}", out.skipped);
    out.objects.into_iter().flat_map(|o| o.triangles).collect()
}

fn transform() -> impl Strategy<Value = DMat4> {
    (
        (-5.0f64..5.0, -5.0f64..5.0, -5.0f64..5.0),
        (-1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0, -3.0f64..3.0),
        (0.2f64..4.0, 0.2f64..4.0, 0.2f64..4.0),
        any::<bool>(),
    )
        .prop_map(|(t, (ax, ay, az, angle), s, mirror)| {
            let axis = DVec3::new(ax, ay, az + 1.5).normalize();
            DMat4::from_translation(DVec3::new(t.0, t.1, t.2))
                * DMat4::from_quat(DQuat::from_axis_angle(axis, angle))
                * DMat4::from_scale(DVec3::new(if mirror { -s.0 } else { s.0 }, s.1, s.2))
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn boxes_under_any_transform_are_closed_and_positive(
        m in transform(),
        size in (0.1f64..3.0, 0.1f64..3.0, 0.1f64..3.0),
    ) {
        let mut model = Model::new();
        let (def, _, _) = box_group(&mut model, "B", DVec3::ZERO, DVec3::new(size.0, size.1, size.2));
        place(&mut model, BodyId::Root, def, m, "B");
        let t = export(&model, 0.01);
        prop_assert_eq!(t.len(), 12);
        assert_closed(&t);
        let expect = size.0 * size.1 * size.2 * m.determinant().abs() * 1e9;
        prop_assert!((volume(&t) - expect).abs() <= expect * 1e-9);
    }

    #[test]
    fn cylinders_are_closed_whatever_the_tolerance(
        n in 3usize..40,
        tol in prop_oneof![0.0005f64..0.5, Just(0.0), Just(1000.0)],
        m in transform(),
    ) {
        let mut model = Model::new();
        let def = cylinder_group(&mut model, "C", 0.01, 0.02, n, true);
        place(&mut model, BodyId::Root, def, m, "C");
        let t = export(&model, tol);
        assert_closed(&t);
        prop_assert!(volume(&t) > 0.0);
        prop_assert!(t.len() >= 4 * n - 4);
    }

    #[test]
    fn a_smaller_tolerance_never_gives_fewer_triangles(
        n in 3usize..30,
        a in 0.001f64..1.0,
        b in 0.001f64..1.0,
    ) {
        let mut model = Model::new();
        let def = cylinder_group(&mut model, "C", 0.01, 0.02, n, true);
        place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "C");
        let (fine, coarse) = (a.min(b), a.max(b));
        prop_assert!(export(&model, fine).len() >= export(&model, coarse).len());
    }

    #[test]
    fn nested_parts_under_random_transforms_are_closed(
        outer in transform(),
        inner in transform(),
    ) {
        let mut model = Model::new();
        let (cube, _, _) = box_group(&mut model, "In", DVec3::ZERO, DVec3::ONE);
        let can = cylinder_group(&mut model, "Can", 0.01, 0.02, 12, true);
        let (group, body) = definition(&mut model, "Out", true);
        place(&mut model, body, cube, inner, "In");
        place(&mut model, body, can, inner, "Can");
        place(&mut model, BodyId::Root, group, outer, "Out");
        let t = export(&model, 0.01);
        assert_closed(&t);
        prop_assert!(volume(&t) > 0.0);
    }
}
