// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `transform` and `moved_vertices` (`docs/specs/m3-transform.md`).

use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::topology::*;
use drydock_kernel::transform::*;
use glam::{DMat4, DQuat, DVec3};

mod transform_common;
use transform_common::*;

fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

/// Add a closed mesh: `faces` lists vertex indexes counter-clockwise seen from the front.
/// Edges are shared between faces. Returns the face ids in order.
fn square(model: &mut Model, x: f64, y: f64, size: f64) -> FaceId {
    let pts = [
        v(x, y, 0.0),
        v(x + size, y, 0.0),
        v(x + size, y + size, 0.0),
        v(x, y + size, 0.0),
    ];
    mesh(model, &pts, &[&[0, 1, 2, 3]])[0]
}

fn ctx(model: &Model) -> &Context {
    model.root().geometry()
}

fn run(
    model: &mut Model,
    undo: &mut UndoStack,
    selection: &[Entity],
    m: DMat4,
) -> Result<(), TransformError> {
    transform(model, BodyId::Root, selection, m, undo, "Move")
}

// -- moved_vertices ---------------------------------------------------------------------

#[test]
fn moved_vertices_of_a_face_is_all_its_loop_vertices() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let set = moved_vertices(ctx(&model), &[Entity::Face(faces[1])]);
    assert_eq!(set.len(), 4);
    for id in &set {
        assert!((ctx(&model).vertex(*id).unwrap().position.z - 1.0).abs() < 1e-12);
    }
}

#[test]
fn moved_vertices_of_an_edge_is_its_two_ends() {
    let mut model = Model::new();
    cube(&mut model);
    let (e, edge) = ctx(&model).edges().next().unwrap();
    let set = moved_vertices(ctx(&model), &[Entity::Edge(e)]);
    assert_eq!(set, edge.ends.into_iter().collect());
}

#[test]
fn moved_vertices_joins_the_selection_and_skips_unknowns() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let all: Vec<_> = faces.iter().map(|&f| Entity::Face(f)).collect();
    assert_eq!(moved_vertices(ctx(&model), &all).len(), 8);
    let unknown = [Entity::Face(FaceId(999)), Entity::Edge(EdgeId(999))];
    assert!(moved_vertices(ctx(&model), &unknown).is_empty());
}

// -- moving ----------------------------------------------------------------------------

#[test]
fn move_a_face_shifts_its_vertices_only() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let a = square(&mut model, 0.0, 0.0, 1.0);
    let _b = square(&mut model, 5.0, 0.0, 1.0);
    let before = positions(&model);
    run(
        &mut model,
        &mut undo,
        &[Entity::Face(a)],
        DMat4::from_translation(v(0.0, 0.0, 2.0)),
    )
    .unwrap();
    let after = positions(&model);
    assert_eq!(after.len(), before.len());
    let lifted = after.iter().filter(|p| (p.z - 2.0).abs() < 1e-12).count();
    assert_eq!(lifted, 4);
    assert_eq!(after.iter().filter(|p| p.z.abs() < 1e-12).count(), 4);
    assert!(ctx(&model).validate().is_empty());
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Move"));
}

#[test]
fn undo_restores_and_redo_reapplies() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube(&mut model);
    let original = model.clone();
    let all: Vec<_> = faces.iter().map(|&f| Entity::Face(f)).collect();
    run(
        &mut model,
        &mut undo,
        &all,
        DMat4::from_translation(v(1.0, 2.0, 3.0)),
    )
    .unwrap();
    let moved = model.clone();
    assert_ne!(model, original);
    undo.undo(&mut model).unwrap();
    assert_eq!(model, original);
    undo.redo(&mut model).unwrap();
    assert_eq!(model, moved);
}

