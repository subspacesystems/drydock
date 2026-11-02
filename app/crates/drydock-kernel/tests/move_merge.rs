// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `transform_sticky` (`docs/specs/m3-move-merge.md`).

use drydock_kernel::model::*;
use drydock_kernel::movemerge::*;
use drydock_kernel::select::Entity;
use drydock_kernel::topology::*;
use drydock_kernel::transform::{TransformError, transform};
use glam::{DMat4, DVec3};

mod transform_common;
use transform_common::*;

fn ctx(model: &Model) -> &Context {
    model.root().geometry()
}

/// Draw a sticky segment in the root body.
fn draw(model: &mut Model, a: DVec3, b: DVec3) {
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(BodyId::Root, &mut edit, |ctx, log| {
            ctx.add_edge(a, b, log).unwrap();
        })
        .unwrap();
}

/// The edge with ends at `a` and `b` (either order).
fn edge_at(model: &Model, a: DVec3, b: DVec3) -> EdgeId {
    let c = ctx(model);
    c.edges()
        .find(|(_, e)| {
            let p = c.vertex(e.ends[0]).unwrap().position;
            let q = c.vertex(e.ends[1]).unwrap().position;
            (p.distance(a) < 1e-9 && q.distance(b) < 1e-9)
                || (p.distance(b) < 1e-9 && q.distance(a) < 1e-9)
        })
        .map(|(id, _)| id)
        .expect("edge not found")
}

fn square(model: &mut Model, x: f64, y: f64, z: f64, size: f64) -> FaceId {
    let pts = [
        v(x, y, z),
        v(x + size, y, z),
        v(x + size, y + size, z),
        v(x, y + size, z),
    ];
    mesh(model, &pts, &[&[0, 1, 2, 3]])[0]
}

fn counts(model: &Model) -> (usize, usize, usize) {
    let c = ctx(model);
    (c.vertex_count(), c.edge_count(), c.face_count())
}

fn has_vertex(model: &Model, p: DVec3) -> bool {
    positions(model).iter().any(|q| q.distance(p) < 1e-9)
}

fn run(
    model: &mut Model,
    undo: &mut UndoStack,
    selection: &[Entity],
    m: DMat4,
) -> Result<(), TransformError> {
    transform_sticky(model, BodyId::Root, selection, m, undo, "Move")
}

fn shift(x: f64, y: f64, z: f64) -> DMat4 {
    DMat4::from_translation(v(x, y, z))
}

#[test]
fn moved_end_welds_to_an_unmoved_vertex() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    draw(&mut model, v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
    draw(&mut model, v(5.0, 0.0, 0.0), v(5.0, 2.0, 0.0));
    let moving = edge_at(&model, v(5.0, 0.0, 0.0), v(5.0, 2.0, 0.0));
    run(
        &mut model,
        &mut undo,
        &[Entity::Edge(moving)],
        shift(-3.0, 0.0, 0.0),
    )
    .unwrap();
    assert_eq!(counts(&model), (3, 2, 0));
    let joint = ctx(&model)
        .vertices()
        .find(|(_, p)| p.position.distance(v(2.0, 0.0, 0.0)) < 1e-9)
        .unwrap()
        .0;
    assert_eq!(ctx(&model).edges_of_vertex(joint).len(), 2);
    assert!(ctx(&model).validate().is_empty());
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Move"));
}

#[test]
fn moved_end_on_an_edge_splits_it() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    draw(&mut model, v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0));
    draw(&mut model, v(2.0, 3.0, 0.0), v(2.0, 5.0, 0.0));
    let moving = edge_at(&model, v(2.0, 3.0, 0.0), v(2.0, 5.0, 0.0));
    run(
        &mut model,
        &mut undo,
        &[Entity::Edge(moving)],
        shift(0.0, -3.0, 0.0),
    )
    .unwrap();
    assert_eq!(counts(&model), (4, 3, 0));
    edge_at(&model, v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
    edge_at(&model, v(2.0, 0.0, 0.0), v(4.0, 0.0, 0.0));
    edge_at(&model, v(2.0, 0.0, 0.0), v(2.0, 2.0, 0.0));
    assert!(ctx(&model).validate().is_empty());
}

