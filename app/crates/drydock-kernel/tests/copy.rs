// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Copy a selection, clipboard snapshots and paste (`docs/specs/m4-copy.md`).

use std::collections::BTreeMap;

use drydock_kernel::copy::{Clipboard, CopyError, copy_selection, extract, insert};
use drydock_kernel::model::*;
use drydock_kernel::paint::Side;
use drydock_kernel::select::Entity;
use drydock_kernel::texture::{TextureMap, side_map};
use drydock_kernel::topology::*;
use glam::{DMat4, DQuat, DVec2, DVec3, DVec4};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Faces from point indices into `body` with the raw primitives (shared edges made once).
fn mesh_in(model: &mut Model, body: BodyId, points: &[DVec3], faces: &[&[usize]]) -> Vec<FaceId> {
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
            let mut out = Vec::new();
            for face in faces {
                let mut uses = Vec::new();
                for i in 0..face.len() {
                    let (a, b) = (face[i], face[(i + 1) % face.len()]);
                    let key = (a.min(b), a.max(b));
                    let e = *edges
                        .entry(key)
                        .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], log).unwrap());
                    uses.push(if a < b {
                        EdgeUse::forward(e)
                    } else {
                        EdgeUse::backward(e)
                    });
                }
                out.push(
                    ctx.insert_face(
                        Face {
                            outer: Loop { uses },
                            inner: vec![],
                        },
                        log,
                    )
                    .unwrap(),
                );
            }
            out
        })
        .unwrap()
}

/// One loose edge from `a` to `b` in `body`.
fn edge_in(model: &mut Model, body: BodyId, a: DVec3, b: DVec3) -> EdgeId {
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let a = ctx.insert_vertex(a, log);
            let b = ctx.insert_vertex(b, log);
            ctx.insert_edge(a, b, log).unwrap()
        })
        .unwrap()
}

fn cube_points(at: DVec3) -> Vec<DVec3> {
    let mut pts = Vec::new();
    for z in [0.0, 1.0] {
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            pts.push(at + v(x, y, z));
        }
    }
    pts
}

const CUBE_FACES: [&[usize]; 6] = [
    &[0, 3, 2, 1], // bottom, normal -z
    &[4, 5, 6, 7], // top, normal +z
    &[0, 1, 5, 4], // front, normal -y
    &[2, 3, 7, 6], // back, normal +y
    &[1, 2, 6, 5], // right, normal +x
    &[3, 0, 4, 7], // left, normal -x
];

fn cube_in(model: &mut Model, body: BodyId, at: DVec3) -> Vec<FaceId> {
    mesh_in(model, body, &cube_points(at), &CUBE_FACES)
}

/// A rectangle face (normal +z) with corners `a` and `b` at height `z`.
fn rect_in(model: &mut Model, body: BodyId, a: DVec2, b: DVec2, z: f64) -> FaceId {
    let pts = [
        v(a.x, a.y, z),
        v(b.x, a.y, z),
        v(b.x, b.y, z),
        v(a.x, b.y, z),
    ];
    mesh_in(model, body, &pts, &[&[0, 1, 2, 3]])[0]
}

fn definition(model: &mut Model, name: &str, group: bool) -> DefinitionId {
    let mut edit = ModelEdit::new();
    model.add_definition(
        DefinitionInfo {
            name: name.into(),
            group,
        },
        &mut edit,
    )
}

/// A unit cube definition (min corner at the origin, normals out).
fn cube_definition(model: &mut Model, name: &str, group: bool) -> (DefinitionId, Vec<FaceId>) {
    let d = definition(model, name, group);
    let faces = cube_in(model, BodyId::Definition(d), DVec3::ZERO);
    (d, faces)
}

fn place(model: &mut Model, body: BodyId, d: DefinitionId, m: DMat4) -> InstanceId {
    let mut edit = ModelEdit::new();
    model
        .add_instance(
            body,
            Instance {
                definition: d,
                transform: m,
                name: String::new(),
                material: None,
            },
            &mut edit,
        )
        .unwrap()
}

fn material(model: &mut Model, name: &str, color: [f32; 4]) -> MaterialId {
    let mut edit = ModelEdit::new();
    model.add_material(Material::new(name, color), &mut edit)
}

const RED: [f32; 4] = [0.9, 0.1, 0.1, 1.0];
const BLUE: [f32; 4] = [0.1, 0.1, 0.9, 1.0];