#[test]
fn moving_a_cube_top_face_stretches_the_sides() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube(&mut model);
    run(
        &mut model,
        &mut undo,
        &[Entity::Face(faces[1])],
        DMat4::from_translation(v(0.0, 0.0, 2.0)),
    )
    .unwrap();
    let zs: Vec<f64> = positions(&model).iter().map(|p| p.z).collect();
    assert_eq!(zs.iter().filter(|z| (**z - 3.0).abs() < 1e-12).count(), 4);
    assert_eq!(zs.iter().filter(|z| z.abs() < 1e-12).count(), 4);
    assert!(ctx(&model).validate().is_empty());
    assert_eq!(ctx(&model).face_count(), 6);
    assert!(normals_outward(&model));
}

#[test]
fn moving_an_edge_tilts_the_face_it_borders() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let c = ctx(&model);
    let right = c
        .edges()
        .find(|(_, e)| {
            e.ends
                .iter()
                .all(|v| c.vertex(*v).unwrap().position.x == 1.0)
        })
        .map(|(id, _)| id)
        .unwrap();
    run(
        &mut model,
        &mut undo,
        &[Entity::Edge(right)],
        DMat4::from_translation(v(0.0, 0.0, 1.0)),
    )
    .unwrap();
    assert!(ctx(&model).validate().is_empty());
    let n = ctx(&model).face_plane(f).unwrap().normal;
    assert!(n.z.abs() < 0.99 && n.x.abs() > 0.1, "{n:?}");
}

#[test]
fn a_duplicated_selection_moves_once() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let sel = [Entity::Face(f), Entity::Face(f)];
    run(
        &mut model,
        &mut undo,
        &sel,
        DMat4::from_translation(v(1.0, 0.0, 0.0)),
    )
    .unwrap();
    let xs: Vec<f64> = positions(&model).iter().map(|p| p.x).collect();
    assert!(
        xs.iter()
            .all(|x| (*x - 1.0).abs() < 1e-12 || (*x - 2.0).abs() < 1e-12)
    );
}

// -- rotate, scale, flip ----------------------------------------------------------------

#[test]
fn rotate_a_square_a_quarter_turn_about_z() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 1.0, 0.0, 1.0);
    let m = DMat4::from_quat(DQuat::from_axis_angle(
        DVec3::Z,
        std::f64::consts::FRAC_PI_2,
    ));
    let before = positions(&model);
    run(&mut model, &mut undo, &[Entity::Face(f)], m).unwrap();
    for (a, b) in before.iter().zip(positions(&model)) {
        assert_close(m.transform_point3(*a), b);
    }
    assert!(ctx(&model).validate().is_empty());
    assert!((ctx(&model).face_plane(f).unwrap().normal - DVec3::Z).length() < 1e-9);
}

#[test]
fn uniform_scale_about_a_point() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube(&mut model);
    let all: Vec<_> = faces.iter().map(|&f| Entity::Face(f)).collect();
    let centre = v(0.5, 0.5, 0.5);
    let m = DMat4::from_translation(centre)
        * DMat4::from_scale(DVec3::splat(3.0))
        * DMat4::from_translation(-centre);
    run(&mut model, &mut undo, &all, m).unwrap();
    for p in positions(&model) {
        for c in [p.x, p.y, p.z] {
            assert!((c + 1.0).abs() < 1e-9 || (c - 2.0).abs() < 1e-9, "{p:?}");
        }
    }
    assert!(ctx(&model).validate().is_empty());
    assert!(normals_outward(&model));
}

#[test]
fn non_uniform_scale_keeps_faces_planar() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube(&mut model);
    let all: Vec<_> = faces.iter().map(|&f| Entity::Face(f)).collect();
    run(
        &mut model,
        &mut undo,
        &all,
        DMat4::from_scale(v(1.0, 2.0, 3.0)),
    )
    .unwrap();
    let top = positions(&model).iter().map(|p| p.z).fold(0.0, f64::max);
    assert!((top - 3.0).abs() < 1e-9);
    assert!(ctx(&model).validate().is_empty());
    assert!(normals_outward(&model));
}

