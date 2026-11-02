// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Make component and make group (`docs/specs/m4-make-component.md`).

use drydock_kernel::component::{ComponentError, make_component, unique_definition_name};
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::topology::{EdgeId, FaceId, VertexId};
use drydock_kernel::units::Unit;
use glam::{DMat4, DVec3};

mod transform_common;
use transform_common::*;

fn counts(model: &Model, body: BodyId) -> (usize, usize, usize) {
    let c = model.body(body).unwrap().geometry();
    (c.vertex_count(), c.edge_count(), c.face_count())
}

fn info(name: &str, group: bool) -> DefinitionInfo {
    DefinitionInfo {
        name: name.into(),
        group,
    }
}

/// A unit cube with its min corner at `at`.
fn cube_at(model: &mut Model, at: DVec3) -> Vec<FaceId> {
    let mut pts = Vec::new();
    for z in [0.0, 1.0] {
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            pts.push(at + v(x, y, z));
        }
    }
    mesh(
        model,
        &pts,
        &[
            &[0, 3, 2, 1],
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[2, 3, 7, 6],
            &[1, 2, 6, 5],
            &[3, 0, 4, 7],
        ],
    )
}

fn all_faces(faces: &[FaceId]) -> Vec<Entity> {
    faces.iter().map(|&f| Entity::Face(f)).collect()
}

fn all_edges(model: &Model) -> Vec<Entity> {
    model
        .root()
        .geometry()
        .edges()
        .map(|(e, _)| Entity::Edge(e))
        .collect()
}

fn definition_of(model: &Model, body: BodyId, inst: InstanceId) -> DefinitionId {
    model.body(body).unwrap().instance(inst).unwrap().definition
}

/// World-space positions of the vertices in the instance's definition, sorted.
fn world_points(model: &Model, body: BodyId, inst: InstanceId) -> Vec<[i64; 3]> {
    let i = model.body(body).unwrap().instance(inst).unwrap();
    let def = model.body(BodyId::Definition(i.definition)).unwrap();
    let mut out: Vec<[i64; 3]> = def
        .geometry()
        .vertices()
        .map(|(_, vx)| key(i.transform.transform_point3(vx.position)))
        .collect();
    out.sort();
    out
}

fn key(p: DVec3) -> [i64; 3] {
    [
        (p.x * 1e6).round() as i64,
        (p.y * 1e6).round() as i64,
        (p.z * 1e6).round() as i64,
    ]
}

fn sorted_keys(points: impl Iterator<Item = DVec3>) -> Vec<[i64; 3]> {
    let mut out: Vec<[i64; 3]> = points.map(key).collect();
    out.sort();
    out
}

fn root_points(model: &Model) -> Vec<[i64; 3]> {
    sorted_keys(
        model
            .root()
            .geometry()
            .vertices()
            .map(|(_, vx)| vx.position),
    )
}

/// Loose edges (no faces) through `pts`, closed when `closed`.
fn loose_edges(model: &mut Model, pts: &[DVec3], closed: bool) -> Vec<EdgeId> {
    let mut log = ModelEdit::new();
    model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            let vs: Vec<VertexId> = pts.iter().map(|&p| ctx.insert_vertex(p, edit)).collect();
            let n = if closed { vs.len() } else { vs.len() - 1 };
            (0..n)
                .map(|i| {
                    ctx.insert_edge(vs[i], vs[(i + 1) % vs.len()], edit)
                        .unwrap()
                })
                .collect()
        })
        .unwrap()
}

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

fn make(model: &mut Model, sel: &[Entity], undo: &mut UndoStack) -> InstanceId {
    let inst = make_component(
        model,
        BodyId::Root,
        sel,
        info("", false),
        undo,
        "Make Component",
    )
    .expect("make component");
    assert!(model.validate().is_empty(), "{:?}", model.validate());
    inst
}

// -- names -------------------------------------------------------------------------------

#[test]
fn definition_names() {
    let mut model = Model::new();
    assert_eq!(unique_definition_name(&model, "", false), "Component");
    assert_eq!(unique_definition_name(&model, "  ", true), "Group");
    assert_eq!(unique_definition_name(&model, " Door ", false), "Door");
    let mut log = ModelEdit::new();
    model.add_definition(info("Door", false), &mut log);
    model.add_definition(info("Door 3", false), &mut log);
    model.add_definition(info("Group", true), &mut log);
    assert_eq!(unique_definition_name(&model, "Door", false), "Door 2");
    assert_eq!(unique_definition_name(&model, "Door", true), "Door 2");
    assert_eq!(unique_definition_name(&model, "", true), "Group 2");
    assert_eq!(unique_definition_name(&model, "", false), "Component");
}

