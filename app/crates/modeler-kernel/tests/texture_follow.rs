// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Texture maps move with their faces: transform, sticky move, make component and explode
//! (`docs/specs/m4-texture-follow.md`).

use std::collections::BTreeMap;

use glam::{DAffine2, DMat4, DQuat, DVec2, DVec3, DVec4};
use modeler_kernel::component::make_component;
use modeler_kernel::explode::explode;
use modeler_kernel::geom::polygon_normal;
use modeler_kernel::model::*;
use modeler_kernel::movemerge::transform_sticky;
use modeler_kernel::paint::Side;
use modeler_kernel::select::Entity;
use modeler_kernel::texture::*;
use modeler_kernel::topology::*;
use modeler_kernel::transform::transform;
use proptest::prelude::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn d4(x: f64, y: f64, z: f64, w: f64) -> DVec4 {
    DVec4::new(x, y, z, w)
}

fn close4(a: DVec4, b: DVec4, tol: f64) -> bool {
    (a - b).abs().max_element() <= tol
}

fn close_map(a: Option<TextureMap>, b: Option<TextureMap>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => close4(a.u, b.u, 1e-9) && close4(a.v, b.v, 1e-9),
        _ => false,
    }
}

/// Faces from point indices into `body` with the raw primitives (shared edges made once).
fn mesh_in(model: &mut Model, body: BodyId, points: &[DVec3], faces: &[&[usize]]) -> Vec<FaceId> {
    let mut edit = ModelEdit::new();
    let ids = model
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
        .unwrap();
    assert!(model.body(body).unwrap().geometry().validate().is_empty());
    ids
}

