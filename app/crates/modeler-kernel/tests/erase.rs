// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Erase the selection (`docs/specs/m3-erase.md`).

use glam::{DMat4, DVec3};
use modeler_kernel::erase::{EraseError, erase};
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::topology::{EdgeId, FaceId};
use modeler_kernel::units::Unit;

mod transform_common;
use transform_common::*;

fn counts(model: &Model) -> (usize, usize, usize) {
    let c = model.root().geometry();
    (c.vertices().count(), c.edges().count(), c.faces().count())
}

/// The edge joining the vertices at `a` and `b`.
fn edge_between(model: &Model, a: DVec3, b: DVec3) -> EdgeId {
    let c = model.root().geometry();
    c.edges()
        .find(|(_, e)| {
            let p = c.vertex(e.ends[0]).unwrap().position;
            let q = c.vertex(e.ends[1]).unwrap().position;
            (p.distance(a) < 1e-9 && q.distance(b) < 1e-9)
                || (p.distance(b) < 1e-9 && q.distance(a) < 1e-9)
        })
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no edge {a:?} {b:?}"))
}

fn has_vertex_at(model: &Model, p: DVec3) -> bool {
    model
        .root()
        .geometry()
        .vertices()
        .any(|(_, v)| v.position.distance(p) < 1e-9)
}