fn set(model: &mut Model, body: BodyId, face: FaceId, materials: FaceMaterials) {
    let mut edit = ModelEdit::new();
    model
        .set_face_materials(body, face, materials, &mut edit)
        .unwrap();
}

fn front(material: MaterialId) -> FaceMaterials {
    FaceMaterials {
        front: Some(material),
        ..FaceMaterials::default()
    }
}

fn counts(model: &Model, body: BodyId) -> (usize, usize, usize) {
    let c = model.body(body).unwrap().geometry();
    (c.vertex_count(), c.edge_count(), c.face_count())
}

fn faces_of(faces: &[FaceId]) -> Vec<Entity> {
    faces.iter().map(|&f| Entity::Face(f)).collect()
}

fn translate(x: f64, y: f64, z: f64) -> DMat4 {
    DMat4::from_translation(v(x, y, z))
}

fn centroid(model: &Model, body: BodyId, face: FaceId) -> DVec3 {
    let ctx = model.body(body).unwrap().geometry();
    let pts = ctx.loop_points(&ctx.face(face).unwrap().outer);
    pts.iter().copied().sum::<DVec3>() / pts.len() as f64
}

fn face_at(model: &Model, body: BodyId, c: DVec3) -> FaceId {
    let ctx = model.body(body).unwrap().geometry();
    ctx.faces()
        .map(|(id, _)| id)
        .find(|&id| centroid(model, body, id).distance(c) < 1e-6)
        .unwrap_or_else(|| panic!("no face with centroid {c:?}"))
}

fn normal(model: &Model, body: BodyId, face: FaceId) -> DVec3 {
    model
        .body(body)
        .unwrap()
        .geometry()
        .face_plane(face)
        .unwrap()
        .normal
}

/// Every face of `body` on exactly one of the unit cubes centred at `centres` points out of
/// that cube.
fn normals_out(model: &Model, body: BodyId, centres: &[DVec3]) -> bool {
    let ctx = model.body(body).unwrap().geometry();
    ctx.faces().all(|(f, _)| {
        let c = centroid(model, body, f);
        let on: Vec<DVec3> = centres
            .iter()
            .copied()
            .filter(|centre| ((c - *centre).length() - 0.5).abs() < 1e-9)
            .collect();
        on.len() != 1 || normal(model, body, f).dot(c - on[0]) > 0.0
    })
}

fn copied(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    m: DMat4,
    undo: &mut UndoStack,
) -> Vec<Entity> {
    let out = copy_selection(model, body, selection, m, undo, "Copy").expect("copy");
    assert_eq!(model.validate(), vec![], "valid after copy");
    out
}

fn kinds(out: &[Entity]) -> (usize, usize, usize) {
    let edges = out.iter().filter(|e| matches!(e, Entity::Edge(_))).count();
    let faces = out.iter().filter(|e| matches!(e, Entity::Face(_))).count();
    let instances = out
        .iter()
        .filter(|e| matches!(e, Entity::Instance(_)))
        .count();
    (edges, faces, instances)
}

// -- extract ------------------------------------------------------------------------------

#[test]
fn extract_refuses_empty_missing_body_and_missing_entities() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    assert_eq!(
        extract(&model, BodyId::Root, &[]).unwrap_err(),
        CopyError::Empty
    );
    let nowhere = BodyId::Definition(DefinitionId(77));
    assert_eq!(
        extract(&model, nowhere, &[Entity::Face(faces[0])]).unwrap_err(),
        CopyError::Model(ModelError::MissingBody(nowhere))
    );
    let gone = Entity::Edge(EdgeId(9999));
    assert_eq!(
        extract(&model, BodyId::Root, &[Entity::Face(faces[0]), gone]).unwrap_err(),
        CopyError::Missing(gone)
    );
}

#[test]
fn extract_takes_a_face_with_its_edges_and_leaves_the_model_alone() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    let before = model.clone();
    let clip = extract(&model, BodyId::Root, &[Entity::Face(faces[1])]).unwrap();
    assert_eq!(model, before);
    assert_eq!(counts(clip.model(), BodyId::Root), (4, 4, 1));
    assert!(!clip.is_empty());
    let ctx = clip.model().root().geometry();
    let (f, _) = ctx.faces().next().unwrap();
    assert!((ctx.face_plane(f).unwrap().normal - DVec3::Z).length() < 1e-9);
    assert!((centroid(clip.model(), BodyId::Root, f) - v(0.5, 0.5, 1.0)).length() < 1e-9);
    assert_eq!(clip.model().definitions().count(), 0);
    assert_eq!(clip.model().materials().count(), 0);
}

