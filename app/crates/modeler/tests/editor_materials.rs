// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Create, edit and purge materials in the editor (`docs/specs/m4-app-materials.md`).

use modeler::editor::*;
use modeler_kernel::model::*;
use modeler_kernel::paint::{Side, paint};
use modeler_kernel::select::Entity;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::units::Unit;

fn mat(name: &str, color: [f32; 4]) -> Material {
    Material::new(name, color)
}

fn undo_label(editor: &Editor) -> Option<String> {
    editor.undo_stack().undo_label().map(str::to_owned)
}

/// A model with one drawn square in the root; returns its face entity.
fn model_with_face() -> (Model, Entity) {
    let mut model = Model::new();
    let p = |x: f64, y: f64| glam::DVec3::new(x, y, 0.0);
    let corners = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
    let segs: Vec<_> = (0..4).map(|i| (corners[i], corners[(i + 1) % 4])).collect();
    let mut undo = UndoStack::new();
    let mut t = Target {
        model: &mut model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut t, &segs, "Draw").expect("draw");
    let face = model
        .root()
        .geometry()
        .faces()
        .next()
        .map(|(id, _)| Entity::Face(id))
        .expect("face");
    (model, face)
}

#[test]
fn create_adds_selects_and_is_one_undo_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let id = editor
        .create_material(&mut model, mat("Brick", [0.7, 0.2, 0.1, 1.0]))
        .expect("created");
    assert_eq!(model.material(id).unwrap().name, "Brick");
    assert_eq!(editor.material(), Some(id));
    assert_eq!(editor.message(), None);
    assert_eq!(undo_label(&editor).as_deref(), Some("New Material"));
}

#[test]
fn create_makes_names_unique() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    editor.create_material(&mut model, mat("Brick", [1.0, 0.0, 0.0, 1.0]));
    let id = editor
        .create_material(&mut model, mat("Brick", [0.0, 1.0, 0.0, 1.0]))
        .unwrap();
    assert_eq!(model.material(id).unwrap().name, "Brick 2");
}

#[test]
fn create_with_bad_color_is_refused() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let before = editor.create_material(&mut model, mat("A", [1.0, 1.0, 1.0, 1.0]));
    assert!(before.is_some());
    let current = editor.material();
    let steps = editor.undo_stack().undo_count();
    assert_eq!(
        editor.create_material(&mut model, mat("B", [2.0, 0.0, 0.0, 1.0])),
        None
    );
    assert_eq!(editor.message(), Some("Can't make that material"));
    assert_eq!(editor.material(), current);
    assert_eq!(model.materials().count(), 1);
    assert_eq!(editor.undo_stack().undo_count(), steps);
}

#[test]
fn edit_replaces_the_material() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let id = editor
        .create_material(&mut model, mat("Brick", [1.0, 0.0, 0.0, 1.0]))
        .unwrap();
    assert!(editor.edit_material(&mut model, id, mat("Red brick", [0.5, 0.0, 0.0, 0.5])));
    let m = model.material(id).unwrap();
    assert_eq!(m.name, "Red brick");
    assert_eq!(m.color, [0.5, 0.0, 0.0, 0.5]);
    assert_eq!(undo_label(&editor).as_deref(), Some("Edit Material"));
    assert_eq!(editor.message(), None);
}

#[test]
fn edit_with_no_change_is_accepted_without_an_undo_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let id = editor
        .create_material(&mut model, mat("Brick", [1.0, 0.0, 0.0, 1.0]))
        .unwrap();
    let steps = editor.undo_stack().undo_count();
    assert!(editor.edit_material(&mut model, id, mat("Brick", [1.0, 0.0, 0.0, 1.0])));
    assert_eq!(editor.undo_stack().undo_count(), steps);
}

#[test]
fn edit_refuses_a_missing_material_and_a_bad_texture_size() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    assert!(!editor.edit_material(&mut model, MaterialId(99), mat("X", [1.0; 4])));
    assert_eq!(editor.message(), Some("Can't change that material"));

    let id = editor
        .create_material(&mut model, mat("Tile", [1.0; 4]))
        .unwrap();
    let mut bad = mat("Tile", [1.0; 4]);
    bad.texture = Some(Texture::new("t.png", glam::DVec2::new(0.0, 1.0)));
    assert!(!editor.edit_material(&mut model, id, bad));
    assert_eq!(editor.message(), Some("Can't change that material"));
    assert!(model.material(id).unwrap().texture.is_none());
}

#[test]
fn purge_removes_unused_and_keeps_used() {
    let (mut model, face) = model_with_face();
    let mut editor = Editor::new();
    let used = editor
        .create_material(&mut model, mat("Used", [1.0, 0.0, 0.0, 1.0]))
        .unwrap();
    let unused = editor
        .create_material(&mut model, mat("Unused", [0.0, 1.0, 0.0, 1.0]))
        .unwrap();
    let mut undo = UndoStack::new();
    paint(
        &mut model,
        BodyId::Root,
        &[face],
        Side::Front,
        Some(used),
        &mut undo,
        "Paint",
    )
    .unwrap();
    editor.set_material(Some(used));
    assert_eq!(editor.purge_materials(&mut model), 1);
    assert!(model.material(used).is_some());
    assert!(model.material(unused).is_none());
    assert_eq!(editor.material(), Some(used));
    assert_eq!(editor.message(), None);
    assert_eq!(undo_label(&editor).as_deref(), Some("Purge Materials"));
}

#[test]
fn purge_resets_a_current_material_that_went() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let id = editor
        .create_material(&mut model, mat("Lonely", [1.0; 4]))
        .unwrap();
    assert_eq!(editor.material(), Some(id));
    assert_eq!(editor.purge_materials(&mut model), 1);
    assert_eq!(editor.material(), None);
}

#[test]
fn purge_with_nothing_to_remove_says_so() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let steps = editor.undo_stack().undo_count();
    assert_eq!(editor.purge_materials(&mut model), 0);
    assert_eq!(editor.message(), Some("No unused materials"));
    assert_eq!(editor.undo_stack().undo_count(), steps);
}