// -- whole solids ------------------------------------------------------------------------

#[test]
fn whole_cube_becomes_one_instance_at_its_corner() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(2.0, 3.0, 0.0));
    let before_points = root_points(&model);
    let mut sel = all_faces(&faces);
    sel.extend(all_edges(&model));
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &sel, &mut undo);

    assert_eq!(counts(&model, BodyId::Root), (0, 0, 0));
    assert_eq!(model.root().instances().count(), 1);
    let i = model.root().instance(inst).unwrap();
    assert_eq!(i.transform, DMat4::from_translation(v(2.0, 3.0, 0.0)));
    assert_eq!(i.name, "");
    assert_eq!(i.material, None);
    let def = BodyId::Definition(i.definition);
    assert_eq!(counts(&model, def), (8, 12, 6));
    let d = model.definition(i.definition).unwrap();
    assert_eq!(d.name, "Component");
    assert!(!d.group);
    for (_, vx) in model.body(def).unwrap().geometry().vertices() {
        let p = vx.position;
        assert!(
            p.min_element() >= -1e-12 && p.max_element() <= 1.0 + 1e-12,
            "{p:?}"
        );
    }
    assert_eq!(world_points(&model, BodyId::Root, inst), before_points);
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Make Component"));
}

#[test]
fn faces_alone_bring_their_edges() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(0.0, 0.0, 0.0));
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &all_faces(&faces), &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (0, 0, 0));
    let def = BodyId::Definition(definition_of(&model, BodyId::Root, inst));
    assert_eq!(counts(&model, def), (8, 12, 6));
    assert_eq!(
        model.root().instance(inst).unwrap().transform,
        DMat4::IDENTITY
    );
}

#[test]
fn copied_faces_keep_their_orientation() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(-4.0, 1.0, 2.0));
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &all_faces(&faces), &mut undo);
    let def = BodyId::Definition(definition_of(&model, BodyId::Root, inst));
    let c = model.body(def).unwrap().geometry();
    // Every face normal points away from the cube's center (0.5, 0.5, 0.5) in definition space.
    for (f, face) in c.faces() {
        let plane = c.face_plane(f).unwrap();
        let pts = c.loop_points(&face.outer);
        let mid = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
        assert!(plane.normal.dot(mid - v(0.5, 0.5, 0.5)) > 0.0, "face {f:?}");
    }
}

#[test]
fn group_flag_and_name() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(0.0, 0.0, 0.0));
    let mut undo = UndoStack::new();
    let inst = make_component(
        &mut model,
        BodyId::Root,
        &all_faces(&faces),
        info("  ", true),
        &mut undo,
        "Make Group",
    )
    .unwrap();
    let d = definition_of(&model, BodyId::Root, inst);
    let di = model.definition(d).unwrap();
    assert!(di.group);
    assert_eq!(di.name, "Group");
    assert!(model.validate().is_empty());
}

#[test]
fn second_component_gets_the_next_name() {
    let mut model = Model::new();
    let a = cube_at(&mut model, v(0.0, 0.0, 0.0));
    let b = cube_at(&mut model, v(3.0, 0.0, 0.0));
    let mut undo = UndoStack::new();
    let ia = make_component(
        &mut model,
        BodyId::Root,
        &all_faces(&a),
        info("Box", false),
        &mut undo,
        "M",
    )
    .unwrap();
    let ib = make_component(
        &mut model,
        BodyId::Root,
        &all_faces(&b),
        info("Box", false),
        &mut undo,
        "M",
    )
    .unwrap();
    let na = &model
        .definition(definition_of(&model, BodyId::Root, ia))
        .unwrap()
        .name;
    let nb = &model
        .definition(definition_of(&model, BodyId::Root, ib))
        .unwrap()
        .name;
    assert_eq!((na.as_str(), nb.as_str()), ("Box", "Box 2"));
    assert_eq!(
        model.root().instance(ib).unwrap().transform,
        DMat4::from_translation(v(3.0, 0.0, 0.0))
    );
    assert_eq!(counts(&model, BodyId::Root), (0, 0, 0));
}

// -- parts of solids -----------------------------------------------------------------------

