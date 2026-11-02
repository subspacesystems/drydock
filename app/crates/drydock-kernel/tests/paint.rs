// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Paint faces and instances; create, edit and purge materials (`docs/specs/m4-paint.md`).

use drydock_kernel::erase::erase;
use drydock_kernel::model::*;
use drydock_kernel::paint::*;
use drydock_kernel::select::Entity;
use drydock_kernel::topology::{EdgeId, FaceId};
use glam::DMat4;

mod transform_common;
use transform_common::*;

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const GLASS: [f32; 4] = [0.6, 0.8, 1.0, 0.3];

fn add(model: &mut Model, name: &str, color: [f32; 4]) -> MaterialId {
    let mut log = ModelEdit::new();
    model.add_material(Material::new(name, color), &mut log)
}

/// A model with a unit cube in the root and two materials.
fn setup() -> (Model, Vec<FaceId>, MaterialId, MaterialId) {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let red = add(&mut model, "Red", RED);
    let blue = add(&mut model, "Blue", BLUE);
    (model, faces, red, blue)
}

/// A definition holding a cube, placed once in the root.
fn with_instance(model: &mut Model) -> InstanceId {
    let mut log = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "Box".into(),
            group: false,
        },
        &mut log,
    );
    model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: def,
                transform: DMat4::from_translation(v(5.0, 0.0, 0.0)),
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap()
}

fn fm(model: &Model, face: FaceId) -> FaceMaterials {
    model.root().face_materials(face)
}

fn any_edge(model: &Model) -> EdgeId {
    model.root().geometry().edges().next().unwrap().0
}

fn names(model: &Model) -> Vec<String> {
    model.materials().map(|(_, m)| m.name.clone()).collect()
}

// -- paint -------------------------------------------------------------------------------

#[test]
fn paints_the_front_of_one_face() {
    let (mut model, faces, red, _) = setup();
    let mut undo = UndoStack::new();
    let n = paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(faces[1])],
        Side::Front,
        Some(red),
        &mut undo,
        "Paint",
    )
    .unwrap();
    assert_eq!(n, 1);
    assert_eq!(
        fm(&model, faces[1]),
        FaceMaterials {
            front: Some(red),
            back: None,
            ..Default::default()
        }
    );
    assert_eq!(fm(&model, faces[0]), FaceMaterials::default());
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Paint"));
}

#[test]
fn painting_the_back_keeps_the_front() {
    let (mut model, faces, red, blue) = setup();
    let mut undo = UndoStack::new();
    let f = Entity::Face(faces[2]);
    paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Back,
        Some(blue),
        &mut undo,
        "P",
    )
    .unwrap();
    assert_eq!(
        fm(&model, faces[2]),
        FaceMaterials {
            front: Some(red),
            back: Some(blue),
            ..Default::default()
        }
    );
    assert_eq!(undo.undo_count(), 2);
}

#[test]
fn painting_none_clears_one_side() {
    let (mut model, faces, red, blue) = setup();
    let mut undo = UndoStack::new();
    let f = Entity::Face(faces[0]);
    paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Back,
        Some(blue),
        &mut undo,
        "P",
    )
    .unwrap();
    let n = paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Front,
        None,
        &mut undo,
        "P",
    )
    .unwrap();
    assert_eq!(n, 1);
    assert_eq!(
        fm(&model, faces[0]),
        FaceMaterials {
            front: None,
            back: Some(blue),
            ..Default::default()
        }
    );
}

#[test]
fn paints_many_faces_in_one_undo_step() {
    let (mut model, faces, red, _) = setup();
    let before = model.clone();
    let mut undo = UndoStack::new();
    let targets: Vec<Entity> = faces.iter().map(|&f| Entity::Face(f)).collect();
    let n = paint(
        &mut model,
        BodyId::Root,
        &targets,
        Side::Front,
        Some(red),
        &mut undo,
        "Paint",
    )
    .unwrap();
    assert_eq!(n, 6);
    assert!(faces.iter().all(|&f| fm(&model, f).front == Some(red)));
    assert_eq!(undo.undo_count(), 1);
    let after = model.clone();
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(model, after);
}

#[test]
fn paints_an_instance_whatever_the_side() {
    let (mut model, _, red, _) = setup();
    let inst = with_instance(&mut model);
    let mut undo = UndoStack::new();
    let n = paint(
        &mut model,
        BodyId::Root,
        &[Entity::Instance(inst)],
        Side::Back,
        Some(red),
        &mut undo,
        "Paint",
    )
    .unwrap();
    assert_eq!(n, 1);
    let i = model.root().instance(inst).unwrap();
    assert_eq!(i.material, Some(red));
    assert_eq!(i.transform, DMat4::from_translation(v(5.0, 0.0, 0.0)));
    assert_eq!(undo.undo_count(), 1);
}