/// Draw closed polygons with the sticky drawing path (faces form, holes are punched).
fn draw(model: &mut Model, loops: &[&[DVec3]]) {
    let mut undo = UndoStack::new();
    let mut t = Target {
        model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    for pts in loops {
        let n = pts.len();
        let segs: Vec<_> = (0..n).map(|i| (pts[i], pts[(i + 1) % n])).collect();
        draw_segments(&mut t, &segs, "Draw").expect("draw");
    }
}

/// Two unit squares side by side on the ground, sharing the edge x = 1.
fn two_squares(model: &mut Model) -> Vec<FaceId> {
    mesh(
        model,
        &[
            v(0., 0., 0.),
            v(1., 0., 0.),
            v(2., 0., 0.),
            v(2., 1., 0.),
            v(1., 1., 0.),
            v(0., 1., 0.),
        ],
        &[&[0, 1, 4, 5], &[1, 2, 3, 4]],
    )
}

fn erase_ok(model: &mut Model, undo: &mut UndoStack, selection: &[Entity]) {
    erase(model, BodyId::Root, selection, undo, "Erase").expect("erase");
    assert!(model.validate().is_empty(), "{:?}", model.validate());
}

// -- refusals --------------------------------------------------------------------------

#[test]
fn an_empty_selection_is_refused() {
    let mut model = Model::new();
    cube(&mut model);
    let before = model.clone();
    let mut undo = UndoStack::new();
    assert_eq!(
        erase(&mut model, BodyId::Root, &[], &mut undo, "Erase"),
        Err(EraseError::Empty)
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn a_missing_body_is_refused() {
    let mut model = Model::new();
    let f = cube(&mut model);
    let before = model.clone();
    let mut undo = UndoStack::new();
    let body = BodyId::Definition(DefinitionId(42));
    assert_eq!(
        erase(&mut model, body, &[Entity::Face(f[0])], &mut undo, "Erase"),
        Err(EraseError::Model(ModelError::MissingBody(body)))
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn a_missing_entity_is_refused_and_nothing_changes() {
    let mut model = Model::new();
    let f = cube(&mut model);
    let before = model.clone();
    let mut undo = UndoStack::new();
    let ghost = Entity::Edge(EdgeId(9999));
    let also = Entity::Instance(InstanceId(77));
    assert_eq!(
        erase(
            &mut model,
            BodyId::Root,
            &[Entity::Face(f[1]), ghost, also],
            &mut undo,
            "Erase"
        ),
        Err(EraseError::Missing(ghost))
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
    assert_eq!(
        erase(&mut model, BodyId::Root, &[also], &mut undo, "Erase"),
        Err(EraseError::Missing(also))
    );
    assert_eq!(model, before);
}

// -- faces -----------------------------------------------------------------------------

#[test]
fn erasing_a_face_keeps_its_edges() {
    let mut model = Model::new();
    let f = cube(&mut model);
    let mut undo = UndoStack::new();
    erase_ok(&mut model, &mut undo, &[Entity::Face(f[1])]);
    assert_eq!(counts(&model), (8, 12, 5));
    assert!(model.root().geometry().face(f[1]).is_none());
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Erase"));
}

#[test]
fn erasing_every_face_leaves_a_wireframe() {
    let mut model = Model::new();
    let f = cube(&mut model);
    let mut undo = UndoStack::new();
    let sel: Vec<Entity> = f.iter().map(|&id| Entity::Face(id)).collect();
    erase_ok(&mut model, &mut undo, &sel);
    assert_eq!(counts(&model), (8, 12, 0));
    assert_eq!(undo.undo_count(), 1);
}

#[test]
fn erasing_a_face_keeps_its_material_for_undo() {
    let mut model = Model::new();
    let f = cube(&mut model);
    let mut edit = ModelEdit::new();
    let red = model.add_material(
        Material {
            name: "Red".into(),
            color: [1.0, 0.0, 0.0, 1.0],
            texture: None,
        },
        &mut edit,
    );
    let paint = FaceMaterials {
        front: Some(red),
        back: None,
    };
    model
        .set_face_materials(BodyId::Root, f[2], paint, &mut edit)
        .unwrap();
    let before = model.clone();
    let mut undo = UndoStack::new();
    erase_ok(&mut model, &mut undo, &[Entity::Face(f[2])]);
    assert!(model.root().geometry().face(f[2]).is_none());
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    assert_eq!(model.root().face_materials(f[2]), paint);
}

// -- edges -----------------------------------------------------------------------------

#[test]
fn erasing_an_edge_takes_the_faces_it_bounds() {
    let mut model = Model::new();
    cube(&mut model);
    let e = edge_between(&model, v(0., 0., 1.), v(1., 0., 1.));
    let mut undo = UndoStack::new();
    erase_ok(&mut model, &mut undo, &[Entity::Edge(e)]);
    // Top and front go; the other four faces and eleven edges stay.
    assert_eq!(counts(&model), (8, 11, 4));
    assert!(model.root().geometry().edge(e).is_none());
}

#[test]
fn erasing_the_top_edges_leaves_the_bottom_and_the_posts() {
    let mut model = Model::new();
    cube(&mut model);
    let top = [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)];
    let sel: Vec<Entity> = (0..4)
        .map(|i| Entity::Edge(edge_between(&model, top[i], top[(i + 1) % 4])))
        .collect();
    let mut undo = UndoStack::new();
    erase_ok(&mut model, &mut undo, &sel);
    // Every side uses a top edge: only the bottom face is left, with its 4 edges and the
    // 4 vertical edges. No vertex lost all its edges.
    assert_eq!(counts(&model), (8, 8, 1));
    assert_eq!(undo.undo_count(), 1);
}

#[test]
fn erasing_a_lone_edge_drops_its_vertices() {
    let mut model = Model::new();
    cube(&mut model);
    let a = v(3., 0., 0.);
    let b = v(4., 0., 0.);
    let mut undo = UndoStack::new();
    let mut t = Target {
        model: &mut model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut t, &[(a, b)], "Draw").unwrap();
    let e = edge_between(&model, a, b);
    let mut undo = UndoStack::new();
    erase_ok(&mut model, &mut undo, &[Entity::Edge(e)]);
    assert_eq!(counts(&model), (8, 12, 6));
    assert!(!has_vertex_at(&model, a));
    assert!(!has_vertex_at(&model, b));
}

#[test]
fn erasing_the_edge_between_coplanar_faces_merges_them() {
    let mut model = Model::new();
    two_squares(&mut model);
    let shared = edge_between(&model, v(1., 0., 0.), v(1., 1., 0.));
    let mut undo = UndoStack::new();
    erase_ok(&mut model, &mut undo, &[Entity::Edge(shared)]);
    assert_eq!(counts(&model), (6, 6, 1));
    let c = model.root().geometry();
    let (_, face) = c.faces().next().unwrap();
    assert_eq!(face.outer.uses.len(), 6);
    assert!(face.inner.is_empty());
}

#[test]
fn selected_faces_go_before_their_edges_so_nothing_merges() {
    let mut model = Model::new();
    let f = two_squares(&mut model);
    let shared = edge_between(&model, v(1., 0., 0.), v(1., 1., 0.));
    let mut undo = UndoStack::new();
    erase_ok(
        &mut model,
        &mut undo,
        &[Entity::Edge(shared), Entity::Face(f[1])],
    );
    // The selected face goes first; then the shared edge bounds only the other face, which
    // goes with it (no merge). The edge's ends keep other edges.
    assert_eq!(counts(&model), (6, 6, 0));
    assert_eq!(undo.undo_count(), 1);
}

#[test]
fn erasing_an_edge_of_an_inner_rectangle_closes_the_hole() {
    let mut model = Model::new();
    draw(
        &mut model,
        &[
            &[v(0., 0., 0.), v(4., 0., 0.), v(4., 4., 0.), v(0., 4., 0.)],
            &[v(1., 1., 0.), v(3., 1., 0.), v(3., 3., 0.), v(1., 3., 0.)],
        ],
    );
    assert_eq!(counts(&model), (8, 8, 2));
    let e = edge_between(&model, v(1., 1., 0.), v(3., 1., 0.));
    let mut undo = UndoStack::new();
    erase_ok(&mut model, &mut undo, &[Entity::Edge(e)]);
    // The inner face goes; the outer face loses its hole; the other three inner edges stay.
    assert_eq!(counts(&model), (8, 7, 1));
    let c = model.root().geometry();
    let (_, face) = c.faces().next().unwrap();
    assert_eq!(face.outer.uses.len(), 4);
    assert!(face.inner.is_empty());
}

#[test]
fn erasing_the_inner_face_keeps_the_hole() {
    let mut model = Model::new();
    draw(
        &mut model,
        &[
            &[v(0., 0., 0.), v(4., 0., 0.), v(4., 4., 0.), v(0., 4., 0.)],
            &[v(1., 1., 0.), v(3., 1., 0.), v(3., 3., 0.), v(1., 3., 0.)],
        ],
    );
    let c = model.root().geometry();
    let inner = c
        .faces()
        .find(|(_, f)| f.inner.is_empty())
        .map(|(id, _)| id)
        .unwrap();
    let mut undo = UndoStack::new();
    erase_ok(&mut model, &mut undo, &[Entity::Face(inner)]);
    assert_eq!(counts(&model), (8, 8, 1));
    let c = model.root().geometry();
    let (_, face) = c.faces().next().unwrap();
    assert_eq!(face.inner.len(), 1);
}

// -- instances -------------------------------------------------------------------------

#[test]
fn erasing_an_instance_keeps_its_definition() {
    let mut model = Model::new();
    cube(&mut model);
    let mut edit = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "Peg".into(),
            group: false,
        },
        &mut edit,
    );
    let inst = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: def,
                transform: DMat4::from_translation(v(5., 0., 0.)),
                name: String::new(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    let before = model.clone();
    let mut undo = UndoStack::new();
    erase_ok(&mut model, &mut undo, &[Entity::Instance(inst)]);
    assert!(model.root().instance(inst).is_none());
    assert!(model.definition(def).is_some());
    assert_eq!(counts(&model), (8, 12, 6));
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
}

// -- undo ------------------------------------------------------------------------------

#[test]
fn a_mixed_selection_is_one_undo_step_and_undo_redo_round_trip() {
    let mut model = Model::new();
    let f = cube(&mut model);
    two_squares_far(&mut model);
    let e = edge_between(&model, v(1., 1., 0.), v(1., 1., 1.));
    let before = model.clone();
    let mut undo = UndoStack::new();
    let sel = [Entity::Face(f[0]), Entity::Edge(e), Entity::Face(f[0])];
    erase_ok(&mut model, &mut undo, &sel);
    let after = model.clone();
    assert_eq!(undo.undo_count(), 1);
    // Bottom goes (selected), back and right go (the edge), the far squares stay.
    assert_eq!(model.root().geometry().faces().count(), 3 + 2);
    assert_eq!(undo.undo(&mut model).unwrap().as_deref(), Some("Erase"));
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(model, after);
}

/// Two squares well away from the unit cube.
fn two_squares_far(model: &mut Model) {
    draw(
        model,
        &[
            &[v(5., 0., 0.), v(6., 0., 0.), v(6., 1., 0.), v(5., 1., 0.)],
            &[v(5., 3., 0.), v(6., 3., 0.), v(6., 4., 0.), v(5., 4., 0.)],
        ],
    );
}