#[test]
fn top_face_only_leaves_the_rest_of_the_cube() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(0.0, 0.0, 0.0));
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &[Entity::Face(faces[1])], &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (8, 12, 5));
    assert!(model.root().geometry().face(faces[1]).is_none());
    let i = model.root().instance(inst).unwrap();
    assert_eq!(i.transform, DMat4::from_translation(v(0.0, 0.0, 1.0)));
    let def = BodyId::Definition(i.definition);
    assert_eq!(counts(&model, def), (4, 4, 1));
    let c = model.body(def).unwrap().geometry();
    let (f, _) = c.faces().next().unwrap();
    assert!((c.face_plane(f).unwrap().normal - v(0.0, 0.0, 1.0)).length() < 1e-9);
    assert!(c.vertices().all(|(_, vx)| vx.position.z.abs() < 1e-12));
}

#[test]
fn one_of_two_squares_keeps_the_shared_edge() {
    let mut model = Model::new();
    let faces = mesh(
        &mut model,
        &[
            v(0., 0., 0.),
            v(1., 0., 0.),
            v(2., 0., 0.),
            v(2., 1., 0.),
            v(1., 1., 0.),
            v(0., 1., 0.),
        ],
        &[&[0, 1, 4, 5], &[1, 2, 3, 4]],
    );
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &[Entity::Face(faces[0])], &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (4, 4, 1));
    assert!(model.root().geometry().face(faces[1]).is_some());
    assert_eq!(
        root_points(&model),
        sorted_keys([v(1., 0., 0.), v(2., 0., 0.), v(2., 1., 0.), v(1., 1., 0.)].into_iter())
    );
    let def = BodyId::Definition(definition_of(&model, BodyId::Root, inst));
    assert_eq!(counts(&model, def), (4, 4, 1));
}

#[test]
fn a_face_with_a_hole_keeps_it() {
    let mut model = Model::new();
    draw(
        &mut model,
        &[
            &[v(0., 0., 0.), v(4., 0., 0.), v(4., 4., 0.), v(0., 4., 0.)],
            &[v(1., 1., 0.), v(3., 1., 0.), v(3., 3., 0.), v(1., 3., 0.)],
        ],
    );
    let c = model.root().geometry();
    assert_eq!(c.face_count(), 2);
    let (outer, _) = c.faces().find(|(_, f)| f.inner.len() == 1).unwrap();
    let (inner, _) = c.faces().find(|(_, f)| f.inner.is_empty()).unwrap();
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &[Entity::Face(outer)], &mut undo);

    assert_eq!(counts(&model, BodyId::Root), (4, 4, 1));
    assert!(model.root().geometry().face(inner).is_some());
    let def = BodyId::Definition(definition_of(&model, BodyId::Root, inst));
    assert_eq!(counts(&model, def), (8, 8, 1));
    let dc = model.body(def).unwrap().geometry();
    let (_, face) = dc.faces().next().unwrap();
    assert_eq!(face.inner.len(), 1);
    assert_eq!(face.inner[0].uses.len(), 4);
}

#[test]
fn loose_edges_make_no_face() {
    let mut model = Model::new();
    let edges = loose_edges(
        &mut model,
        &[v(0., 0., 0.), v(1., 0., 0.), v(1., 1., 0.), v(0., 1., 0.)],
        true,
    );
    let sel: Vec<Entity> = edges.iter().map(|&e| Entity::Edge(e)).collect();
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &sel, &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (0, 0, 0));
    let def = BodyId::Definition(definition_of(&model, BodyId::Root, inst));
    assert_eq!(counts(&model, def), (4, 4, 0));
}

#[test]
fn part_of_a_loose_polyline() {
    let mut model = Model::new();
    let edges = loose_edges(
        &mut model,
        &[v(0., 0., 0.), v(1., 0., 0.), v(1., 2., 0.), v(1., 2., 3.)],
        false,
    );
    let mut undo = UndoStack::new();
    let inst = make(
        &mut model,
        &[Entity::Edge(edges[1]), Entity::Edge(edges[2])],
        &mut undo,
    );
    // The middle vertex (1, 0, 0) still has the first edge, so it stays in the root.
    assert_eq!(counts(&model, BodyId::Root), (2, 1, 0));
    assert_eq!(
        root_points(&model),
        sorted_keys([v(0., 0., 0.), v(1., 0., 0.)].into_iter())
    );
    let i = model.root().instance(inst).unwrap();
    assert_eq!(i.transform, DMat4::from_translation(v(1.0, 0.0, 0.0)));
    let def = BodyId::Definition(i.definition);
    assert_eq!(counts(&model, def), (3, 2, 0));
    assert_eq!(
        world_points(&model, BodyId::Root, inst),
        sorted_keys([v(1., 0., 0.), v(1., 2., 0.), v(1., 2., 3.)].into_iter())
    );
}