#[test]
fn extract_counts_duplicates_once_and_adds_selected_edges() {
    let mut model = Model::new();
    let f = rect_in(&mut model, BodyId::Root, DVec2::ZERO, DVec2::ONE, 0.0);
    let e = edge_in(&mut model, BodyId::Root, v(5.0, 0.0, 0.0), v(6.0, 0.0, 0.0));
    let clip = extract(
        &model,
        BodyId::Root,
        &[Entity::Face(f), Entity::Edge(e), Entity::Face(f)],
    )
    .unwrap();
    assert_eq!(counts(clip.model(), BodyId::Root), (6, 5, 1));
}

#[test]
fn extract_keeps_face_materials_and_only_the_materials_used() {
    let mut model = Model::new();
    let red = material(&mut model, "red", RED);
    let _unused = material(&mut model, "unused", BLUE);
    let f = rect_in(&mut model, BodyId::Root, DVec2::ZERO, DVec2::ONE, 0.0);
    let map = TextureMap {
        u: DVec4::new(1.0, 0.5, 0.0, 0.25),
        v: DVec4::new(0.0, 1.0, 0.0, -1.0),
    };
    let materials = FaceMaterials {
        front: Some(red),
        back: None,
        front_map: Some(map),
        back_map: None,
    };
    set(&mut model, BodyId::Root, f, materials);
    let clip = extract(&model, BodyId::Root, &[Entity::Face(f)]).unwrap();
    let cm = clip.model();
    let (cf, _) = cm.root().geometry().faces().next().unwrap();
    assert_eq!(cm.root().face_materials(cf), materials);
    let got: Vec<(MaterialId, Material)> = cm.materials().map(|(id, m)| (id, m.clone())).collect();
    assert_eq!(got, vec![(red, model.material(red).unwrap().clone())]);
}

#[test]
fn extract_instance_brings_every_reached_definition_exactly() {
    let mut model = Model::new();
    let red = material(&mut model, "red", RED);
    let blue = material(&mut model, "blue", BLUE);
    let _spare = material(&mut model, "spare", [0.5; 4]);
    let (leg, leg_faces) = cube_definition(&mut model, "leg", false);
    set(
        &mut model,
        BodyId::Definition(leg),
        leg_faces[0],
        front(red),
    );
    let table = definition(&mut model, "table", false);
    rect_in(
        &mut model,
        BodyId::Definition(table),
        DVec2::ZERO,
        DVec2::splat(2.0),
        1.0,
    );
    let mut edit = ModelEdit::new();
    model
        .add_instance(
            BodyId::Definition(table),
            Instance {
                definition: leg,
                transform: DMat4::from_scale(v(0.1, 0.1, 1.0)),
                name: "leg".into(),
                material: Some(blue),
            },
            &mut edit,
        )
        .unwrap();
    let (_other, _) = cube_definition(&mut model, "other", false);
    let inst = place(&mut model, BodyId::Root, table, translate(4.0, 0.0, 0.0));

    let clip = extract(&model, BodyId::Root, &[Entity::Instance(inst)]).unwrap();
    let cm = clip.model();
    let defs: Vec<DefinitionId> = cm.definitions().map(|(id, _)| id).collect();
    let mut want = vec![leg, table];
    want.sort();
    assert_eq!(defs, want, "only the reached definitions, same ids");
    for d in [leg, table] {
        assert_eq!(cm.definition(d), model.definition(d));
        assert_eq!(
            cm.body(BodyId::Definition(d)),
            model.body(BodyId::Definition(d))
        );
    }
    let mats: Vec<MaterialId> = cm.materials().map(|(id, _)| id).collect();
    let mut want = vec![red, blue];
    want.sort();
    assert_eq!(
        mats, want,
        "materials used inside the definitions come along"
    );
    let (_, copy) = cm.root().instances().next().unwrap();
    assert_eq!(copy, model.root().instance(inst).unwrap());
    assert_eq!(counts(cm, BodyId::Root), (0, 0, 0));
}