#[test]
fn edges_are_skipped_and_only_changes_count() {
    let (mut model, faces, red, _) = setup();
    let mut undo = UndoStack::new();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(faces[0])],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    let edge = any_edge(&model);
    let n = paint(
        &mut model,
        BodyId::Root,
        &[
            Entity::Edge(edge),
            Entity::Face(faces[0]),
            Entity::Face(faces[1]),
        ],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    assert_eq!(n, 1, "face 0 was already red");
    assert_eq!(undo.undo_count(), 2);
}

#[test]
fn nothing_to_change_pushes_no_step() {
    let (mut model, faces, red, _) = setup();
    let mut undo = UndoStack::new();
    let f = Entity::Face(faces[3]);
    paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    let before = model.clone();
    let n = paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    assert_eq!(n, 0);
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(model, before);
    let edge = any_edge(&model);
    let n = paint(
        &mut model,
        BodyId::Root,
        &[Entity::Edge(edge)],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    assert_eq!(n, 0);
    assert_eq!(undo.undo_count(), 1);
}

#[test]
fn duplicate_targets_count_once() {
    let (mut model, faces, red, _) = setup();
    let mut undo = UndoStack::new();
    let f = Entity::Face(faces[4]);
    let n = paint(
        &mut model,
        BodyId::Root,
        &[f, f, f],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn paint_refusals_change_nothing() {
    let (mut model, faces, red, _) = setup();
    let before = model.clone();
    let mut undo = UndoStack::new();
    let f = Entity::Face(faces[0]);

    assert_eq!(
        paint(
            &mut model,
            BodyId::Root,
            &[],
            Side::Front,
            Some(red),
            &mut undo,
            "P"
        ),
        Err(PaintError::Empty)
    );
    let ghost = BodyId::Definition(DefinitionId(99));
    assert_eq!(
        paint(
            &mut model,
            ghost,
            &[f],
            Side::Front,
            Some(red),
            &mut undo,
            "P"
        ),
        Err(PaintError::Model(ModelError::MissingBody(ghost)))
    );
    assert_eq!(
        paint(
            &mut model,
            BodyId::Root,
            &[f],
            Side::Front,
            Some(MaterialId(77)),
            &mut undo,
            "P"
        ),
        Err(PaintError::Model(ModelError::MissingMaterial(MaterialId(
            77
        ))))
    );
    let missing = Entity::Face(FaceId(9999));
    let missing_edge = Entity::Edge(EdgeId(9999));
    assert_eq!(
        paint(
            &mut model,
            BodyId::Root,
            &[f, missing, missing_edge],
            Side::Front,
            Some(red),
            &mut undo,
            "P"
        ),
        Err(PaintError::Missing(missing))
    );
    assert_eq!(
        paint(
            &mut model,
            BodyId::Root,
            &[Entity::Instance(InstanceId(5))],
            Side::Front,
            Some(red),
            &mut undo,
            "P"
        ),
        Err(PaintError::Missing(Entity::Instance(InstanceId(5))))
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn a_deleted_face_is_missing() {
    let (mut model, faces, red, _) = setup();
    let mut undo = UndoStack::new();
    erase(
        &mut model,
        BodyId::Root,
        &[Entity::Face(faces[1])],
        &mut undo,
        "Erase",
    )
    .unwrap();
    assert_eq!(
        paint(
            &mut model,
            BodyId::Root,
            &[Entity::Face(faces[1])],
            Side::Front,
            Some(red),
            &mut undo,
            "P"
        ),
        Err(PaintError::Missing(Entity::Face(faces[1])))
    );
}

// -- sample ------------------------------------------------------------------------------

#[test]
fn sample_reads_face_sides_and_instances() {
    let (mut model, faces, red, blue) = setup();
    let inst = with_instance(&mut model);
    let mut undo = UndoStack::new();
    let f = Entity::Face(faces[5]);
    paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Back,
        Some(blue),
        &mut undo,
        "P",
    )
    .unwrap();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Instance(inst)],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();

    assert_eq!(sample(&model, BodyId::Root, f, Side::Front), Some(None));
    assert_eq!(
        sample(&model, BodyId::Root, f, Side::Back),
        Some(Some(blue))
    );
    assert_eq!(
        sample(&model, BodyId::Root, Entity::Instance(inst), Side::Back),
        Some(Some(red))
    );
    assert_eq!(
        sample(
            &model,
            BodyId::Root,
            Entity::Edge(any_edge(&model)),
            Side::Front
        ),
        None
    );
    assert_eq!(
        sample(
            &model,
            BodyId::Root,
            Entity::Face(FaceId(9999)),
            Side::Front
        ),
        None
    );
    assert_eq!(
        sample(&model, BodyId::Definition(DefinitionId(99)), f, Side::Front),
        None
    );
}

// -- names -------------------------------------------------------------------------------

#[test]
fn unique_names() {
    let mut model = Model::new();
    assert_eq!(unique_material_name(&model, "Brick"), "Brick");
    assert_eq!(unique_material_name(&model, ""), "Material");
    assert_eq!(unique_material_name(&model, "   "), "Material");
    assert_eq!(unique_material_name(&model, "  Wood "), "Wood");
    add(&mut model, "Brick", RED);
    assert_eq!(unique_material_name(&model, "Brick"), "Brick 2");
    assert_eq!(unique_material_name(&model, " Brick"), "Brick 2");
    add(&mut model, "Brick 3", RED);
    assert_eq!(unique_material_name(&model, "Brick"), "Brick 2");
    add(&mut model, "Brick 2", RED);
    assert_eq!(unique_material_name(&model, "Brick"), "Brick 4");
    assert_eq!(unique_material_name(&model, "brick"), "brick");
    add(&mut model, "Material", RED);
    assert_eq!(unique_material_name(&model, ""), "Material 2");
}

// -- create ------------------------------------------------------------------------------

#[test]
fn create_material_uses_a_unique_name_and_one_step() {
    let mut model = Model::new();
    add(&mut model, "Glass", GLASS);
    let mut undo = UndoStack::new();
    let mut material = Material::new("Glass", GLASS);
    material.texture = Some("textures/glass.png".into());
    let id = create_material(&mut model, material, &mut undo, "New Material").unwrap();
    let stored = model.material(id).unwrap();
    assert_eq!(stored.name, "Glass 2");
    assert_eq!(stored.color, GLASS);
    assert_eq!(
        stored.texture.as_ref().map(|t| t.path.as_str()),
        Some("textures/glass.png")
    );
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("New Material"));
    undo.undo(&mut model).unwrap();
    assert!(model.material(id).is_none());
    assert_eq!(names(&model), vec!["Glass".to_string()]);
}

#[test]
fn create_material_refuses_bad_colors() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    for color in [
        [1.5, 0.0, 0.0, 1.0],
        [0.0, -0.1, 0.0, 1.0],
        [0.0, 0.0, f32::NAN, 1.0],
        [0.0, 0.0, 0.0, 1.01],
        [0.0, 0.0, 0.0, f32::INFINITY],
    ] {
        assert_eq!(
            create_material(&mut model, Material::new("X", color), &mut undo, "New"),
            Err(PaintError::BadColor),
            "{color:?}"
        );
    }
    assert_eq!(model.materials().count(), 0);
    assert_eq!(undo.undo_count(), 0);
    let edge_values = create_material(
        &mut model,
        Material::new("Edges", [0.0, 1.0, 0.0, 0.0]),
        &mut undo,
        "New",
    );
    assert!(edge_values.is_ok());
}

// -- edit --------------------------------------------------------------------------------

#[test]
fn edit_material_changes_it_in_one_step() {
    let (mut model, _, red, _) = setup();
    let before = model.clone();
    let mut undo = UndoStack::new();
    edit_material(
        &mut model,
        red,
        Material::new("Dark red", [0.5, 0.0, 0.0, 1.0]),
        &mut undo,
        "Edit Material",
    )
    .unwrap();
    let stored = model.material(red).unwrap();
    assert_eq!(stored.name, "Dark red");
    assert_eq!(stored.color, [0.5, 0.0, 0.0, 1.0]);
    assert_eq!(undo.undo_count(), 1);
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
}

#[test]
fn edit_material_keeps_names_unique() {
    let (mut model, _, red, blue) = setup();
    let mut undo = UndoStack::new();
    edit_material(&mut model, red, Material::new("Blue", RED), &mut undo, "E").unwrap();
    assert_eq!(model.material(red).unwrap().name, "Blue 2");
    assert_eq!(model.material(blue).unwrap().name, "Blue");
    // Its own name is never taken.
    edit_material(
        &mut model,
        blue,
        Material::new(" Blue ", GLASS),
        &mut undo,
        "E",
    )
    .unwrap();
    assert_eq!(model.material(blue).unwrap().name, "Blue");
    assert_eq!(model.material(blue).unwrap().color, GLASS);
    edit_material(&mut model, blue, Material::new("", GLASS), &mut undo, "E").unwrap();
    assert_eq!(model.material(blue).unwrap().name, "Material");
}

#[test]
fn edit_material_without_change_pushes_no_step() {
    let (mut model, _, red, _) = setup();
    let mut undo = UndoStack::new();
    edit_material(&mut model, red, Material::new("Red", RED), &mut undo, "E").unwrap();
    edit_material(&mut model, red, Material::new("Red  ", RED), &mut undo, "E").unwrap();
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn edit_material_refusals() {
    let (mut model, _, red, _) = setup();
    let before = model.clone();
    let mut undo = UndoStack::new();
    assert_eq!(
        edit_material(
            &mut model,
            MaterialId(42),
            Material::new("X", RED),
            &mut undo,
            "E"
        ),
        Err(PaintError::Model(ModelError::MissingMaterial(MaterialId(
            42
        ))))
    );
    assert_eq!(
        edit_material(
            &mut model,
            red,
            Material::new("X", [2.0, 0.0, 0.0, 1.0]),
            &mut undo,
            "E"
        ),
        Err(PaintError::BadColor)
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

// -- purge -------------------------------------------------------------------------------

#[test]
fn purge_removes_only_unused_materials() {
    let (mut model, faces, red, blue) = setup();
    let glass = add(&mut model, "Glass", GLASS);
    let spare = add(&mut model, "Spare", RED);
    let inst = with_instance(&mut model);
    let mut undo = UndoStack::new();
    let f = Entity::Face(faces[0]);
    paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    paint(
        &mut model,
        BodyId::Root,
        &[f],
        Side::Back,
        Some(blue),
        &mut undo,
        "P",
    )
    .unwrap();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Instance(inst)],
        Side::Front,
        Some(glass),
        &mut undo,
        "P",
    )
    .unwrap();
    let before = model.clone();
    let removed = purge_materials(&mut model, &mut undo, "Purge").unwrap();
    assert_eq!(removed, vec![spare]);
    assert!(model.material(spare).is_none());
    assert!(model.material(red).is_some());
    assert!(model.material(blue).is_some());
    assert!(model.material(glass).is_some());
    assert_eq!(undo.undo_count(), 4);
    assert_eq!(undo.undo_label(), Some("Purge"));
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
}

#[test]
fn purge_with_nothing_unused_pushes_no_step() {
    let (mut model, faces, red, blue) = setup();
    let mut undo = UndoStack::new();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(faces[0]), Entity::Face(faces[1])],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(faces[2])],
        Side::Back,
        Some(blue),
        &mut undo,
        "P",
    )
    .unwrap();
    let before = model.clone();
    assert_eq!(purge_materials(&mut model, &mut undo, "Purge"), Ok(vec![]));
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 2);

    let mut empty = Model::new();
    assert_eq!(purge_materials(&mut empty, &mut undo, "Purge"), Ok(vec![]));
}

