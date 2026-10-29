// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Solid check on random boxes (`docs/specs/m7-solid-check.md`).

use glam::{DMat4, DVec3};
use modeler_kernel::model::*;
use modeler_kernel::solid::{check_geometry, winding_number};
use modeler_kernel::topology::FaceId;
use proptest::prelude::*;

mod transform_common;
use transform_common::*;

const FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [2, 3, 7, 6],
    [1, 2, 6, 5],
    [3, 0, 4, 7],
];

/// A box of the given size, turned and moved by `m`, with the faces in `flip` (bit i = face
/// i) stored back to front and the faces in `drop` left out.
fn placed_box(model: &mut Model, size: DVec3, m: DMat4, flip: u32, drop: u32) -> Vec<FaceId> {
    let pts: Vec<DVec3> = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
        v(0.0, 0.0, 1.0),
        v(1.0, 0.0, 1.0),
        v(1.0, 1.0, 1.0),
        v(0.0, 1.0, 1.0),
    ]
    .iter()
    .map(|p| m.transform_point3(*p * size))
    .collect();
    let faces: Vec<Vec<usize>> = FACES
        .iter()
        .enumerate()
        .filter(|(i, _)| drop & (1 << i) == 0)
        .map(|(i, f)| {
            let mut f = f.to_vec();
            if flip & (1 << i) != 0 {
                f.reverse();
            }
            f
        })
        .collect();
    let refs: Vec<&[usize]> = faces.iter().map(|f| f.as_slice()).collect();
    mesh(model, &pts, &refs)
}

fn placement() -> impl Strategy<Value = (DVec3, DMat4)> {
    (
        (0.001f64..5.0, 0.001f64..5.0, 0.001f64..5.0),
        (-3.2f64..3.2, -3.2f64..3.2, -3.2f64..3.2),
        (-100.0f64..100.0, -100.0f64..100.0, -100.0f64..100.0),
    )
        .prop_map(|((sx, sy, sz), (a, b, c), (tx, ty, tz))| {
            let m = DMat4::from_translation(v(tx, ty, tz))
                * DMat4::from_rotation_z(a)
                * DMat4::from_rotation_y(b)
                * DMat4::from_rotation_x(c);
            (v(sx, sy, sz), m)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn flipped_faces_are_exactly_the_reversed_ones(
        (size, m) in placement(),
        flip in 0u32..64,
    ) {
        let mut model = Model::new();
        let ids = placed_box(&mut model, size, m, flip, 0);
        let r = check_geometry(model.root().geometry());
        let mut want: Vec<FaceId> =
            (0..6).filter(|i| flip & (1 << i) != 0).map(|i| ids[i]).collect();
        want.sort();
        prop_assert_eq!(&r.reversed_faces, &want);
        prop_assert_eq!(r.shells, 1);
        prop_assert!(r.is_solid());
        prop_assert_eq!(r.is_clean(), flip == 0);
        let vol = size.x * size.y * size.z;
        let got = r.volume.unwrap();
        prop_assert!((got - vol).abs() <= 1e-9 * (1.0 + vol) + 1e-6, "{} vs {}", got, vol);
    }

    #[test]
    fn dropped_faces_open_their_edges(
        (size, m) in placement(),
        drop in 1u32..63,
    ) {
        // Every corner must keep a face, or it would be a stray vertex.
        let corner_kept =
            |c: usize| (0..6).any(|i| drop & (1 << i) == 0 && FACES[i].contains(&c));
        prop_assume!((0..8).all(corner_kept));
        let mut model = Model::new();
        placed_box(&mut model, size, m, 0, drop);
        let ctx = model.root().geometry();
        let r = check_geometry(ctx);
        // An edge is open when exactly one of its two box faces is left.
        let mut want: Vec<_> = ctx
            .edges()
            .filter(|(e, _)| ctx.faces_of_edge(*e).len() == 1)
            .map(|(e, _)| e)
            .collect();
        want.sort();
        prop_assert!(!want.is_empty());
        prop_assert_eq!(&r.open_edges, &want);
        prop_assert!(r.reversed_faces.is_empty(), "{:?}", r.reversed_faces);
        // Edges whose two faces are both gone were never built.
        prop_assert!(r.loose_edges.is_empty());
        prop_assert!(!r.is_solid());
        prop_assert_eq!(r.volume, None);
    }

    #[test]
    fn winding_number_is_one_inside_and_zero_outside(
        (size, m) in placement(),
        (u, w, t) in (0.05f64..0.95, 0.05f64..0.95, 0.05f64..0.95),
        out in (1.2f64..4.0, -3.0f64..3.0, -3.0f64..3.0),
    ) {
        let mut model = Model::new();
        let ids = placed_box(&mut model, size, m, 0, 0);
        let ctx = model.root().geometry();
        let inside = m.transform_point3(v(u, w, t) * size);
        let outside = m.transform_point3(v(out.0, out.1, out.2) * size);
        let wi = winding_number(ctx, &ids, inside);
        let wo = winding_number(ctx, &ids, outside);
        prop_assert!((wi - 1.0).abs() < 1e-6, "{}", wi);
        prop_assert!(wo.abs() < 1e-6, "{}", wo);
    }
}