#[test]
fn bounds_cover_geometry_and_instances() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let inst = place(
        &mut model,
        BodyId::Root,
        d,
        translate(5.0, 0.0, 0.0) * DMat4::from_scale(DVec3::splat(2.0)),
    );
    let e = edge_in(
        &mut model,
        BodyId::Root,
        v(-1.0, 0.5, 0.0),
        v(0.0, 0.5, 3.0),
    );
    let clip = extract(
        &model,
        BodyId::Root,
        &[Entity::Edge(e), Entity::Instance(inst)],
    )
    .unwrap();
    let (min, max) = clip.bounds().unwrap();
    assert!((min - v(-1.0, 0.0, 0.0)).length() < 1e-9, "{min:?}");
    assert!((max - v(7.0, 2.0, 3.0)).length() < 1e-9, "{max:?}");
}

#[test]
fn bounds_of_an_instance_of_an_empty_definition_are_none() {
    let mut model = Model::new();
    let d = definition(&mut model, "empty", false);
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let clip = extract(&model, BodyId::Root, &[Entity::Instance(inst)]).unwrap();
    assert!(!clip.is_empty());
    assert_eq!(clip.bounds(), None);
}

// -- copy_selection -----------------------------------------------------------------------

#[test]
fn cube_copied_apart() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    let mut undo = UndoStack::new();
    let out = copied(
        &mut model,
        BodyId::Root,
        &faces_of(&faces),
        translate(3.0, 0.0, 0.0),
        &mut undo,
    );
    assert_eq!(counts(&model, BodyId::Root), (16, 24, 12));
    assert_eq!(kinds(&out), (12, 6, 0));
    let mut sorted = out.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(out, sorted, "result sorted, no duplicates");
    let ctx = model.root().geometry();
    for e in &out {
        match *e {
            Entity::Face(f) => {
                let c = centroid(&model, BodyId::Root, f);
                assert!(c.x > 2.9 && c.x < 4.1, "copied face at {c:?}");
            }
            Entity::Edge(id) => {
                let edge = ctx.edge(id).unwrap();
                for end in edge.ends {
                    let p = ctx.vertex(end).unwrap().position;
                    assert!(p.x > 2.9 && p.x < 4.1);
                }
            }
            Entity::Instance(_) => unreachable!(),
        }
    }
    for &f in &faces {
        assert!(ctx.face(f).is_some(), "originals stay");
    }
    assert!(normals_out(
        &model,
        BodyId::Root,
        &[v(0.5, 0.5, 0.5), v(3.5, 0.5, 0.5)]
    ));
    assert_eq!(undo.undo_label(), Some("Copy"));
    assert_eq!(undo.undo_count(), 1);
}

#[test]
fn cube_copied_touching_shares_the_square() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    let mut undo = UndoStack::new();
    let out = copied(
        &mut model,
        BodyId::Root,
        &faces_of(&faces),
        translate(1.0, 0.0, 0.0),
        &mut undo,
    );
    assert_eq!(counts(&model, BodyId::Root), (12, 20, 11));
    assert_eq!(kinds(&out), (12, 6, 0));
}

#[test]
fn loose_edge_copied_up() {
    let mut model = Model::new();
    let e = edge_in(&mut model, BodyId::Root, DVec3::ZERO, v(1.0, 0.0, 0.0));
    let mut undo = UndoStack::new();
    let out = copied(
        &mut model,
        BodyId::Root,
        &[Entity::Edge(e)],
        translate(0.0, 0.0, 1.0),
        &mut undo,
    );
    assert_eq!(counts(&model, BodyId::Root), (4, 2, 0));
    assert_eq!(kinds(&out), (1, 0, 0));
    assert!(!out.contains(&Entity::Edge(e)));
}

#[test]
fn copy_onto_a_larger_face_splits_it() {
    let mut model = Model::new();
    let big = rect_in(
        &mut model,
        BodyId::Root,
        DVec2::ZERO,
        DVec2::splat(10.0),
        0.0,
    );
    let small = rect_in(
        &mut model,
        BodyId::Root,
        v(20.0, 0.0, 0.0).truncate(),
        DVec2::new(21.0, 1.0),
        0.0,
    );
    let mut undo = UndoStack::new();
    let out = copied(
        &mut model,
        BodyId::Root,
        &[Entity::Face(small)],
        translate(-16.0, 4.0, 0.0),
        &mut undo,
    );
    // The big face now has a hole where the copy sits, and the copy is a face of its own.
    assert_eq!(model.root().geometry().face_count(), 3);
    assert_eq!(kinds(&out), (4, 1, 0));
    let Entity::Face(copy) = out[4] else {
        panic!("faces sort after edges: {out:?}")
    };
    assert!((centroid(&model, BodyId::Root, copy) - v(4.5, 4.5, 0.0)).length() < 1e-9);
    assert!((normal(&model, BodyId::Root, copy) - DVec3::Z).length() < 1e-9);
    let ring = model
        .root()
        .geometry()
        .faces()
        .find(|(_, face)| face.inner.len() == 1)
        .map(|(id, _)| id);
    assert!(ring.is_some(), "the big face {big:?} got a hole");
}