#[test]
fn an_edge_still_used_by_a_face_is_copied_and_stays() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(0.0, 0.0, 0.0));
    let edge = model.root().geometry().edges().next().unwrap().0;
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &[Entity::Edge(edge)], &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (8, 12, 6));
    assert!(
        faces
            .iter()
            .all(|&f| model.root().geometry().face(f).is_some())
    );
    let def = BodyId::Definition(definition_of(&model, BodyId::Root, inst));
    assert_eq!(counts(&model, def), (2, 1, 0));
}

// -- materials and instances ---------------------------------------------------------------

#[test]
fn face_materials_come_along() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(0.0, 0.0, 0.0));
    let mut log = ModelEdit::new();
    let red = model.add_material(Material::new("Red", [1.0, 0.0, 0.0, 1.0]), &mut log);
    let blue = model.add_material(Material::new("Blue", [0.0, 0.0, 1.0, 1.0]), &mut log);
    let painted = FaceMaterials {
        front: Some(red),
        back: Some(blue),
        ..Default::default()
    };
    model
        .set_face_materials(BodyId::Root, faces[1], painted, &mut log)
        .unwrap();
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &all_faces(&faces), &mut undo);
    let def = BodyId::Definition(definition_of(&model, BodyId::Root, inst));
    let body = model.body(def).unwrap();
    let c = body.geometry();
    let top: Vec<FaceId> = c
        .faces()
        .filter(|(f, _)| (c.face_plane(*f).unwrap().normal - v(0., 0., 1.)).length() < 1e-9)
        .map(|(f, _)| f)
        .collect();
    assert_eq!(top.len(), 1);
    assert_eq!(body.face_materials(top[0]), painted);
    let others = c
        .faces()
        .filter(|(f, _)| *f != top[0])
        .all(|(f, _)| body.face_materials(f) == FaceMaterials::default());
    assert!(others);
}

#[test]
fn selected_instances_move_inside_and_stay_in_place() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(1.0, 1.0, 0.0));
    let mut log = ModelEdit::new();
    let red = model.add_material(Material::new("Red", [1.0, 0.0, 0.0, 1.0]), &mut log);
    let chair = model.add_definition(info("Chair", false), &mut log);
    let placed = DMat4::from_translation(v(5.0, 2.0, 0.0)) * DMat4::from_rotation_z(0.5);
    let old = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: chair,
                transform: placed,
                name: "Left chair".into(),
                material: Some(red),
            },
            &mut log,
        )
        .unwrap();
    // The chair's definition is empty, so the selection box is the cube's.
    let mut sel = all_faces(&faces);
    sel.push(Entity::Instance(old));
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &sel, &mut undo);

    assert!(model.root().instance(old).is_none());
    assert_eq!(model.root().instances().count(), 1);
    let outer = model.root().instance(inst).unwrap().clone();
    assert_eq!(outer.transform, DMat4::from_translation(v(1.0, 1.0, 0.0)));
    let body = model.body(BodyId::Definition(outer.definition)).unwrap();
    let inner: Vec<&Instance> = body.instances().map(|(_, i)| i).collect();
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0].definition, chair);
    assert_eq!(inner[0].name, "Left chair");
    assert_eq!(inner[0].material, Some(red));
    assert!((outer.transform * inner[0].transform).abs_diff_eq(placed, 1e-12));
    assert_eq!(model.instances_of(chair).len(), 1);
}

