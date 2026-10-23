// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Explode an instance into its parent body (`docs/specs/m4-explode.md`).

use std::collections::BTreeMap;

use glam::{DMat4, DQuat, DVec3};
use modeler_kernel::explode::{ExplodeError, explode};
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;
use modeler_kernel::topology::*;

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

/// Loose edges between consecutive points (no faces) in `body`.
fn polyline_in(model: &mut Model, body: BodyId, pts: &[DVec3]) {
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = pts.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            for w in vs.windows(2) {
                ctx.insert_edge(w[0], w[1], log).unwrap();
            }
        })
        .unwrap();
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

const BOTTOM: &[usize] = &[0, 3, 2, 1];
const TOP: &[usize] = &[4, 5, 6, 7];
const SIDES: [&[usize]; 4] = [&[0, 1, 5, 4], &[2, 3, 7, 6], &[1, 2, 6, 5], &[3, 0, 4, 7]];

fn cube_faces() -> Vec<&'static [usize]> {
    let mut f = vec![BOTTOM, TOP];
    f.extend(SIDES);
    f
}

/// A unit cube (min corner at the origin, normals out) in a new definition.
fn cube_definition(model: &mut Model, name: &str, group: bool) -> (DefinitionId, Vec<FaceId>) {
    let mut edit = ModelEdit::new();
    let d = model.add_definition(
        DefinitionInfo {
            name: name.into(),
            group,
        },
        &mut edit,
    );
    let faces = mesh_in(
        model,
        BodyId::Definition(d),
        &cube_points(DVec3::ZERO),
        &cube_faces(),
    );
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

fn counts(model: &Model, body: BodyId) -> (usize, usize, usize) {
    let c = model.body(body).unwrap().geometry();
    (c.vertex_count(), c.edge_count(), c.face_count())
}

fn key(p: DVec3) -> [i64; 3] {
    [
        (p.x * 1e6).round() as i64,
        (p.y * 1e6).round() as i64,
        (p.z * 1e6).round() as i64,
    ]
}

fn points_of(model: &Model, body: BodyId) -> Vec<[i64; 3]> {
    let mut out: Vec<[i64; 3]> = model
        .body(body)
        .unwrap()
        .geometry()
        .vertices()
        .map(|(_, x)| key(x.position))
        .collect();
    out.sort();
    out
}

fn expected_points(pts: &[DVec3], m: DMat4) -> Vec<[i64; 3]> {
    let mut out: Vec<[i64; 3]> = pts.iter().map(|&p| key(m.transform_point3(p))).collect();
    out.sort();
    out.dedup();
    out
}

/// Every face's normal points away from the centroid of the body's vertices.
fn normals_outward(model: &Model, body: BodyId) -> bool {
    let c = model.body(body).unwrap().geometry();
    let pts: Vec<DVec3> = c.vertices().map(|(_, x)| x.position).collect();
    let centre = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
    c.faces().all(|(f, face)| {
        let plane = c.face_plane(f).unwrap();
        let pts = c.loop_points(&face.outer);
        let mid = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
        plane.normal.dot(mid - centre) > 0.0
    })
}

/// The face of the body whose outer loop's centroid is `c`.
fn face_with_centroid(model: &Model, body: BodyId, c: DVec3) -> FaceId {
    let ctx = model.body(body).unwrap().geometry();
    ctx.faces()
        .find(|(_, face)| {
            let pts = ctx.loop_points(&face.outer);
            let mid = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
            mid.distance(c) < 1e-6
        })
        .map(|(id, _)| id)
        .expect("face")
}

fn material(model: &mut Model, name: &str) -> MaterialId {
    let mut edit = ModelEdit::new();
    model.add_material(Material::new(name, [0.5, 0.5, 0.5, 1.0]), &mut edit)
}

fn exploded(
    model: &mut Model,
    body: BodyId,
    inst: InstanceId,
    undo: &mut UndoStack,
) -> Vec<Entity> {
    let out = explode(model, body, inst, undo, "Explode").expect("explode");
    assert_eq!(model.validate(), vec![], "valid after explode");
    out
}

#[test]
fn translated_cube_lands_in_the_root() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let m = DMat4::from_translation(v(5.0, 0.0, 0.0));
    let inst = place(&mut model, BodyId::Root, d, m);
    let mut undo = UndoStack::new();
    let out = exploded(&mut model, BodyId::Root, inst, &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (8, 12, 6));
    assert_eq!(
        points_of(&model, BodyId::Root),
        expected_points(&cube_points(DVec3::ZERO), m)
    );
    assert!(normals_outward(&model, BodyId::Root));
    assert_eq!(model.root().instances().count(), 0);
    let edges = out.iter().filter(|e| matches!(e, Entity::Edge(_))).count();
    let faces = out.iter().filter(|e| matches!(e, Entity::Face(_))).count();
    assert_eq!((edges, faces, out.len()), (12, 6, 18));
    let mut sorted = out.clone();
    sorted.sort();
    assert_eq!(out, sorted, "result sorted");
    assert_eq!(undo.undo_label(), Some("Explode"));
}