fn cube_points(at: DVec3) -> Vec<DVec3> {
    [
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
    .map(|&p| p + at)
    .collect()
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

fn textured(model: &mut Model, name: &str) -> MaterialId {
    let mut material = Material::new(name, [0.8, 0.4, 0.2, 1.0]);
    material.texture = Some(Texture::new("brick.png", DVec2::new(2.0, 0.5)));
    let mut edit = ModelEdit::new();
    model.add_material(material, &mut edit)
}

fn plain(model: &mut Model, name: &str) -> MaterialId {
    let mut edit = ModelEdit::new();
    model.add_material(Material::new(name, [0.1, 0.6, 0.3, 1.0]), &mut edit)
}

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

/// A map with every entry set, unlike any default placement.
fn odd_map(k: f64) -> TextureMap {
    TextureMap {
        u: d4(0.5 * k, -0.25, 0.125, 3.0),
        v: d4(0.0, 2.0, -1.0 + k, -0.75),
    }
}

fn some_matrix() -> DMat4 {
    DMat4::from_scale_rotation_translation(
        v(1.5, 0.75, 2.0),
        DQuat::from_axis_angle(v(1.0, 2.0, 0.5).normalize(), 0.7),
        v(3.0, -2.0, 1.5),
    )
}

/// A face side whose texture placement is meant to follow: it has a stored map, or its
/// material has a texture.
fn follows(model: &Model, body: BodyId, face: FaceId, side: Side) -> bool {
    let materials = model.body(body).unwrap().face_materials(face);
    let (material, map) = match side {
        Side::Front => (materials.front, materials.front_map),
        Side::Back => (materials.back, materials.back_map),
    };
    map.is_some()
        || material
            .and_then(|id| model.material(id))
            .is_some_and(|m| m.texture.is_some())
}

fn loop_points_of(model: &Model, body: BodyId, face: FaceId) -> Vec<DVec3> {
    let ctx = model.body(body).unwrap().geometry();
    let face = ctx.face(face).unwrap();
    std::iter::once(&face.outer)
        .chain(face.inner.iter())
        .flat_map(|l| ctx.loop_points(l))
        .collect()
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

/// Every face side of `before`'s body that follows has a face in `after`'s body at
/// `m · centroid` whose same side gives each moved corner `m · p` the texture point `p` had.
fn assert_follow(before: &Model, from: BodyId, after: &Model, to: BodyId, m: DMat4) {
    let mut checked = 0;
    for (face, _) in before.body(from).unwrap().geometry().faces() {
        for side in [Side::Front, Side::Back] {
            if !follows(before, from, face, side) {
                continue;
            }
            let moved = face_at(after, to, m.transform_point3(centroid(before, from, face)));
            let old = side_map(before, from, face, side).unwrap();
            let new = side_map(after, to, moved, side).unwrap();
            for p in loop_points_of(before, from, face) {
                let want = old.apply(p);
                let got = new.apply(m.transform_point3(p));
                assert!(
                    (got - want).length() <= 1e-7 * (1.0 + want.length()),
                    "face {face:?} {side:?} at {p:?}: {got:?} != {want:?}"
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "nothing was textured");
}

fn all_maps_none(model: &Model, body: BodyId) -> bool {
    model
        .body(body)
        .unwrap()
        .face_material_entries()
        .all(|(_, m)| m.front_map.is_none() && m.back_map.is_none())
}

/// A root cube at `at`: every front textured, the top with a stored odd map, the bottom's
/// back plain with an odd map of its own.
fn textured_cube(model: &mut Model, body: BodyId, at: DVec3) -> Vec<FaceId> {
    let faces = cube_in(model, body, at);
    let brick = textured(model, "brick");
    let paint = plain(model, "paint");
    for &f in &faces {
        set(model, body, f, front(brick));
    }
    set(
        model,
        body,
        faces[1],
        FaceMaterials {
            front: Some(brick),
            front_map: Some(odd_map(1.0)),
            ..FaceMaterials::default()
        },
    );
    set(
        model,
        body,
        faces[0],
        FaceMaterials {
            front: Some(brick),
            back: Some(paint),
            back_map: Some(odd_map(-2.0)),
            ..FaceMaterials::default()
        },
    );
    faces
}

// -- moved_maps --------------------------------------------------------------------------

#[test]
fn stored_maps_are_carried_by_the_matrix() {
    let model = Model::new();
    let m = some_matrix();
    let materials = FaceMaterials {
        front_map: Some(odd_map(1.0)),
        back_map: Some(odd_map(3.0)),
        ..FaceMaterials::default()
    };
    let out = moved_maps(&model, materials, DVec3::Z, m);
    assert_eq!((out.front, out.back), (None, None));
    assert!(close_map(out.front_map, odd_map(1.0).transformed(m)));
    assert!(close_map(out.back_map, odd_map(3.0).transformed(m)));
}

#[test]
fn textured_side_without_a_map_gets_its_default_carried() {
    let mut model = Model::new();
    let brick = textured(&mut model, "brick");
    let paint = plain(&mut model, "paint");
    let m = DMat4::from_translation(v(1.0, 2.0, 3.0));
    let materials = FaceMaterials {
        front: Some(brick),
        back: Some(paint),
        ..FaceMaterials::default()
    };
    let out = moved_maps(&model, materials, v(0.0, 0.0, 4.0), m);
    assert_eq!((out.front, out.back), (Some(brick), Some(paint)));
    assert!(close_map(
        out.front_map,
        TextureMap::default_for(DVec3::Z).transformed(m)
    ));
    assert_eq!(out.back_map, None, "a plain side stores no map");
}

#[test]
fn textured_back_uses_the_opposite_normal() {
    let mut model = Model::new();
    let brick = textured(&mut model, "brick");
    let m = DMat4::from_quat(DQuat::from_rotation_z(0.3));
    let materials = FaceMaterials {
        back: Some(brick),
        ..FaceMaterials::default()
    };
    let out = moved_maps(&model, materials, DVec3::X, m);
    assert_eq!(out.front_map, None);
    assert!(close_map(
        out.back_map,
        TextureMap::default_for(-DVec3::X).transformed(m)
    ));
}

#[test]
fn untextured_and_unknown_materials_store_no_map() {
    let mut model = Model::new();
    let paint = plain(&mut model, "paint");
    let m = some_matrix();
    for materials in [
        FaceMaterials::default(),
        front(paint),
        FaceMaterials {
            front: Some(MaterialId(999)),
            back: Some(paint),
            ..FaceMaterials::default()
        },
    ] {
        assert_eq!(moved_maps(&model, materials, DVec3::Y, m), materials);
    }
}

#[test]
fn a_singular_matrix_changes_nothing() {
    let mut model = Model::new();
    let brick = textured(&mut model, "brick");
    let materials = FaceMaterials {
        front: Some(brick),
        back_map: Some(odd_map(1.0)),
        ..FaceMaterials::default()
    };
    for m in [DMat4::from_scale(v(1.0, 0.0, 1.0)), {
        let mut bad = DMat4::IDENTITY;
        bad.w_axis.y = f64::NAN;
        bad
    }] {
        assert_eq!(moved_maps(&model, materials, DVec3::Z, m), materials);
    }
}

#[test]
fn a_mirror_carries_the_maps_too() {
    let mut model = Model::new();
    let brick = textured(&mut model, "brick");
    let m = DMat4::from_scale(v(-1.0, 1.0, 1.0));
    let materials = FaceMaterials {
        front: Some(brick),
        back_map: Some(odd_map(0.5)),
        ..FaceMaterials::default()
    };
    let out = moved_maps(&model, materials, DVec3::X, m);
    assert!(close_map(
        out.front_map,
        TextureMap::default_for(DVec3::X).transformed(m)
    ));
    assert!(close_map(out.back_map, odd_map(0.5).transformed(m)));
}

// -- transform ---------------------------------------------------------------------------

fn all_faces(faces: &[FaceId]) -> Vec<Entity> {
    faces.iter().map(|&f| Entity::Face(f)).collect()
}

#[test]
fn moving_a_textured_cube_keeps_its_texture_points() {
    let mut model = Model::new();
    let faces = textured_cube(&mut model, BodyId::Root, v(0.0, 0.0, 0.0));
    let before = model.clone();
    let m = some_matrix();
    let mut undo = UndoStack::new();
    transform(
        &mut model,
        BodyId::Root,
        &all_faces(&faces),
        m,
        &mut undo,
        "Move",
    )
    .unwrap();
    assert_follow(&before, BodyId::Root, &model, BodyId::Root, m);
    assert_eq!(undo.undo_count(), 1, "one undo step");
}

#[test]
fn mirroring_a_textured_cube_keeps_its_texture_points() {
    let mut model = Model::new();
    let faces = textured_cube(&mut model, BodyId::Root, v(0.0, 0.0, 0.0));
    let before = model.clone();
    let m = DMat4::from_translation(v(4.0, 0.0, 0.0)) * DMat4::from_scale(v(-1.0, 1.0, 2.0));
    let mut undo = UndoStack::new();
    transform(
        &mut model,
        BodyId::Root,
        &all_faces(&faces),
        m,
        &mut undo,
        "Flip",
    )
    .unwrap();
    assert_follow(&before, BodyId::Root, &model, BodyId::Root, m);
}

#[test]
fn selecting_only_the_edges_moves_the_maps_too() {
    let mut model = Model::new();
    textured_cube(&mut model, BodyId::Root, v(0.0, 0.0, 0.0));
    let before = model.clone();
    let edges: Vec<Entity> = model
        .root()
        .geometry()
        .edges()
        .map(|(id, _)| Entity::Edge(id))
        .collect();
    let m = DMat4::from_translation(v(0.0, 0.0, 5.0));
    let mut undo = UndoStack::new();
    transform(&mut model, BodyId::Root, &edges, m, &mut undo, "Move").unwrap();
    assert_follow(&before, BodyId::Root, &model, BodyId::Root, m);
}

#[test]
fn untextured_geometry_stores_no_maps() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, DVec3::ZERO);
    let paint = plain(&mut model, "paint");
    for &f in &faces {
        set(&mut model, BodyId::Root, f, front(paint));
    }
    let mut undo = UndoStack::new();
    transform(
        &mut model,
        BodyId::Root,
        &all_faces(&faces),
        some_matrix(),
        &mut undo,
        "Move",
    )
    .unwrap();
    assert!(all_maps_none(&model, BodyId::Root));
    let mut undo = UndoStack::new();
    transform_sticky(
        &mut model,
        BodyId::Root,
        &all_faces(&faces),
        DMat4::from_translation(v(0.0, 3.0, 0.0)),
        &mut undo,
        "Move",
    )
    .unwrap();
    assert!(all_maps_none(&model, BodyId::Root));
}

#[test]
fn a_face_moved_only_in_part_keeps_its_map() {
    let mut model = Model::new();
    let f = mesh_in(
        &mut model,
        BodyId::Root,
        &[
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
        ],
        &[&[0, 1, 2, 3]],
    )[0];
    let brick = textured(&mut model, "brick");
    let materials = FaceMaterials {
        front: Some(brick),
        front_map: Some(odd_map(1.0)),
        ..FaceMaterials::default()
    };
    set(&mut model, BodyId::Root, f, materials);
    let right = model
        .root()
        .geometry()
        .edges()
        .find(|(_, e)| {
            let ctx = model.root().geometry();
            e.ends
                .iter()
                .all(|&x| ctx.vertex(x).unwrap().position.x == 1.0)
        })
        .map(|(id, _)| id)
        .unwrap();
    let mut undo = UndoStack::new();
    transform(
        &mut model,
        BodyId::Root,
        &[Entity::Edge(right)],
        DMat4::from_translation(v(1.0, 0.0, 0.0)),
        &mut undo,
        "Move",
    )
    .unwrap();
    assert_eq!(model.root().face_materials(f), materials);
}

#[test]
fn undo_and_redo_restore_the_maps() {
    let mut model = Model::new();
    let faces = textured_cube(&mut model, BodyId::Root, v(0.0, 0.0, 0.0));
    let before = model.clone();
    let mut undo = UndoStack::new();
    transform_sticky(
        &mut model,
        BodyId::Root,
        &all_faces(&faces),
        some_matrix(),
        &mut undo,
        "Move",
    )
    .unwrap();
    let after = model.clone();
    undo.undo(&mut model).unwrap();
    let entries =
        |m: &Model| -> Vec<(FaceId, FaceMaterials)> { m.root().face_material_entries().collect() };
    assert_eq!(entries(&model), entries(&before));
    undo.redo(&mut model).unwrap();
    assert_eq!(entries(&model), entries(&after));
}

#[test]
fn sticky_move_of_a_free_cube_keeps_its_texture_points() {
    let mut model = Model::new();
    let faces = textured_cube(&mut model, BodyId::Root, v(0.0, 0.0, 0.0));
    cube_in(&mut model, BodyId::Root, v(10.0, 0.0, 0.0));
    let before = model.clone();
    let m = some_matrix();
    let mut undo = UndoStack::new();
    transform_sticky(
        &mut model,
        BodyId::Root,
        &all_faces(&faces),
        m,
        &mut undo,
        "Move",
    )
    .unwrap();
    // only the moved cube is textured, so only its faces are checked
    assert_follow(&before, BodyId::Root, &model, BodyId::Root, m);
}

#[test]
fn sticky_move_into_contact_keeps_the_texture_points() {
    let mut model = Model::new();
    let f = mesh_in(
        &mut model,
        BodyId::Root,
        &[
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
            v(3.0, 0.0, 0.0),
            v(4.0, 0.0, 0.0),
            v(4.0, 1.0, 0.0),
            v(3.0, 1.0, 0.0),
        ],
        &[&[0, 1, 2, 3], &[4, 5, 6, 7]],
    );
    let brick = textured(&mut model, "brick");
    set(&mut model, BodyId::Root, f[1], front(brick));
    let before = model.clone();
    let m = DMat4::from_translation(v(-2.0, 0.0, 0.0));
    let mut undo = UndoStack::new();
    transform_sticky(
        &mut model,
        BodyId::Root,
        &[Entity::Face(f[1])],
        m,
        &mut undo,
        "Move",
    )
    .unwrap();
    assert!(model.validate().is_empty());
    // the squares now share the edge at x = 1
    assert_eq!(model.root().geometry().edge_count(), 7);
    assert_follow(&before, BodyId::Root, &model, BodyId::Root, m);
    let other = face_at(&model, BodyId::Root, v(0.5, 0.5, 0.0));
    assert_eq!(model.root().face_materials(other), FaceMaterials::default());
}

#[test]
fn rectangle_pulled_loose_from_a_cube_keeps_its_texture_points() {
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
    let f = mesh_in(
        &mut model,
        BodyId::Root,
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
    let brick = textured(&mut model, "brick");
    set(
        &mut model,
        BodyId::Root,
        f[6],
        FaceMaterials {
            front: Some(brick),
            back: Some(brick),
            back_map: Some(odd_map(2.0)),
            ..FaceMaterials::default()
        },
    );
    let before = model.clone();
    let m = DMat4::from_translation(v(2., 2., 0.))
        * DMat4::from_quat(DQuat::from_rotation_z(0.5))
        * DMat4::from_translation(v(-2., -2., 0.));
    let mut undo = UndoStack::new();
    transform_sticky(
        &mut model,
        BodyId::Root,
        &[Entity::Face(f[6])],
        m,
        &mut undo,
        "Rotate",
    )
    .unwrap();
    assert!(model.validate().is_empty());
    assert_follow(&before, BodyId::Root, &model, BodyId::Root, m);
}

#[test]
fn moving_an_instance_leaves_its_definition_alone() {
    let mut model = Model::new();
    let mut edit = ModelEdit::new();
    let d = model.add_definition(
        DefinitionInfo {
            name: "box".into(),
            group: false,
        },
        &mut edit,
    );
    let inside = BodyId::Definition(d);
    textured_cube(&mut model, inside, DVec3::ZERO);
    let inst = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    let entries = |m: &Model| -> Vec<(FaceId, FaceMaterials)> {
        m.body(inside).unwrap().face_material_entries().collect()
    };
    let before = entries(&model);
    let mut undo = UndoStack::new();
    transform_sticky(
        &mut model,
        BodyId::Root,
        &[Entity::Instance(inst)],
        some_matrix(),
        &mut undo,
        "Move",
    )
    .unwrap();
    assert_eq!(entries(&model), before);
}

// -- make component ----------------------------------------------------------------------

fn info(name: &str) -> DefinitionInfo {
    DefinitionInfo {
        name: name.into(),
        group: false,
    }
}

#[test]
fn make_component_keeps_the_texture_points_in_place() {
    let mut model = Model::new();
    let at = v(2.0, 3.0, 1.0);
    let faces = textured_cube(&mut model, BodyId::Root, at);
    let before = model.clone();
    let mut undo = UndoStack::new();
    let inst = make_component(
        &mut model,
        BodyId::Root,
        &all_faces(&faces),
        info("box"),
        &mut undo,
        "Make Component",
    )
    .unwrap();
    let placed = model.root().instance(inst).unwrap().clone();
    assert_eq!(placed.transform, DMat4::from_translation(at));
    // definition coordinates are the old ones less the origin
    assert_follow(
        &before,
        BodyId::Root,
        &model,
        BodyId::Definition(placed.definition),
        DMat4::from_translation(-at),
    );
}

#[test]
fn make_component_of_untextured_faces_stores_no_maps() {
    let mut model = Model::new();
    let faces = cube_in(&mut model, BodyId::Root, v(2.0, 3.0, 1.0));
    let paint = plain(&mut model, "paint");
    for &f in &faces {
        set(&mut model, BodyId::Root, f, front(paint));
    }
    let mut undo = UndoStack::new();
    let inst = make_component(
        &mut model,
        BodyId::Root,
        &all_faces(&faces),
        info("box"),
        &mut undo,
        "Make Component",
    )
    .unwrap();
    let d = model.root().instance(inst).unwrap().definition;
    assert!(all_maps_none(&model, BodyId::Definition(d)));
}

// -- explode -----------------------------------------------------------------------------

fn instance_of_textured_cube(model: &mut Model, m: DMat4) -> (DefinitionId, InstanceId) {
    let mut edit = ModelEdit::new();
    let d = model.add_definition(info("box"), &mut edit);
    textured_cube(model, BodyId::Definition(d), DVec3::ZERO);
    let inst = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: m,
                name: String::new(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    (d, inst)
}

#[test]
fn explode_keeps_the_texture_points_of_a_turned_scaled_instance() {
    let mut model = Model::new();
    let m = some_matrix();
    let (d, inst) = instance_of_textured_cube(&mut model, m);
    let before = model.clone();
    let mut undo = UndoStack::new();
    explode(&mut model, BodyId::Root, inst, &mut undo, "Explode").unwrap();
    assert!(model.validate().is_empty());
    assert_follow(&before, BodyId::Definition(d), &model, BodyId::Root, m);
}

#[test]
fn explode_keeps_the_texture_points_of_a_mirrored_instance() {
    let mut model = Model::new();
    let m = DMat4::from_translation(v(-3.0, 1.0, 0.0)) * DMat4::from_scale(v(1.0, -2.0, 1.0));
    let (d, inst) = instance_of_textured_cube(&mut model, m);
    let before = model.clone();
    let mut undo = UndoStack::new();
    explode(&mut model, BodyId::Root, inst, &mut undo, "Explode").unwrap();
    assert!(model.validate().is_empty());
    assert_follow(&before, BodyId::Definition(d), &model, BodyId::Root, m);
}

#[test]
fn an_instance_texture_lands_with_the_placement_it_showed() {
    let mut model = Model::new();
    let mut edit = ModelEdit::new();
    let d = model.add_definition(info("box"), &mut edit);
    let inside = BodyId::Definition(d);
    let faces = cube_in(&mut model, inside, DVec3::ZERO);
    let brick = textured(&mut model, "brick");
    let m = DMat4::from_scale_rotation_translation(
        v(2.0, 2.0, 2.0),
        DQuat::from_rotation_z(0.4),
        v(5.0, 0.0, 0.0),
    );
    let inst = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: m,
                name: String::new(),
                material: Some(brick),
            },
            &mut edit,
        )
        .unwrap();
    let before = model.clone();
    let mut undo = UndoStack::new();
    explode(&mut model, BodyId::Root, inst, &mut undo, "Explode").unwrap();
    for &f in &faces {
        let moved = face_at(
            &model,
            BodyId::Root,
            m.transform_point3(centroid(&before, inside, f)),
        );
        let got = model.root().face_materials(moved);
        assert_eq!((got.front, got.back), (Some(brick), Some(brick)));
        for side in [Side::Front, Side::Back] {
            // what the instance showed: the definition face's default placement
            let old = side_map(&before, inside, f, side).unwrap();
            let new = side_map(&model, BodyId::Root, moved, side).unwrap();
            for p in loop_points_of(&before, inside, f) {
                let (want, got) = (old.apply(p), new.apply(m.transform_point3(p)));
                assert!(
                    (got - want).length() < 1e-9,
                    "{side:?}: {got:?} != {want:?}"
                );
            }
        }
    }
}

#[test]
fn explode_with_a_plain_instance_material_stores_no_maps() {
    let mut model = Model::new();
    let mut edit = ModelEdit::new();
    let d = model.add_definition(info("box"), &mut edit);
    cube_in(&mut model, BodyId::Definition(d), DVec3::ZERO);
    let paint = plain(&mut model, "paint");
    let inst = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: some_matrix(),
                name: String::new(),
                material: Some(paint),
            },
            &mut edit,
        )
        .unwrap();
    let mut undo = UndoStack::new();
    explode(&mut model, BodyId::Root, inst, &mut undo, "Explode").unwrap();
    assert!(all_maps_none(&model, BodyId::Root));
    assert!(
        model
            .root()
            .face_material_entries()
            .all(|(_, m)| m.front == Some(paint))
    );
}

// -- properties --------------------------------------------------------------------------

fn matrix() -> impl Strategy<Value = DMat4> {
    (
        (-1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0),
        -3.0f64..3.0,
        (0.3f64..3.0, 0.3f64..3.0, 0.3f64..3.0),
        (-5.0f64..5.0, -5.0f64..5.0, -5.0f64..5.0),
        any::<bool>(),
    )
        .prop_filter_map("axis", |(axis, angle, s, t, mirror)| {
            let axis = v(axis.0, axis.1, axis.2);
            (axis.length() > 1e-2).then(|| {
                let scale = v(if mirror { -s.0 } else { s.0 }, s.1, s.2);
                DMat4::from_scale_rotation_translation(
                    scale,
                    DQuat::from_axis_angle(axis.normalize(), angle),
                    v(t.0, t.1, t.2),
                )
            })
        })
}

/// One side per cube face: 0 = untextured, 1 = textured default, 2 = textured with a
/// turned and shifted image.
fn paint_plan() -> impl Strategy<Value = Vec<(u8, f64)>> {
    proptest::collection::vec((0u8..3, -2.0f64..2.0), 6)
}

fn plan_cube(model: &mut Model, body: BodyId, plan: &[(u8, f64)]) {
    let faces = cube_in(model, body, DVec3::ZERO);
    let brick = textured(model, "brick");
    let paint = plain(model, "paint");
    for (&f, &(kind, k)) in faces.iter().zip(plan) {
        let normal = polygon_normal(
            &model
                .body(body)
                .unwrap()
                .geometry()
                .loop_points(&model.body(body).unwrap().geometry().face(f).unwrap().outer),
        );
        let materials = match kind {
            0 => FaceMaterials {
                front: Some(paint),
                ..FaceMaterials::default()
            },
            1 => front(brick),
            _ => FaceMaterials {
                front: Some(brick),
                front_map: Some(TextureMap::default_for(normal).then(
                    DAffine2::from_scale_angle_translation(
                        DVec2::new(1.0 + k.abs(), 0.5),
                        k,
                        DVec2::new(k, -k),
                    ),
                )),
                ..FaceMaterials::default()
            },
        };
        set(model, body, f, materials);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn sticky_transform_keeps_texture_points(plan in paint_plan(), m in matrix()) {
        prop_assume!(plan.iter().any(|(kind, _)| *kind > 0));
        let mut model = Model::new();
        plan_cube(&mut model, BodyId::Root, &plan);
        let before = model.clone();
        let faces: Vec<Entity> = model.root().geometry().faces().map(|(f, _)| Entity::Face(f)).collect();
        let mut undo = UndoStack::new();
        transform_sticky(&mut model, BodyId::Root, &faces, m, &mut undo, "Move").unwrap();
        prop_assert!(model.validate().is_empty());
        assert_follow(&before, BodyId::Root, &model, BodyId::Root, m);
        // sides that never had a texture store nothing
        for (f, materials) in model.root().face_material_entries() {
            if !follows(&model, BodyId::Root, f, Side::Front) {
                prop_assert_eq!(materials.front_map, None);
            }
            prop_assert_eq!(materials.back_map, None);
        }
    }

    #[test]
    fn explode_keeps_texture_points(plan in paint_plan(), m in matrix()) {
        prop_assume!(plan.iter().any(|(kind, _)| *kind > 0));
        let mut model = Model::new();
        let mut edit = ModelEdit::new();
        let d = model.add_definition(info("box"), &mut edit);
        plan_cube(&mut model, BodyId::Definition(d), &plan);
        let inst = model
            .add_instance(
                BodyId::Root,
                Instance { definition: d, transform: m, name: String::new(), material: None },
                &mut edit,
            )
            .unwrap();
        let before = model.clone();
        let mut undo = UndoStack::new();
        explode(&mut model, BodyId::Root, inst, &mut undo, "Explode").unwrap();
        prop_assert!(model.validate().is_empty());
        assert_follow(&before, BodyId::Definition(d), &model, BodyId::Root, m);
    }

    #[test]
    fn make_component_then_explode_gives_back_the_texture_points(
        plan in paint_plan(),
        at in (-5.0f64..5.0, -5.0f64..5.0, -5.0f64..5.0),
    ) {
        prop_assume!(plan.iter().any(|(kind, _)| *kind > 0));
        let mut model = Model::new();
        plan_cube(&mut model, BodyId::Root, &plan);
        let shift = DMat4::from_translation(v(at.0, at.1, at.2));
        let faces: Vec<Entity> = model.root().geometry().faces().map(|(f, _)| Entity::Face(f)).collect();
        let mut undo = UndoStack::new();
        transform(&mut model, BodyId::Root, &faces, shift, &mut undo, "Move").unwrap();
        let before = model.clone();
        let faces: Vec<Entity> = model.root().geometry().faces().map(|(f, _)| Entity::Face(f)).collect();
        let inst = make_component(&mut model, BodyId::Root, &faces, info("box"), &mut undo, "Make Component").unwrap();
        explode(&mut model, BodyId::Root, inst, &mut undo, "Explode").unwrap();
        prop_assert!(model.validate().is_empty());
        assert_follow(&before, BodyId::Root, &model, BodyId::Root, DMat4::IDENTITY);
    }
}
