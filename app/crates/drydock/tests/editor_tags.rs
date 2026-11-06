// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-panel: the editor's tag operations, each one undo step.
//! See docs/specs/m10-tags-panel.md.

use drydock::editor::*;
use drydock::input::EditKey;
use drydock_kernel::model::*;
use glam::DMat4;

fn undo_label(editor: &Editor) -> Option<String> {
    editor.undo_stack().undo_label().map(str::to_owned)
}

fn steps(editor: &Editor) -> usize {
    editor.undo_stack().undo_count()
}

/// An instance of an empty component in the root.
fn instance(model: &mut Model) -> InstanceId {
    let mut log = ModelEdit::new();
    let definition = model.add_definition(
        DefinitionInfo {
            name: "part".into(),
            group: false,
        },
        &mut log,
    );
    model
        .add_instance(
            BodyId::Root,
            Instance {
                definition,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap()
}

#[test]
fn add_is_one_undo_step_called_new_tag() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let id = editor.add_tag(&mut model, "Walls").expect("added");
    let tag = model.tag(id).unwrap();
    assert_eq!(tag.name, "Walls");
    assert!(tag.visible);
    assert_eq!(steps(&editor), 1);
    assert_eq!(undo_label(&editor).as_deref(), Some("New Tag"));
    assert_eq!(editor.message(), None);

    assert!(editor.key(EditKey::Undo, &mut model));
    assert!(model.tag(id).is_none());
    assert!(editor.key(EditKey::Redo, &mut model));
    assert_eq!(model.tag(id).unwrap().name, "Walls");
}

#[test]
fn add_with_a_blank_name_is_refused_with_a_message() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    assert_eq!(editor.add_tag(&mut model, "   "), None);
    assert!(editor.message().is_some());
    assert_eq!(steps(&editor), 0);
    assert!(model.tags().is_empty());
}

#[test]
fn rename_is_one_undo_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let id = editor.add_tag(&mut model, "Walls").unwrap();
    assert!(editor.rename_tag(&mut model, id, "Roof"));
    assert_eq!(model.tag(id).unwrap().name, "Roof");
    assert_eq!(steps(&editor), 2);
    assert_eq!(undo_label(&editor).as_deref(), Some("Rename Tag"));
    assert!(editor.key(EditKey::Undo, &mut model));
    assert_eq!(model.tag(id).unwrap().name, "Walls");
}

#[test]
fn rename_to_the_same_name_is_accepted_without_a_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let id = editor.add_tag(&mut model, "Walls").unwrap();
    assert!(editor.rename_tag(&mut model, id, "Walls"));
    assert_eq!(steps(&editor), 1);
}

#[test]
fn rename_refused_for_a_blank_name_or_a_missing_tag() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let id = editor.add_tag(&mut model, "Walls").unwrap();
    assert!(!editor.rename_tag(&mut model, id, " "));
    assert!(editor.message().is_some());
    assert_eq!(model.tag(id).unwrap().name, "Walls");
    assert!(!editor.rename_tag(&mut model, drydock_kernel::tags::TagId(999), "X"));
    assert_eq!(steps(&editor), 1);
}

#[test]
fn show_and_hide_are_one_step_each_with_their_own_label() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let id = editor.add_tag(&mut model, "Walls").unwrap();
    assert!(editor.set_tag_visible(&mut model, id, false));
    assert!(!model.tag(id).unwrap().visible);
    assert_eq!(steps(&editor), 2);
    assert_eq!(undo_label(&editor).as_deref(), Some("Hide Tag"));
    assert!(editor.set_tag_visible(&mut model, id, true));
    assert!(model.tag(id).unwrap().visible);
    assert_eq!(undo_label(&editor).as_deref(), Some("Show Tag"));
    assert_eq!(steps(&editor), 3);

    // Unchanged: accepted, no step.
    assert!(editor.set_tag_visible(&mut model, id, true));
    assert_eq!(steps(&editor), 3);

    assert!(editor.key(EditKey::Undo, &mut model));
    assert!(!model.tag(id).unwrap().visible);
    assert!(!editor.set_tag_visible(&mut model, drydock_kernel::tags::TagId(999), false));
}

#[test]
fn delete_untags_the_instances_in_one_undo_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let inst = instance(&mut model);
    let id = editor.add_tag(&mut model, "Walls").unwrap();
    let mut log = ModelEdit::new();
    model.assign_tag(inst, Some(id), &mut log).unwrap();
    let before = steps(&editor);

    assert!(editor.remove_tag(&mut model, id));
    assert!(model.tag(id).is_none());
    assert_eq!(model.instance_tag(inst), None);
    assert_eq!(steps(&editor), before + 1);
    assert_eq!(undo_label(&editor).as_deref(), Some("Delete Tag"));

    assert!(editor.key(EditKey::Undo, &mut model));
    assert_eq!(model.tag(id).unwrap().name, "Walls");
    assert_eq!(model.instance_tag(inst), Some(id));
    assert!(editor.key(EditKey::Redo, &mut model));
    assert!(model.tag(id).is_none());
    assert_eq!(model.instance_tag(inst), None);
}

#[test]
fn delete_of_a_missing_tag_is_refused() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    assert!(!editor.remove_tag(&mut model, drydock_kernel::tags::TagId(7)));
    assert!(editor.message().is_some());
    assert_eq!(steps(&editor), 0);
}