#[test]
fn component_definition_stays_after_explode() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let inside = model.body(BodyId::Definition(d)).unwrap().clone();
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    assert!(model.definition(d).is_some());
    assert_eq!(model.body(BodyId::Definition(d)), Some(&inside));
}

#[test]
fn other_instances_are_untouched() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let a = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let b = place(
        &mut model,
        BodyId::Root,
        d,
        DMat4::from_translation(v(3.0, 0.0, 0.0)),
    );
    let b_data = model.root().instance(b).unwrap().clone();
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, a, &mut undo);
    assert_eq!(model.root().instance(a), None);
    assert_eq!(model.root().instance(b), Some(&b_data));
    assert_eq!(model.instances_of(d), vec![(BodyId::Root, b)]);
    assert_eq!(counts(&model, BodyId::Definition(d)), (8, 12, 6));
}

#[test]
fn group_definition_goes_away() {
    let mut model = Model::new();
    let (d, faces) = cube_definition(&mut model, "box", true);
    let red = material(&mut model, "red");
    let mut edit = ModelEdit::new();
    model
        .set_face_materials(
            BodyId::Definition(d),
            faces[1],
            FaceMaterials {
                front: Some(red),
                back: None,
            },
            &mut edit,
        )
        .unwrap();
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    assert_eq!(model.validate(), vec![]);
    let before = model.clone();
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    assert_eq!(model.definition(d), None, "the emptied group is removed");
    assert_eq!(model.definitions().count(), 0);
    assert_eq!(counts(&model, BodyId::Root), (8, 12, 6));
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
}

#[test]
fn undo_restores_exactly_and_redo_reapplies() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    mesh_in(
        &mut model,
        BodyId::Root,
        &cube_points(v(-3.0, 0.0, 0.0)),
        &cube_faces(),
    );
    let inst = place(
        &mut model,
        BodyId::Root,
        d,
        DMat4::from_translation(v(1.0, 0.0, 0.0)),
    );
    let before = model.clone();
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    let after = model.clone();
    assert_eq!(undo.undo(&mut model).unwrap().as_deref(), Some("Explode"));
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(model, after);
}

#[test]
fn rotation_and_scale_place_the_points() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let m = DMat4::from_scale_rotation_translation(
        DVec3::splat(2.0),
        DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2),
        v(10.0, 4.0, 1.0),
    );
    let inst = place(&mut model, BodyId::Root, d, m);
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    assert_eq!(
        points_of(&model, BodyId::Root),
        expected_points(&cube_points(DVec3::ZERO), m)
    );
    assert!(normals_outward(&model, BodyId::Root));
}

#[test]
fn mirrored_instance_keeps_normals_out() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let m = DMat4::from_translation(v(4.0, 0.0, 0.0)) * DMat4::from_scale(v(-1.0, 1.0, 1.0));
    let inst = place(&mut model, BodyId::Root, d, m);
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    assert_eq!(
        points_of(&model, BodyId::Root),
        expected_points(&cube_points(DVec3::ZERO), m)
    );
    assert!(normals_outward(&model, BodyId::Root));
}

