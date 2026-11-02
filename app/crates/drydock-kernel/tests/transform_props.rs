// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Properties of `transform` (`docs/specs/m3-transform.md`).

use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::transform::*;
use glam::{DMat4, DQuat, DVec3};
use proptest::prelude::*;

mod transform_common;
use transform_common as common;

fn matrix() -> impl Strategy<Value = DMat4> {
    (
        (-1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0),
        0.0f64..std::f64::consts::TAU,
        (-5.0f64..5.0, -5.0f64..5.0, -5.0f64..5.0),
        0.2f64..5.0,
        any::<bool>(),
    )
        .prop_map(|((ax, ay, az), angle, (tx, ty, tz), scale, flip)| {
            let axis = DVec3::new(ax, ay, az + 1.5).normalize();
            let sign = if flip { -1.0 } else { 1.0 };
            DMat4::from_translation(DVec3::new(tx, ty, tz))
                * DMat4::from_quat(DQuat::from_axis_angle(axis, angle))
                * DMat4::from_scale(DVec3::new(scale * sign, scale, scale))
        })
}

proptest! {
    #[test]
    fn a_cube_stays_a_valid_outward_cube(m in matrix()) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let faces = common::cube(&mut model);
        let original = model.clone();
        let all: Vec<_> = faces.iter().map(|&f| Entity::Face(f)).collect();
        transform(&mut model, BodyId::Root, &all, m, &mut undo, "T").unwrap();
        prop_assert!(model.validate().is_empty());
        prop_assert_eq!(model.root().geometry().face_count(), 6);
        prop_assert!(common::normals_outward(&model));
        let moved = common::positions(&model);
        for (a, b) in common::positions(&original).iter().zip(moved) {
            prop_assert!(m.transform_point3(*a).distance(b) < 1e-7);
        }
        undo.undo(&mut model).unwrap();
        prop_assert_eq!(model, original);
    }
}