#[test]
fn undo_restores_exactly_and_redo_reapplies() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    let red = material(&mut model, "red", RED);
    set(&mut model, BodyId::Root, faces[1], front(red));
    let (d, _) = cube_definition(&mut model, "box", false);
    let inst = place(&mut model, BodyId::Root, d, translate(0.0, 3.0, 0.0));
    let mut selection = faces_of(&faces);
    selection.push(Entity::Instance(inst));
    let before = model.clone();
    let mut undo = UndoStack::new();
    copied(
        &mut model,
        BodyId::Root,
        &selection,
        translate(1.0, 0.0, 0.0),
        &mut undo,
    );
    let after = model.clone();
    assert_eq!(undo.undo(&mut model).unwrap().as_deref(), Some("Copy"));
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(model, after);
}

#[test]
fn errors_change_nothing() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    let before = model.clone();
    let mut undo = UndoStack::new();
    let sel = faces_of(&faces);
    let mut nan = DMat4::IDENTITY;
    nan.w_axis.x = f64::NAN;
    let mut projective = DMat4::IDENTITY;
    projective.x_axis.w = 0.5;
    let cases: Vec<(Vec<Entity>, DMat4, CopyError)> = vec![
        (vec![], translate(1.0, 0.0, 0.0), CopyError::Empty),
        (
            vec![Entity::Face(FaceId(9999))],
            translate(1.0, 0.0, 0.0),
            CopyError::Missing(Entity::Face(FaceId(9999))),
        ),
        (sel.clone(), nan, CopyError::BadMatrix),
        (
            sel.clone(),
            DMat4::from_scale(v(1.0, 1.0, 0.0)),
            CopyError::BadMatrix,
        ),
        (sel.clone(), projective, CopyError::BadMatrix),
    ];
    for (selection, m, want) in cases {
        let got = copy_selection(&mut model, BodyId::Root, &selection, m, &mut undo, "Copy");
        assert_eq!(got, Err(want));
        assert_eq!(model, before);
        assert_eq!(undo.undo_count(), 0);
    }
    let nowhere = BodyId::Definition(DefinitionId(77));
    assert_eq!(
        copy_selection(
            &mut model,
            nowhere,
            &sel,
            DMat4::IDENTITY,
            &mut undo,
            "Copy"
        ),
        Err(CopyError::Model(ModelError::MissingBody(nowhere)))
    );
}

#[test]
fn copied_component_instance_shares_its_definition() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let red = material(&mut model, "red", RED);
    let mut edit = ModelEdit::new();
    let inst = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: translate(1.0, 2.0, 0.0),
                name: "crate".into(),
                material: Some(red),
            },
            &mut edit,
        )
        .unwrap();
    let mut undo = UndoStack::new();
    let m = translate(0.0, 5.0, 0.0);
    let out = copied(
        &mut model,
        BodyId::Root,
        &[Entity::Instance(inst)],
        m,
        &mut undo,
    );
    assert_eq!(kinds(&out), (0, 0, 1));
    let Entity::Instance(new) = out[0] else {
        unreachable!()
    };
    assert_ne!(new, inst);
    assert_eq!(model.definitions().count(), 1);
    assert_eq!(model.materials().count(), 1);
    let copy = model.root().instance(new).unwrap();
    assert_eq!(copy.definition, d);
    assert_eq!(copy.transform, m * translate(1.0, 2.0, 0.0));
    assert_eq!(copy.name, "crate");
    assert_eq!(copy.material, Some(red));
    assert_eq!(model.instances_of(d).len(), 2);
}