#[test]
fn mirrored_instance_keeps_painted_sides() {
    let mut model = Model::new();
    let (d, faces) = cube_definition(&mut model, "box", false);
    let red = material(&mut model, "red");
    let blue = material(&mut model, "blue");
    let mut edit = ModelEdit::new();
    // The +x side (outside front red, inside back blue).
    model
        .set_face_materials(
            BodyId::Definition(d),
            faces[4],
            FaceMaterials {
                front: Some(red),
                back: Some(blue),
            },
            &mut edit,
        )
        .unwrap();
    let m = DMat4::from_scale(v(-1.0, 1.0, 1.0));
    let inst = place(&mut model, BodyId::Root, d, m);
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    // The +x side lands at x = -1, still red outside.
    let f = face_with_centroid(&model, BodyId::Root, v(-1.0, 0.5, 0.5));
    assert!(model.root().geometry().face_plane(f).unwrap().normal.x < 0.0);
    assert_eq!(
        model.root().face_materials(f),
        FaceMaterials {
            front: Some(red),
            back: Some(blue),
        }
    );
}

#[test]
fn face_materials_come_along_and_the_instance_material_fills_defaults() {
    let mut model = Model::new();
    let (d, faces) = cube_definition(&mut model, "box", false);
    let red = material(&mut model, "red");
    let blue = material(&mut model, "blue");
    let mut edit = ModelEdit::new();
    model
        .set_face_materials(
            BodyId::Definition(d),
            faces[1],
            FaceMaterials {
                front: Some(red),
                back: None,
            },
            &mut edit,
        )
        .unwrap();
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let mut data = model.root().instance(inst).unwrap().clone();
    data.material = Some(blue);
    model
        .set_instance(BodyId::Root, inst, data, &mut edit)
        .unwrap();
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    let top = face_with_centroid(&model, BodyId::Root, v(0.5, 0.5, 1.0));
    assert_eq!(
        model.root().face_materials(top),
        FaceMaterials {
            front: Some(red),
            back: Some(blue),
        }
    );
    let bottom = face_with_centroid(&model, BodyId::Root, v(0.5, 0.5, 0.0));
    assert_eq!(
        model.root().face_materials(bottom),
        FaceMaterials {
            front: Some(blue),
            back: Some(blue),
        }
    );
}

#[test]
fn unpainted_instance_leaves_defaults_alone() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    let ctx = model.root().geometry();
    for (f, _) in ctx.faces() {
        assert_eq!(model.root().face_materials(f), FaceMaterials::default());
    }
}

