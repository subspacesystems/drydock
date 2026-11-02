// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Geometry glued to a body by a shared vertex is pulled loose when moving it would bend
//! the neighbours (found by hand: rotating a rectangle attached to a cube by one corner).

use drydock_kernel::model::*;
use drydock_kernel::movemerge::transform_sticky;
use drydock_kernel::select::Entity;
use glam::{DMat4, DQuat};

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

#[test]
fn rotating_a_cube_edge_about_its_own_axis_is_a_no_op() {
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
    ];
    mesh(
        &mut model,
        &pts,
        &[
            &[0, 3, 2, 1],
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[2, 3, 7, 6],
            &[1, 2, 6, 5],
            &[3, 0, 4, 7],
        ],
    );
    let edge = model
        .root()
        .geometry()
        .edges()
        .map(|(id, _)| id)
        .find(|&id| {
            let ctx = model.root().geometry();
            let ends = ctx.edge(id).unwrap().ends;
            let a = ctx.vertex(ends[0]).unwrap().position;
            let b = ctx.vertex(ends[1]).unwrap().position;
            a.distance(v(1., 1., 0.)) < 1e-9 && b.distance(v(1., 1., 1.)) < 1e-9
                || b.distance(v(1., 1., 0.)) < 1e-9 && a.distance(v(1., 1., 1.)) < 1e-9
        })
        .unwrap();
    let before = positions(&model);
    let mut undo = UndoStack::new();
    let m = DMat4::from_translation(v(1., 1., 0.))
        * DMat4::from_quat(DQuat::from_rotation_z(0.7))
        * DMat4::from_translation(v(-1., -1., 0.));
    transform_sticky(
        &mut model,
        BodyId::Root,
        &[Entity::Edge(edge)],
        m,
        &mut undo,
        "Rotate",
    )
    .unwrap();
    assert_eq!(positions(&model), before);
}

#[test]
fn rectangle_selected_by_its_edges_only_still_rotates_onto_the_cube() {
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
        v(1.5, 1., 1.),
        v(1.5, 1.8, 1.),
        v(1., 1.8, 1.),
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
            &[6, 8, 9, 10],
        ],
    );
    let ctx = model.root().geometry();
    let face = ctx.face(f[6]).unwrap();
    let edges: Vec<Entity> = face
        .outer
        .uses
        .iter()
        .map(|u| Entity::Edge(u.edge))
        .collect();
    // Swing the rectangle, flush with the cube's top, across it, about the shared corner.
    for deg in [
        30.0_f64, 60., 90., 120., 150., 180., 210., 240., 270., 300., 330.,
    ] {
        let mut model = model.clone();
        let m = DMat4::from_translation(v(1., 1., 1.))
            * DMat4::from_quat(DQuat::from_rotation_z(deg.to_radians()))
            * DMat4::from_translation(v(-1., -1., -1.));
        let mut undo = UndoStack::new();
        transform_sticky(&mut model, BodyId::Root, &edges, m, &mut undo, "Rotate")
            .unwrap_or_else(|e| panic!("{deg} degrees: {e:?}"));
        assert!(model.root().geometry().validate().is_empty());
    }
}

#[test]
fn rectangle_selected_with_a_cube_edge_on_the_rotation_axis_still_rotates() {
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
        v(1.5, 1., 1.),
        v(1.5, 1.8, 1.),
        v(1., 1.8, 1.),
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
            &[6, 8, 9, 10],
        ],
    );
    let ctx = model.root().geometry();
    let vert = ctx
        .edges()
        .map(|(id, _)| id)
        .find(|&id| {
            let e = ctx.edge(id).unwrap();
            let a = ctx.vertex(e.ends[0]).unwrap().position;
            let b = ctx.vertex(e.ends[1]).unwrap().position;
            (a.distance(v(1., 1., 0.)) < 1e-9 && b.distance(v(1., 1., 1.)) < 1e-9)
                || (b.distance(v(1., 1., 0.)) < 1e-9 && a.distance(v(1., 1., 1.)) < 1e-9)
        })
        .unwrap();
    let mut sel: Vec<Entity> = vec![Entity::Face(f[6]), Entity::Edge(vert)];
    for u in &ctx.face(f[6]).unwrap().outer.uses {
        sel.push(Entity::Edge(u.edge));
    }
    for deg in [30.0_f64, 60., 89., 90., 91., 120., 180.] {
        let mut m2 = model.clone();
        let m = DMat4::from_translation(v(1., 1., 1.))
            * DMat4::from_quat(DQuat::from_rotation_z(deg.to_radians()))
            * DMat4::from_translation(v(-1., -1., -1.));
        let mut undo = UndoStack::new();
        let r = transform_sticky(&mut m2, BodyId::Root, &sel, m, &mut undo, "Rotate");
        r.unwrap_or_else(|e| panic!("{deg} degrees: {e:?}"));
        assert!(m2.root().geometry().validate().is_empty());
    }
}