#[test]
fn flip_a_cube_keeps_normals_outward() {
    for axis in [v(-1.0, 1.0, 1.0), v(1.0, -1.0, 1.0), v(1.0, 1.0, -1.0)] {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let faces = cube(&mut model);
        let all: Vec<_> = faces.iter().map(|&f| Entity::Face(f)).collect();
        run(&mut model, &mut undo, &all, DMat4::from_scale(axis)).unwrap();
        assert!(ctx(&model).validate().is_empty(), "{axis:?}");
        assert_eq!(ctx(&model).face_count(), 6);
        assert!(normals_outward(&model), "{axis:?}");
        assert_eq!(undo.undo_count(), 1);
    }
}

#[test]
fn flip_keeps_each_faces_materials() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let mut edit = ModelEdit::new();
    let red = model.add_material(
        Material {
            name: "red".into(),
            color: [1.0, 0.0, 0.0, 1.0],
            texture: None,
        },
        &mut edit,
    );
    let f = square(&mut model, 0.0, 0.0, 1.0);
    model
        .set_face_materials(
            BodyId::Root,
            f,
            FaceMaterials {
                front: Some(red),
                back: None,
                ..Default::default()
            },
            &mut edit,
        )
        .unwrap();
    run(
        &mut model,
        &mut undo,
        &[Entity::Face(f)],
        DMat4::from_scale(v(-1.0, 1.0, 1.0)),
    )
    .unwrap();
    // The flipped face may have a new id; exactly one face carries the material, in front,
    // and its front still looks up.
    let c = ctx(&model);
    let (nf, _) = c.faces().next().unwrap();
    assert_eq!(c.face_count(), 1);
    assert_eq!(
        model.root().face_materials(nf),
        FaceMaterials {
            front: Some(red),
            back: None,
            ..Default::default()
        }
    );
    assert!(c.face_plane(nf).unwrap().normal.z > 0.99);
    assert!(model.validate().is_empty());
    undo.undo(&mut model).unwrap();
    assert_eq!(
        model.root().face_materials(f),
        FaceMaterials {
            front: Some(red),
            back: None,
            ..Default::default()
        }
    );
}

// -- instances -------------------------------------------------------------------------

#[test]
fn an_instance_gets_the_matrix_on_its_transform() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let mut edit = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "box".into(),
            group: false,
        },
        &mut edit,
    );
    let start = DMat4::from_translation(v(1.0, 0.0, 0.0));
    let id = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: def,
                transform: start,
                name: "i".into(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    let m = DMat4::from_rotation_z(0.5) * DMat4::from_translation(v(0.0, 2.0, 0.0));
    run(&mut model, &mut undo, &[Entity::Instance(id)], m).unwrap();
    let got = model.root().instance(id).unwrap().transform;
    assert!(got.abs_diff_eq(m * start, 1e-12));
    assert_eq!(undo.undo_count(), 1);
    undo.undo(&mut model).unwrap();
    assert_eq!(model.root().instance(id).unwrap().transform, start);
}

// -- refusals --------------------------------------------------------------------------