#[test]
fn copied_group_gets_a_definition_of_its_own() {
    let mut model = Model::new();
    let (g, _) = cube_definition(&mut model, "Group", true);
    let inst = place(&mut model, BodyId::Root, g, DMat4::IDENTITY);
    let mut undo = UndoStack::new();
    let out = copied(
        &mut model,
        BodyId::Root,
        &[Entity::Instance(inst)],
        translate(2.0, 0.0, 0.0),
        &mut undo,
    );
    let Entity::Instance(new) = out[0] else {
        unreachable!()
    };
    let d2 = model.root().instance(new).unwrap().definition;
    assert_ne!(d2, g);
    assert_eq!(
        model.definition(d2),
        Some(&DefinitionInfo {
            name: "Group 2".into(),
            group: true
        })
    );
    assert_eq!(counts(&model, BodyId::Definition(d2)), (8, 12, 6));
    assert_eq!(model.instances_of(g), vec![(BodyId::Root, inst)]);
    assert_eq!(model.instances_of(d2), vec![(BodyId::Root, new)]);
}

#[test]
fn paste_into_another_model_brings_definitions_and_materials() {
    let mut source = Model::new();
    let red = material(&mut source, "red", RED);
    let (leg, faces) = cube_definition(&mut source, "leg", false);
    set(&mut source, BodyId::Definition(leg), faces[1], front(red));
    let (g, _) = cube_definition(&mut source, "Group", true);
    let table = definition(&mut source, "table", false);
    place(&mut source, BodyId::Definition(table), leg, DMat4::IDENTITY);
    place(
        &mut source,
        BodyId::Definition(table),
        leg,
        translate(3.0, 0.0, 0.0),
    );
    place(&mut source, BodyId::Definition(table), g, DMat4::IDENTITY);
    let inst = place(&mut source, BodyId::Root, table, DMat4::IDENTITY);
    let top = rect_in(&mut source, BodyId::Root, DVec2::ZERO, DVec2::ONE, 5.0);
    set(&mut source, BodyId::Root, top, front(red));
    let clip: Clipboard = extract(
        &source,
        BodyId::Root,
        &[Entity::Instance(inst), Entity::Face(top)],
    )
    .unwrap();

    let mut target = Model::new();
    let mut undo = UndoStack::new();
    let out = insert(
        &mut target,
        BodyId::Root,
        &clip,
        translate(0.0, 0.0, 1.0),
        &mut undo,
        "Paste",
    )
    .unwrap();
    assert_eq!(target.validate(), vec![]);
    assert_eq!(kinds(&out), (4, 1, 1));
    let mut names: Vec<(String, bool)> = target
        .definitions()
        .map(|(_, info)| (info.name.clone(), info.group))
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            ("Group".to_string(), true),
            ("leg".to_string(), false),
            ("table".to_string(), false)
        ]
    );
    let mats: Vec<Material> = target.materials().map(|(_, m)| m.clone()).collect();
    assert_eq!(mats, vec![source.material(red).unwrap().clone()]);
    let (_, placed) = target.root().instances().next().unwrap();
    let t = BodyId::Definition(placed.definition);
    assert_eq!(target.body(t).unwrap().instances().count(), 3);
    assert_eq!(counts(&target, t), (0, 0, 0));
    let new_red = target.materials().next().unwrap().0;
    let f = face_at(&target, BodyId::Root, v(0.5, 0.5, 6.0));
    assert_eq!(target.root().face_materials(f).front, Some(new_red));
    assert_eq!(undo.undo_label(), Some("Paste"));
}

#[test]
fn paste_after_the_definition_changed_brings_its_own_copy() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let clip = extract(&model, BodyId::Root, &[Entity::Instance(inst)]).unwrap();
    // Edit the definition after copying.
    edge_in(
        &mut model,
        BodyId::Definition(d),
        v(0.0, 0.0, 2.0),
        v(1.0, 0.0, 2.0),
    );
    let mut undo = UndoStack::new();
    let out = insert(
        &mut model,
        BodyId::Root,
        &clip,
        translate(4.0, 0.0, 0.0),
        &mut undo,
        "Paste",
    )
    .unwrap();
    let Entity::Instance(new) = out[0] else {
        unreachable!()
    };
    let d2 = model.root().instance(new).unwrap().definition;
    assert_ne!(d2, d);
    assert_eq!(model.definition(d2).unwrap().name, "box 2");
    assert_eq!(counts(&model, BodyId::Definition(d2)), (8, 12, 6));
    assert_eq!(counts(&model, BodyId::Definition(d)), (10, 13, 6));
}