#[test]
fn moved_edge_crossing_an_edge_splits_both() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    draw(&mut model, v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0));
    draw(&mut model, v(2.0, 1.0, 0.0), v(2.0, 3.0, 0.0));
    let moving = edge_at(&model, v(2.0, 1.0, 0.0), v(2.0, 3.0, 0.0));
    run(
        &mut model,
        &mut undo,
        &[Entity::Edge(moving)],
        shift(0.0, -2.0, 0.0),
    )
    .unwrap();
    assert_eq!(counts(&model), (5, 4, 0));
    edge_at(&model, v(2.0, -1.0, 0.0), v(2.0, 0.0, 0.0));
    edge_at(&model, v(2.0, 0.0, 0.0), v(2.0, 1.0, 0.0));
    assert!(ctx(&model).validate().is_empty());
}

#[test]
fn moved_edge_laid_over_edges_reuses_them() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    draw(&mut model, v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0));
    draw(&mut model, v(2.0, 0.0, 0.0), v(2.0, -3.0, 0.0));
    draw(&mut model, v(0.0, 5.0, 0.0), v(4.0, 5.0, 0.0));
    assert_eq!(counts(&model), (6, 4, 0));
    let moving = edge_at(&model, v(0.0, 5.0, 0.0), v(4.0, 5.0, 0.0));
    run(
        &mut model,
        &mut undo,
        &[Entity::Edge(moving)],
        shift(0.0, -5.0, 0.0),
    )
    .unwrap();
    assert_eq!(counts(&model), (4, 3, 0));
    assert!(ctx(&model).validate().is_empty());
}

#[test]
fn moved_edge_overlapping_partly_splits_at_the_unmoved_vertices() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    draw(&mut model, v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
    draw(&mut model, v(2.0, 0.0, 0.0), v(2.0, -3.0, 0.0));
    draw(&mut model, v(-1.0, 5.0, 0.0), v(4.0, 5.0, 0.0));
    let moving = edge_at(&model, v(-1.0, 5.0, 0.0), v(4.0, 5.0, 0.0));
    run(
        &mut model,
        &mut undo,
        &[Entity::Edge(moving)],
        shift(0.0, -5.0, 0.0),
    )
    .unwrap();
    assert_eq!(counts(&model), (5, 4, 0));
    edge_at(&model, v(-1.0, 0.0, 0.0), v(0.0, 0.0, 0.0));
    edge_at(&model, v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
    edge_at(&model, v(2.0, 0.0, 0.0), v(4.0, 0.0, 0.0));
    assert!(ctx(&model).validate().is_empty());
}

#[test]
fn face_dropped_inside_a_face_makes_a_hole_and_a_new_face() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    square(&mut model, 0.0, 0.0, 0.0, 10.0);
    let small = square(&mut model, 3.0, 3.0, 3.0, 2.0);
    run(
        &mut model,
        &mut undo,
        &[Entity::Face(small)],
        shift(0.0, 0.0, -3.0),
    )
    .unwrap();
    assert_eq!(counts(&model), (8, 8, 2));
    let c = ctx(&model);
    assert!(c.validate().is_empty());
    let holes: Vec<usize> = c.faces().map(|(_, f)| f.inner.len()).collect();
    assert_eq!(holes.iter().filter(|&&n| n == 1).count(), 1);
    assert_eq!(holes.iter().filter(|&&n| n == 0).count(), 1);
}

#[test]
fn face_moved_beside_a_face_shares_the_edge() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let _a = square(&mut model, 0.0, 0.0, 0.0, 1.0);
    let b = square(&mut model, 3.0, 0.0, 0.0, 1.0);
    run(
        &mut model,
        &mut undo,
        &[Entity::Face(b)],
        shift(-2.0, 0.0, 0.0),
    )
    .unwrap();
    assert_eq!(counts(&model), (6, 7, 2));
    let shared = edge_at(&model, v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0));
    assert_eq!(ctx(&model).faces_of_edge(shared).len(), 2);
    assert!(ctx(&model).validate().is_empty());
}