#[test]
fn nested_instances_move_up() {
    let mut model = Model::new();
    let (inner, _) = cube_definition(&mut model, "inner", false);
    let mut edit = ModelEdit::new();
    let outer = model.add_definition(
        DefinitionInfo {
            name: "outer".into(),
            group: false,
        },
        &mut edit,
    );
    let red = material(&mut model, "red");
    let blue = material(&mut model, "blue");
    let t1 = DMat4::from_translation(v(0.0, 2.0, 0.0));
    let t2 = DMat4::from_translation(v(0.0, 0.0, 3.0));
    let n1 = model
        .add_instance(
            BodyId::Definition(outer),
            Instance {
                definition: inner,
                transform: t1,
                name: "plain".into(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    model
        .add_instance(
            BodyId::Definition(outer),
            Instance {
                definition: inner,
                transform: t2,
                name: "painted".into(),
                material: Some(red),
            },
            &mut edit,
        )
        .unwrap();
    let _ = n1;
    let m = DMat4::from_translation(v(5.0, 0.0, 0.0));
    let inst = place(&mut model, BodyId::Root, outer, m);
    let mut data = model.root().instance(inst).unwrap().clone();
    data.material = Some(blue);
    model
        .set_instance(BodyId::Root, inst, data, &mut edit)
        .unwrap();
    let mut undo = UndoStack::new();
    let out = exploded(&mut model, BodyId::Root, inst, &mut undo);
    let mut placed: Vec<(String, [i64; 3], Option<MaterialId>, DefinitionId)> = model
        .root()
        .instances()
        .map(|(_, i)| {
            (
                i.name.clone(),
                key(i.transform.transform_point3(DVec3::ZERO)),
                i.material,
                i.definition,
            )
        })
        .collect();
    placed.sort();
    assert_eq!(
        placed,
        vec![
            ("painted".into(), key(v(5.0, 0.0, 3.0)), Some(red), inner),
            ("plain".into(), key(v(5.0, 2.0, 0.0)), Some(blue), inner),
        ]
    );
    let new_instances = out
        .iter()
        .filter(|e| matches!(e, Entity::Instance(_)))
        .count();
    assert_eq!(new_instances, 2);
    assert_eq!(out.len(), 2, "no geometry in the outer definition");
    assert_eq!(counts(&model, BodyId::Root), (0, 0, 0));
}

#[test]
fn explode_inside_a_definition_body() {
    let mut model = Model::new();
    let (inner, _) = cube_definition(&mut model, "inner", false);
    let mut edit = ModelEdit::new();
    let outer = model.add_definition(
        DefinitionInfo {
            name: "outer".into(),
            group: false,
        },
        &mut edit,
    );
    let m = DMat4::from_translation(v(1.0, 1.0, 1.0));
    let nested = place(&mut model, BodyId::Definition(outer), inner, m);
    place(&mut model, BodyId::Root, outer, DMat4::IDENTITY);
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Definition(outer), nested, &mut undo);
    assert_eq!(counts(&model, BodyId::Definition(outer)), (8, 12, 6));
    assert_eq!(
        points_of(&model, BodyId::Definition(outer)),
        expected_points(&cube_points(DVec3::ZERO), m)
    );
    assert_eq!(model.root().instances().count(), 1);
}

#[test]
fn touching_cubes_weld_and_share_a_face() {
    let mut model = Model::new();
    mesh_in(
        &mut model,
        BodyId::Root,
        &cube_points(DVec3::ZERO),
        &cube_faces(),
    );
    let (d, _) = cube_definition(&mut model, "box", false);
    let inst = place(
        &mut model,
        BodyId::Root,
        d,
        DMat4::from_translation(v(1.0, 0.0, 0.0)),
    );
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    // 12 vertices, 20 edges; the two faces at x = 1 become one.
    assert_eq!(counts(&model, BodyId::Root), (12, 20, 11));
}

#[test]
fn edges_cross_and_split() {
    let mut model = Model::new();
    polyline_in(
        &mut model,
        BodyId::Root,
        &[v(1.0, -1.0, 0.0), v(1.0, 1.0, 0.0)],
    );
    let mut edit = ModelEdit::new();
    let d = model.add_definition(
        DefinitionInfo {
            name: "stick".into(),
            group: false,
        },
        &mut edit,
    );
    polyline_in(
        &mut model,
        BodyId::Definition(d),
        &[v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0)],
    );
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let mut undo = UndoStack::new();
    let out = exploded(&mut model, BodyId::Root, inst, &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (5, 4, 0));
    assert_eq!(out.len(), 2, "both halves of the exploded edge");
    assert!(out.iter().all(|e| matches!(e, Entity::Edge(_))));
}

#[test]
fn end_on_an_edge_splits_it() {
    let mut model = Model::new();
    polyline_in(
        &mut model,
        BodyId::Root,
        &[v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0)],
    );
    let mut edit = ModelEdit::new();
    let d = model.add_definition(
        DefinitionInfo {
            name: "stick".into(),
            group: false,
        },
        &mut edit,
    );
    polyline_in(
        &mut model,
        BodyId::Definition(d),
        &[v(0.0, 0.0, 0.0), v(0.0, 1.0, 0.0)],
    );
    let inst = place(
        &mut model,
        BodyId::Root,
        d,
        DMat4::from_translation(v(1.0, 0.0, 0.0)),
    );
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (4, 3, 0));
}

#[test]
fn edges_closing_a_loop_with_the_parent_make_no_face() {
    // Three sides of a square in the root, the fourth inside the instance.
    let mut model = Model::new();
    polyline_in(
        &mut model,
        BodyId::Root,
        &[
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
        ],
    );
    let mut edit = ModelEdit::new();
    let d = model.add_definition(
        DefinitionInfo {
            name: "side".into(),
            group: false,
        },
        &mut edit,
    );
    polyline_in(
        &mut model,
        BodyId::Definition(d),
        &[v(0.0, 1.0, 0.0), v(0.0, 0.0, 0.0)],
    );
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (4, 4, 0));
}

