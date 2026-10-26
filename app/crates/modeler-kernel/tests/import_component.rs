// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-components-import: a model becomes a component definition.
//! See docs/specs/m6-components-import.md.

use glam::{DMat4, DVec3};
use modeler_kernel::copy::{CopyError, import_component};
use modeler_kernel::model::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A model whose root holds a closed square (four edges, one face) of side `side`.
fn square_model(side: f64) -> Model {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(side, 0.0, 0.0),
        v(side, side, 0.0),
        v(0.0, side, 0.0),
    ];
    model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
    model
}

fn define(model: &mut Model, name: &str, group: bool) -> DefinitionId {
    let mut log = ModelEdit::new();
    model.add_definition(
        DefinitionInfo {
            name: name.to_string(),
            group,
        },
        &mut log,
    )
}

fn counts(model: &Model, body: BodyId) -> (usize, usize) {
    let geometry = model.body(body).unwrap().geometry();
    (geometry.edge_count(), geometry.face_count())
}

#[test]
fn the_roots_geometry_becomes_a_new_component() {
    let source = square_model(2.0);
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let id = import_component(&mut model, &source, "Table", &mut undo, "Import").unwrap();
    let info = model.definition(id).unwrap();
    assert_eq!(info.name, "Table");
    assert!(!info.group);
    assert_eq!(counts(&model, BodyId::Definition(id)), (4, 1));
    let points: Vec<DVec3> = model
        .body(BodyId::Definition(id))
        .unwrap()
        .geometry()
        .vertices()
        .map(|(_, vertex)| vertex.position)
        .collect();
    assert!(points.contains(&v(2.0, 2.0, 0.0)), "coordinates kept");
    assert!(model.validate().is_empty());
}

#[test]
fn nothing_is_placed() {
    let source = square_model(1.0);
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    import_component(&mut model, &source, "Table", &mut undo, "Import").unwrap();
    assert_eq!(counts(&model, BodyId::Root), (0, 0));
    assert_eq!(model.root().instances().count(), 0);
}

#[test]
fn the_name_is_trimmed_and_made_unique() {
    let source = square_model(1.0);
    let mut model = Model::new();
    define(&mut model, "Table", false);
    define(&mut model, "Component", true);
    let mut undo = UndoStack::new();
    let a = import_component(&mut model, &source, "  Table ", &mut undo, "Import").unwrap();
    let b = import_component(&mut model, &source, "", &mut undo, "Import").unwrap();
    let c = import_component(&mut model, &source, "Table", &mut undo, "Import").unwrap();
    assert_eq!(model.definition(a).unwrap().name, "Table 2");
    assert_eq!(model.definition(b).unwrap().name, "Component 2");
    assert_eq!(model.definition(c).unwrap().name, "Table 3");
}

#[test]
fn an_empty_model_is_refused_and_nothing_changes() {
    let mut model = Model::new();
    define(&mut model, "Keep", false);
    let before = model.clone();
    let mut undo = UndoStack::new();
    assert_eq!(
        import_component(&mut model, &Model::new(), "X", &mut undo, "Import"),
        Err(CopyError::Empty)
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn nested_definitions_and_materials_come_along() {
    let mut source = Model::new();
    let mut log = ModelEdit::new();
    let leg = define(&mut source, "Leg", false);
    source
        .edit_geometry(BodyId::Definition(leg), &mut log, |ctx, edit| {
            ctx.add_edge(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), edit)
                .unwrap();
        })
        .unwrap();
    for x in [0.0, 3.0] {
        source
            .add_instance(
                BodyId::Root,
                Instance {
                    definition: leg,
                    transform: DMat4::from_translation(v(x, 0.0, 0.0)),
                    name: String::new(),
                    material: None,
                },
                &mut log,
            )
            .unwrap();
    }
    let mut model = Model::new();
    define(&mut model, "Leg", false);
    let mut undo = UndoStack::new();
    let id = import_component(&mut model, &source, "Stool", &mut undo, "Import").unwrap();
    assert_eq!(
        model
            .body(BodyId::Definition(id))
            .unwrap()
            .instances()
            .count(),
        2
    );
    // The source's Leg was copied in as its own definition next to the existing "Leg".
    assert_eq!(model.definitions().count(), 3);
    assert!(model.validate().is_empty());
    assert_eq!(source.definitions().count(), 1, "source untouched");
}

#[test]
fn face_materials_are_kept() {
    let mut source = square_model(1.0);
    let mut log = ModelEdit::new();
    let red = source.add_material(Material::new("Red", [1.0, 0.0, 0.0, 1.0]), &mut log);
    let face = source.root().geometry().faces().next().unwrap().0;
    source
        .set_face_materials(
            BodyId::Root,
            face,
            FaceMaterials {
                front: Some(red),
                ..FaceMaterials::default()
            },
            &mut log,
        )
        .unwrap();
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let id = import_component(&mut model, &source, "Panel", &mut undo, "Import").unwrap();
    let body = model.body(BodyId::Definition(id)).unwrap();
    let (_, materials) = body.face_material_entries().next().unwrap();
    let front = materials.front.expect("front material kept");
    assert_eq!(model.material(front).unwrap().name, "Red");
}

#[test]
fn it_is_one_undo_step_and_redo_restores_it() {
    let source = square_model(1.0);
    let mut model = Model::new();
    let before = model.clone();
    let mut undo = UndoStack::new();
    let id = import_component(&mut model, &source, "Table", &mut undo, "Import Component").unwrap();
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Import Component"));
    let after = model.clone();
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    assert!(model.definition(id).is_none());
    undo.redo(&mut model).unwrap();
    assert_eq!(model, after);
}