#[test]
fn face_moved_across_a_face_splits_the_faces_at_the_overlap() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let _a = square(&mut model, 0.0, 0.0, 0.0, 2.0);
    let b = square(&mut model, 5.0, 0.0, 0.0, 2.0);
    run(
        &mut model,
        &mut undo,
        &[Entity::Face(b)],
        shift(-4.0, 0.0, 0.0),
    )
    .unwrap();
    // x from 0 to 3 in three strips: 0-1, 1-2, 2-3.
    assert_eq!(counts(&model), (8, 10, 3));
    assert!(ctx(&model).validate().is_empty());
    for x in [0.0, 1.0, 2.0, 3.0] {
        assert!(has_vertex(&model, v(x, 0.0, 0.0)));
        assert!(has_vertex(&model, v(x, 2.0, 0.0)));
    }
}

#[test]
fn without_contact_it_is_the_plain_transform() {
    let mut a = Model::new();
    let mut b = Model::new();
    let fa = square(&mut a, 0.0, 0.0, 0.0, 1.0);
    let fb = square(&mut b, 0.0, 0.0, 0.0, 1.0);
    square(&mut a, 5.0, 5.0, 0.0, 1.0);
    square(&mut b, 5.0, 5.0, 0.0, 1.0);
    let (mut ua, mut ub) = (UndoStack::new(), UndoStack::new());
    let m = shift(0.0, 0.0, 2.0);
    run(&mut a, &mut ua, &[Entity::Face(fa)], m).unwrap();
    transform(
        &mut b,
        BodyId::Root,
        &[Entity::Face(fb)],
        m,
        &mut ub,
        "Move",
    )
    .unwrap();
    assert_eq!(a, b);
}

#[test]
fn a_cube_top_still_stretches_like_the_plain_transform() {
    let mut a = Model::new();
    let mut b = Model::new();
    let fa = cube(&mut a);
    let fb = cube(&mut b);
    let (mut ua, mut ub) = (UndoStack::new(), UndoStack::new());
    let m = shift(0.0, 0.0, 2.0);
    run(&mut a, &mut ua, &[Entity::Face(fa[1])], m).unwrap();
    transform(
        &mut b,
        BodyId::Root,
        &[Entity::Face(fb[1])],
        m,
        &mut ub,
        "Move",
    )
    .unwrap();
    assert_eq!(a, b);
    assert!(normals_outward(&a));
}

#[test]
fn undo_restores_and_redo_reapplies() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    square(&mut model, 0.0, 0.0, 0.0, 10.0);
    let small = square(&mut model, 3.0, 3.0, 3.0, 2.0);
    let original = model.clone();
    run(
        &mut model,
        &mut undo,
        &[Entity::Face(small)],
        shift(0.0, 0.0, -3.0),
    )
    .unwrap();
    let moved = model.clone();
    assert_ne!(moved, original);
    undo.undo(&mut model).unwrap();
    assert_eq!(model, original);
    undo.redo(&mut model).unwrap();
    assert_eq!(model, moved);
}

#[test]
fn refusals_match_transform_and_change_nothing() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 0.0, 1.0);
    let original = model.clone();
    let sel = [Entity::Face(f)];
    assert_eq!(
        transform_sticky(
            &mut model,
            BodyId::Root,
            &[],
            shift(1.0, 0.0, 0.0),
            &mut undo,
            "Move"
        ),
        Err(TransformError::Empty)
    );
    assert_eq!(
        run(&mut model, &mut undo, &sel, DMat4::ZERO),
        Err(TransformError::BadMatrix)
    );
    assert_eq!(
        run(
            &mut model,
            &mut undo,
            &[Entity::Face(FaceId(999))],
            shift(1.0, 0.0, 0.0)
        ),
        Err(TransformError::MissingEntity(Entity::Face(FaceId(999))))
    );
    run(&mut model, &mut undo, &sel, DMat4::IDENTITY).unwrap();
    assert_eq!(model, original);
    assert_eq!(undo.undo_count(), 0);
}