#[test]
fn open_box_stays_open_even_when_touching() {
    let mut model = Model::new();
    // A loose edge in the root that the box's corner lands on.
    polyline_in(
        &mut model,
        BodyId::Root,
        &[v(-1.0, 0.0, 0.0), v(0.0, 0.0, 0.0)],
    );
    let mut edit = ModelEdit::new();
    let d = model.add_definition(
        DefinitionInfo {
            name: "tray".into(),
            group: false,
        },
        &mut edit,
    );
    let mut faces = vec![BOTTOM];
    faces.extend(SIDES);
    mesh_in(
        &mut model,
        BodyId::Definition(d),
        &cube_points(DVec3::ZERO),
        &faces,
    );
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    assert_eq!(counts(&model, BodyId::Root), (9, 13, 5));
}

#[test]
fn box_dropped_on_a_painted_face_keeps_both_materials() {
    // A painted 4 x 4 ground face; a cube stands on it. The ground gets a hole-free split
    // along the cube's footprint; every region keeps a face; the footprint region takes the
    // cube bottom's material where it has one.
    let mut model = Model::new();
    let green = material(&mut model, "green");
    let red = material(&mut model, "red");
    let ground = mesh_in(
        &mut model,
        BodyId::Root,
        &[
            v(-2.0, -2.0, 0.0),
            v(2.0, -2.0, 0.0),
            v(2.0, 2.0, 0.0),
            v(-2.0, 2.0, 0.0),
        ],
        &[&[0, 1, 2, 3]],
    );
    let mut edit = ModelEdit::new();
    model
        .set_face_materials(
            BodyId::Root,
            ground[0],
            FaceMaterials {
                front: Some(green),
                back: None,
            },
            &mut edit,
        )
        .unwrap();
    let (d, faces) = cube_definition(&mut model, "box", false);
    model
        .set_face_materials(
            BodyId::Definition(d),
            faces[1],
            FaceMaterials {
                front: Some(red),
                back: None,
            },
            &mut edit,
        )
        .unwrap();
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let mut undo = UndoStack::new();
    exploded(&mut model, BodyId::Root, inst, &mut undo);
    let ctx = model.root().geometry();
    // Ground with a square hole, the footprint face, five more cube faces.
    assert_eq!(ctx.face_count(), 7);
    let top = face_with_centroid(&model, BodyId::Root, v(0.5, 0.5, 1.0));
    assert_eq!(model.root().face_materials(top).front, Some(red));
    let holed = ctx
        .faces()
        .find(|(_, f)| f.inner.len() == 1)
        .map(|(id, _)| id)
        .expect("ground face with a hole");
    assert_eq!(model.root().face_materials(holed).front, Some(green));
}

#[test]
fn errors_change_nothing() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let inst = place(&mut model, BodyId::Root, d, DMat4::IDENTITY);
    let before = model.clone();
    let mut undo = UndoStack::new();
    assert_eq!(
        explode(
            &mut model,
            BodyId::Root,
            InstanceId(9999),
            &mut undo,
            "Explode"
        ),
        Err(ExplodeError::MissingInstance(InstanceId(9999)))
    );
    let missing = BodyId::Definition(DefinitionId(9999));
    assert_eq!(
        explode(&mut model, missing, inst, &mut undo, "Explode"),
        Err(ExplodeError::Model(ModelError::MissingBody(missing)))
    );
    assert_eq!(
        explode(
            &mut model,
            BodyId::Definition(d),
            inst,
            &mut undo,
            "Explode"
        ),
        Err(ExplodeError::MissingInstance(inst)),
        "the instance is in the root, not inside its own definition"
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn bad_transform_is_refused() {
    let mut model = Model::new();
    let (d, _) = cube_definition(&mut model, "box", false);
    let inst = place(
        &mut model,
        BodyId::Root,
        d,
        DMat4::from_scale(v(1.0, 1.0, 0.0)),
    );
    let before = model.clone();
    let mut undo = UndoStack::new();
    assert_eq!(
        explode(&mut model, BodyId::Root, inst, &mut undo, "Explode"),
        Err(ExplodeError::BadTransform)
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}
