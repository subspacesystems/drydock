// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Geometry glued to a body by a shared vertex is pulled loose when moving it would bend
//! the neighbours (found by hand: rotating a rectangle attached to a cube by one corner).

use glam::{DMat4, DQuat};
use modeler_kernel::model::*;
use modeler_kernel::movemerge::transform_sticky;
use modeler_kernel::select::Entity;

mod transform_common;
use transform_common::*;

#[test]
fn rectangle_sharing_a_cube_corner_rotates_free_of_the_cube() {
    let mut model = Model::new();
    let pts = [
        v(0., 0., 0.),
        v(1., 0., 0.),
        v(1., 1., 0.),
        v(0., 1., 0.),
        v(0., 0., 1.),
        v(1., 0., 1.),
        v(1., 1., 1.),
        v(0., 1., 1.),
        v(3., 1., 0.),
        v(3., 3., 0.),
        v(1., 3., 0.),
    ];
    let f = mesh(
        &mut model,
        &pts,
        &[
            &[0, 3, 2, 1],
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[2, 3, 7, 6],
            &[1, 2, 6, 5],
            &[3, 0, 4, 7],
            &[2, 8, 9, 10],
        ],
    );
    let before = positions(&model);
    let mut undo = UndoStack::new();
    let m = DMat4::from_translation(v(2., 2., 0.))
        * DMat4::from_quat(DQuat::from_rotation_z(0.5))
        * DMat4::from_translation(v(-2., -2., 0.));
    transform_sticky(
        &mut model,
        BodyId::Root,
        &[Entity::Face(f[6])],
        m,
        &mut undo,
        "Rotate",
    )
    .unwrap();
    assert!(model.root().geometry().validate().is_empty());
    assert_eq!(model.root().geometry().faces().count(), 7);
    let after = positions(&model);
    // The cube's corner stayed; the rectangle's old corner moved to a new place.
    assert!(
        after
            .iter()
            .filter(|p| p.distance(v(1., 1., 0.)) < 1e-9)
            .count()
            >= 1
    );
    assert!(
        after
            .iter()
            .any(|p| p.distance(m.transform_point3(v(3., 3., 0.))) < 1e-9)
    );
    assert!(undo.undo(&mut model).is_ok());
    assert_eq!(positions(&model).len(), before.len());
}