#[test]
fn materials_map_by_id_then_by_value_then_get_a_new_name() {
    let mut source = Model::new();
    let red = material(&mut source, "red", RED);
    let blue = material(&mut source, "blue", BLUE);
    let a = rect_in(&mut source, BodyId::Root, DVec2::ZERO, DVec2::ONE, 0.0);
    let b = rect_in(
        &mut source,
        BodyId::Root,
        DVec2::new(2.0, 0.0),
        DVec2::new(3.0, 1.0),
        0.0,
    );
    set(&mut source, BodyId::Root, a, front(red));
    set(&mut source, BodyId::Root, b, front(blue));
    let clip = extract(&source, BodyId::Root, &[Entity::Face(a), Entity::Face(b)]).unwrap();

    // The target has a different "red" under red's id and an equal "blue" under another id.
    let mut target = Model::new();
    let other_red = material(&mut target, "red", [0.5, 0.0, 0.0, 1.0]);
    let filler = material(&mut target, "filler", [0.0; 4]);
    let target_blue = material(&mut target, "blue", BLUE);
    assert_eq!(other_red, red);
    assert_eq!(filler, blue);
    let mut undo = UndoStack::new();
    insert(
        &mut target,
        BodyId::Root,
        &clip,
        translate(0.0, 5.0, 0.0),
        &mut undo,
        "Paste",
    )
    .unwrap();
    assert_eq!(target.materials().count(), 4);
    let fa = face_at(&target, BodyId::Root, v(0.5, 5.5, 0.0));
    let fb = face_at(&target, BodyId::Root, v(2.5, 5.5, 0.0));
    let ma = target.root().face_materials(fa).front.unwrap();
    assert_ne!(ma, other_red);
    assert_eq!(target.material(ma).unwrap().name, "red 2");
    assert_eq!(target.material(ma).unwrap().color, RED);
    assert_eq!(target.root().face_materials(fb).front, Some(target_blue));
}

#[test]
fn pasting_into_the_same_model_adds_no_materials_or_definitions() {
    let mut model = Model::new();
    let red = material(&mut model, "red", RED);
    let (d, faces) = cube_definition(&mut model, "box", false);
    set(&mut model, BodyId::Definition(d), faces[0], front(red));
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let f = rect_in(
        &mut model,
        BodyId::Root,
        v(3.0, 0.0, 0.0).truncate(),
        DVec2::new(4.0, 1.0),
        0.0,
    );
    set(&mut model, BodyId::Root, f, front(red));
    let clip = extract(
        &model,
        BodyId::Root,
        &[Entity::Instance(inst), Entity::Face(f)],
    )
    .unwrap();
    let mut undo = UndoStack::new();
    for k in 1..=2 {
        insert(
            &mut model,
            BodyId::Root,
            &clip,
            translate(0.0, 3.0 * k as f64, 0.0),
            &mut undo,
            "Paste",
        )
        .unwrap();
    }
    assert_eq!(model.materials().count(), 1);
    assert_eq!(model.definitions().count(), 1);
    assert_eq!(model.instances_of(d).len(), 3);
    assert_eq!(undo.undo_count(), 2);
}

#[test]
fn mirrored_copy_keeps_normals_and_painted_sides() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    let red = material(&mut model, "red", RED);
    let blue = material(&mut model, "blue", BLUE);
    // The +x side: outside red, inside blue.
    set(
        &mut model,
        BodyId::Root,
        faces[4],
        FaceMaterials {
            front: Some(red),
            back: Some(blue),
            ..FaceMaterials::default()
        },
    );
    let m = translate(-2.0, 0.0, 0.0) * DMat4::from_scale(v(-1.0, 1.0, 1.0));
    let mut undo = UndoStack::new();
    copied(&mut model, BodyId::Root, &faces_of(&faces), m, &mut undo);
    // The copy spans x -3..-2; the +x side lands at x = -3, still red outside.
    let f = face_at(&model, BodyId::Root, v(-3.0, 0.5, 0.5));
    assert!(normal(&model, BodyId::Root, f).x < 0.0);
    let got = model.root().face_materials(f);
    assert_eq!((got.front, got.back), (Some(red), Some(blue)));
    assert!(normals_out(
        &model,
        BodyId::Root,
        &[v(0.5, 0.5, 0.5), v(-2.5, 0.5, 0.5)]
    ));
}