fn unchanged_after_error(
    model: &mut Model,
    selection: &[Entity],
    m: DMat4,
    expect: impl Fn(&TransformError) -> bool,
) {
    let mut undo = UndoStack::new();
    let before = model.clone();
    let err = run(model, &mut undo, selection, m).unwrap_err();
    assert!(expect(&err), "{err:?}");
    assert_eq!(*model, before);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn empty_selection_is_refused() {
    let mut model = Model::new();
    square(&mut model, 0.0, 0.0, 1.0);
    unchanged_after_error(
        &mut model,
        &[],
        DMat4::from_translation(v(1.0, 0.0, 0.0)),
        |e| *e == TransformError::Empty,
    );
}

#[test]
fn bad_matrices_are_refused() {
    let mut model = Model::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let sel = [Entity::Face(f)];
    for m in [
        DMat4::from_scale(v(1.0, 0.0, 1.0)),
        DMat4::from_scale(v(1.0, 1e-12, 1.0)),
        DMat4::from_translation(v(f64::NAN, 0.0, 0.0)),
        DMat4::from_translation(v(f64::INFINITY, 0.0, 0.0)),
        not_affine(),
    ] {
        unchanged_after_error(&mut model, &sel, m, |e| *e == TransformError::BadMatrix);
    }
}

/// A matrix whose bottom row is not (0, 0, 0, 1).
fn not_affine() -> DMat4 {
    let mut m = DMat4::IDENTITY;
    m.x_axis.w = 0.5;
    m
}

#[test]
fn unknown_entity_and_body_are_refused() {
    let mut model = Model::new();
    square(&mut model, 0.0, 0.0, 1.0);
    let m = DMat4::from_translation(v(1.0, 0.0, 0.0));
    let ghost = Entity::Face(FaceId(999));
    unchanged_after_error(&mut model, &[ghost], m, |e| {
        *e == TransformError::MissingEntity(ghost)
    });
    let mut undo = UndoStack::new();
    let body = BodyId::Definition(DefinitionId(77));
    let err = transform(&mut model, body, &[ghost], m, &mut undo, "Move").unwrap_err();
    assert_eq!(err, TransformError::MissingBody(body));
}

#[test]
fn a_move_that_makes_a_face_non_planar_is_refused() {
    let mut model = Model::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(2.0, 0.0, 0.0),
        v(2.0, 1.0, 0.0),
        v(1.0, 2.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    let f = mesh(&mut model, &pts, &[&[0, 1, 2, 3, 4]])[0];
    let c = ctx(&model);
    let bc = c
        .edges()
        .find(|(_, e)| {
            let p: Vec<_> = e
                .ends
                .iter()
                .map(|v| c.vertex(*v).unwrap().position)
                .collect();
            p.iter().all(|p| p.x == 2.0)
        })
        .map(|(id, _)| id)
        .unwrap();
    unchanged_after_error(
        &mut model,
        &[Entity::Edge(bc)],
        DMat4::from_translation(v(0.0, 0.0, 1.0)),
        |e| *e == TransformError::Invalid(Violation::NonPlanarFace(f)),
    );
}

#[test]
fn a_move_that_crosses_other_edges_is_refused() {
    let mut model = Model::new();
    let a = square(&mut model, 0.0, 0.0, 1.0);
    square(&mut model, 5.0, 0.0, 1.0);
    unchanged_after_error(
        &mut model,
        &[Entity::Face(a)],
        DMat4::from_translation(v(5.5, 0.5, 0.0)),
        |e| matches!(e, TransformError::Invalid(Violation::EdgesIntersect(..))),
    );
}

#[test]
fn a_move_onto_another_vertex_is_refused() {
    let mut model = Model::new();
    let a = square(&mut model, 0.0, 0.0, 1.0);
    square(&mut model, 5.0, 0.0, 1.0);
    unchanged_after_error(
        &mut model,
        &[Entity::Face(a)],
        DMat4::from_translation(v(5.0, 0.0, 0.0)),
        |e| matches!(e, TransformError::Invalid(_)),
    );
}

#[test]
fn existing_violations_do_not_block_a_transform() {
    // A stray vertex is already a violation; moving a square elsewhere is still fine.
    let mut model = Model::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(BodyId::Root, &mut edit, |ctx, log| {
            ctx.insert_vertex(v(9.0, 9.0, 9.0), log);
        })
        .unwrap();
    assert!(!ctx(&model).validate().is_empty());
    let mut undo = UndoStack::new();
    run(
        &mut model,
        &mut undo,
        &[Entity::Face(f)],
        DMat4::from_translation(v(0.0, 0.0, 1.0)),
    )
    .unwrap();
}

#[test]
fn the_identity_changes_nothing_and_adds_no_step() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let before = model.clone();
    run(&mut model, &mut undo, &[Entity::Face(f)], DMat4::IDENTITY).unwrap();
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}