#[test]
fn instance_with_geometry_sets_the_box() {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let post = model.add_definition(info("Post", false), &mut log);
    model
        .edit_geometry(BodyId::Definition(post), &mut log, |ctx, edit| {
            let a = ctx.insert_vertex(v(0., 0., 0.), edit);
            let b = ctx.insert_vertex(v(0., 0., 2.), edit);
            ctx.insert_edge(a, b, edit).unwrap();
        })
        .unwrap();
    let placed = DMat4::from_translation(v(-1.0, 4.0, 0.5));
    let old = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: post,
                transform: placed,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    let faces = cube_at(&mut model, v(0.0, 0.0, 0.0));
    let mut sel = all_faces(&faces);
    sel.push(Entity::Instance(old));
    let mut undo = UndoStack::new();
    let inst = make(&mut model, &sel, &mut undo);
    let outer = model.root().instance(inst).unwrap().clone();
    // Box min: x from the post (-1), y and z from the cube (0, 0).
    assert_eq!(outer.transform, DMat4::from_translation(v(-1.0, 0.0, 0.0)));
    let body = model.body(BodyId::Definition(outer.definition)).unwrap();
    let (_, moved) = body.instances().next().unwrap();
    assert!((outer.transform * moved.transform).abs_diff_eq(placed, 1e-12));
}

#[test]
fn works_inside_a_definition() {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let house = model.add_definition(info("House", false), &mut log);
    model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: house,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    let edges = {
        let mut out = Vec::new();
        model
            .edit_geometry(BodyId::Definition(house), &mut log, |ctx, edit| {
                let a = ctx.insert_vertex(v(2., 0., 0.), edit);
                let b = ctx.insert_vertex(v(3., 0., 0.), edit);
                let c = ctx.insert_vertex(v(3., 0., 1.), edit);
                out.push(ctx.insert_edge(a, b, edit).unwrap());
                out.push(ctx.insert_edge(b, c, edit).unwrap());
            })
            .unwrap();
        out
    };
    let body = BodyId::Definition(house);
    let mut undo = UndoStack::new();
    let inst = make_component(
        &mut model,
        body,
        &[Entity::Edge(edges[0]), Entity::Edge(edges[1])],
        info("Window", false),
        &mut undo,
        "M",
    )
    .unwrap();
    assert!(model.validate().is_empty());
    assert_eq!(counts(&model, body), (0, 0, 0));
    let i = model.body(body).unwrap().instance(inst).unwrap();
    assert_eq!(i.transform, DMat4::from_translation(v(2.0, 0.0, 0.0)));
    assert_eq!(model.root().instances().count(), 1);
}

// -- undo and refusals ---------------------------------------------------------------------

#[test]
fn undo_restores_exactly_and_redo_repeats() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(0.5, -2.0, 1.0));
    let before = model.clone();
    let mut undo = UndoStack::new();
    make(
        &mut model,
        &[Entity::Face(faces[2]), Entity::Face(faces[4])],
        &mut undo,
    );
    let after = model.clone();
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    assert_eq!(model.definitions().count(), 0);
    undo.redo(&mut model).unwrap();
    assert_eq!(model, after);
}

#[test]
fn refusals_change_nothing() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(0.0, 0.0, 0.0));
    let before = model.clone();
    let mut undo = UndoStack::new();
    let f = Entity::Face(faces[0]);
    assert_eq!(
        make_component(
            &mut model,
            BodyId::Root,
            &[],
            info("", false),
            &mut undo,
            "M"
        ),
        Err(ComponentError::Empty)
    );
    let ghost = BodyId::Definition(DefinitionId(42));
    assert_eq!(
        make_component(&mut model, ghost, &[f], info("", false), &mut undo, "M"),
        Err(ComponentError::Model(ModelError::MissingBody(ghost)))
    );
    let missing = Entity::Edge(EdgeId(999));
    assert_eq!(
        make_component(
            &mut model,
            BodyId::Root,
            &[f, missing, Entity::Instance(InstanceId(3))],
            info("", false),
            &mut undo,
            "M"
        ),
        Err(ComponentError::Missing(missing))
    );
    assert_eq!(
        make_component(
            &mut model,
            BodyId::Root,
            &[Entity::Instance(InstanceId(3))],
            info("", false),
            &mut undo,
            "M"
        ),
        Err(ComponentError::Missing(Entity::Instance(InstanceId(3))))
    );
    assert_eq!(model, before);
    assert_eq!(model.definitions().count(), 0);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn duplicates_count_once() {
    let mut model = Model::new();
    let faces = cube_at(&mut model, v(0.0, 0.0, 0.0));
    let mut undo = UndoStack::new();
    let f = Entity::Face(faces[1]);
    let inst = make(&mut model, &[f, f], &mut undo);
    let def = BodyId::Definition(definition_of(&model, BodyId::Root, inst));
    assert_eq!(counts(&model, def), (4, 4, 1));
}