#[test]
fn textured_sides_keep_their_texture_points() {
    let mut model = Model::new();
    let mut brick = Material::new("brick", [0.8, 0.4, 0.2, 1.0]);
    brick.texture = Some(Texture::new("brick.png", DVec2::new(2.0, 0.5)));
    let mut edit = ModelEdit::new();
    let brick = model.add_material(brick, &mut edit);
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    for &f in &faces {
        set(&mut model, BodyId::Root, f, front(brick));
    }
    let odd = TextureMap {
        u: DVec4::new(0.5, -0.25, 0.125, 3.0),
        v: DVec4::new(0.0, 2.0, -1.0, -0.75),
    };
    set(
        &mut model,
        BodyId::Root,
        faces[1],
        FaceMaterials {
            front: Some(brick),
            front_map: Some(odd),
            ..FaceMaterials::default()
        },
    );
    let before = model.clone();
    let m = DMat4::from_scale_rotation_translation(
        v(1.5, 0.75, 2.0),
        DQuat::from_axis_angle(v(1.0, 2.0, 0.5).normalize(), 0.7),
        v(6.0, -2.0, 1.5),
    );
    let mut undo = UndoStack::new();
    copied(&mut model, BodyId::Root, &faces_of(&faces), m, &mut undo);
    for &f in &faces {
        let moved = face_at(
            &model,
            BodyId::Root,
            m.transform_point3(centroid(&before, BodyId::Root, f)),
        );
        let old = side_map(&before, BodyId::Root, f, Side::Front).unwrap();
        let new = side_map(&model, BodyId::Root, moved, Side::Front).unwrap();
        let ctx = before.root().geometry();
        for p in ctx.loop_points(&ctx.face(f).unwrap().outer) {
            let want = old.apply(p);
            let got = new.apply(m.transform_point3(p));
            assert!(
                (got - want).length() <= 1e-7 * (1.0 + want.length()),
                "{got:?} != {want:?}"
            );
        }
        // The original keeps its own placement.
        assert_eq!(
            model.root().face_materials(f),
            before.root().face_materials(f)
        );
    }
}

#[test]
fn paste_into_an_open_definition() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    place(&mut model, BodyId::Root, d, translate(5.0, 0.0, 0.0));
    let e = edge_in(&mut model, BodyId::Root, v(0.0, 0.0, 3.0), v(1.0, 0.0, 3.0));
    let clip = extract(&model, BodyId::Root, &[Entity::Edge(e)]).unwrap();
    let mut undo = UndoStack::new();
    let out = insert(
        &mut model,
        BodyId::Definition(d),
        &clip,
        translate(0.0, 0.0, -1.0),
        &mut undo,
        "Paste",
    )
    .unwrap();
    assert_eq!(model.validate(), vec![]);
    assert_eq!(kinds(&out), (1, 0, 0));
    assert_eq!(counts(&model, BodyId::Definition(d)), (10, 13, 6));
    assert_eq!(counts(&model, BodyId::Root), (2, 1, 0));
}

#[test]
fn paste_inside_its_own_definition_is_a_cycle() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let inner = definition(&mut model, "holder", false);
    place(&mut model, BodyId::Definition(inner), d, DMat4::IDENTITY);
    let inst = place(&mut model, BodyId::Root, inner, DMat4::IDENTITY);
    let clip = extract(&model, BodyId::Root, &[Entity::Instance(inst)]).unwrap();
    let before = model.clone();
    let mut undo = UndoStack::new();
    for body in [BodyId::Definition(inner), BodyId::Definition(d)] {
        let got = insert(&mut model, body, &clip, DMat4::IDENTITY, &mut undo, "Paste");
        assert_eq!(
            got,
            Err(CopyError::Model(ModelError::Cycle)),
            "into {body:?}"
        );
        assert_eq!(model, before);
        assert_eq!(undo.undo_count(), 0);
    }
}

#[test]
fn insert_refuses_bad_matrix_and_missing_body() {
    let mut model = Model::new();
    let f = rect_in(&mut model, BodyId::Root, DVec2::ZERO, DVec2::ONE, 0.0);
    let clip = extract(&model, BodyId::Root, &[Entity::Face(f)]).unwrap();
    let before = model.clone();
    let mut undo = UndoStack::new();
    let nowhere = BodyId::Definition(DefinitionId(77));
    assert_eq!(
        insert(
            &mut model,
            nowhere,
            &clip,
            DMat4::IDENTITY,
            &mut undo,
            "Paste"
        ),
        Err(CopyError::Model(ModelError::MissingBody(nowhere)))
    );
    assert_eq!(
        insert(
            &mut model,
            BodyId::Root,
            &clip,
            DMat4::ZERO,
            &mut undo,
            "Paste"
        ),
        Err(CopyError::BadMatrix)
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}