#[test]
fn purge_drops_paint_of_deleted_faces_and_undo_brings_it_back() {
    let (mut model, faces, red, blue) = setup();
    let mut undo = UndoStack::new();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(faces[1])],
        Side::Front,
        Some(red),
        &mut undo,
        "P",
    )
    .unwrap();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(faces[1])],
        Side::Back,
        Some(blue),
        &mut undo,
        "P",
    )
    .unwrap();
    // Blue stays in use on another face; red is only on the face about to go.
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(faces[2])],
        Side::Front,
        Some(blue),
        &mut undo,
        "P",
    )
    .unwrap();
    let painted = model.clone();
    erase(
        &mut model,
        BodyId::Root,
        &[Entity::Face(faces[1])],
        &mut undo,
        "Erase",
    )
    .unwrap();

    let removed = purge_materials(&mut model, &mut undo, "Purge").unwrap();
    assert_eq!(removed, vec![red]);
    assert!(model.material(red).is_none());
    assert!(model.material(blue).is_some());
    // The deleted face's entry keeps the side that names a material still present.
    assert_eq!(
        fm(&model, faces[1]),
        FaceMaterials {
            front: None,
            back: Some(blue),
            ..Default::default()
        }
    );
    assert!(model.validate().is_empty());

    undo.undo(&mut model).unwrap(); // purge
    undo.undo(&mut model).unwrap(); // erase
    assert_eq!(model, painted);
    assert_eq!(
        fm(&model, faces[1]),
        FaceMaterials {
            front: Some(red),
            back: Some(blue),
            ..Default::default()
        }
    );
}

#[test]
fn purge_sees_materials_used_inside_definitions() {
    let mut model = Model::new();
    let red = add(&mut model, "Red", RED);
    let blue = add(&mut model, "Blue", BLUE);
    let mut log = ModelEdit::new();
    let outer = model.add_definition(
        DefinitionInfo {
            name: "Outer".into(),
            group: false,
        },
        &mut log,
    );
    let inner = model.add_definition(
        DefinitionInfo {
            name: "Inner".into(),
            group: false,
        },
        &mut log,
    );
    model
        .add_instance(
            BodyId::Definition(outer),
            Instance {
                definition: inner,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: Some(blue),
            },
            &mut log,
        )
        .unwrap();
    let mut undo = UndoStack::new();
    let removed = purge_materials(&mut model, &mut undo, "Purge").unwrap();
    assert_eq!(removed, vec![red]);
    assert!(model.material(blue).is_some());
}
