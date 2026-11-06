// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Hiding a tag drops its instances from the selection and closes an edit path through one
//! (`docs/specs/m10-tags-hidden-selection.md`).

use drydock::editor::*;
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::tags::TagId;
use glam::{DMat4, DVec3};

fn def(model: &mut Model, name: &str, log: &mut ModelEdit) -> DefinitionId {
    model.add_definition(
        DefinitionInfo {
            name: name.into(),
            group: false,
        },
        log,
    )
}

fn inst(
    model: &mut Model,
    body: BodyId,
    d: DefinitionId,
    x: f64,
    log: &mut ModelEdit,
) -> InstanceId {
    model
        .add_instance(
            body,
            Instance {
                definition: d,
                transform: DMat4::from_translation(DVec3::new(x, 0.0, 0.0)),
                name: String::new(),
                material: None,
            },
            log,
        )
        .unwrap()
}

struct World {
    model: Model,
    editor: Editor,
    /// Root instances `a` and `b` (of `leaf`), `outer` (of `mid`).
    a: InstanceId,
    b: InstanceId,
    outer: InstanceId,
    /// Inside `outer`'s definition: an instance of `leaf`.
    inner: InstanceId,
    tag: TagId,
    other: TagId,
}

/// `a` carries `tag`; `b` is untagged; `outer` carries `other`; `inner` carries `tag`.
fn world() -> World {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let leaf = def(&mut model, "leaf", &mut log);
    let mid = def(&mut model, "mid", &mut log);
    let a = inst(&mut model, BodyId::Root, leaf, 0.0, &mut log);
    let b = inst(&mut model, BodyId::Root, leaf, 5.0, &mut log);
    let outer = inst(&mut model, BodyId::Root, mid, 10.0, &mut log);
    let inner = inst(&mut model, BodyId::Definition(mid), leaf, 0.0, &mut log);
    let tag = model.add_tag("Walls", &mut log).unwrap();
    let other = model.add_tag("Roof", &mut log).unwrap();
    model.assign_tag(a, Some(tag), &mut log).unwrap();
    model.assign_tag(outer, Some(other), &mut log).unwrap();
    model.assign_tag(inner, Some(tag), &mut log).unwrap();
    World {
        model,
        editor: Editor::new(),
        a,
        b,
        outer,
        inner,
        tag,
        other,
    }
}

#[test]
fn hiding_drops_only_the_tags_instances_from_the_selection() {
    let mut w = world();
    w.editor
        .select(vec![Entity::Instance(w.a), Entity::Instance(w.b)]);
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert!(!w.model.instance_visible(w.a));
    assert_eq!(w.editor.selection(), &[Entity::Instance(w.b)]);
}

#[test]
fn hiding_keeps_the_selection_when_no_selected_instance_has_the_tag() {
    let mut w = world();
    w.editor
        .select(vec![Entity::Instance(w.b), Entity::Instance(w.outer)]);
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert_eq!(
        w.editor.selection(),
        &[Entity::Instance(w.b), Entity::Instance(w.outer)]
    );
}

#[test]
fn hiding_keeps_non_instance_entities_selected() {
    let mut w = world();
    let mut log = ModelEdit::new();
    w.model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            ctx.add_edge(
                DVec3::new(-4.0, -4.0, 0.0),
                DVec3::new(-2.0, -4.0, 0.0),
                edit,
            )
            .unwrap();
        })
        .unwrap();
    let edge = w.model.root().geometry().edges().next().unwrap().0;
    w.editor
        .select(vec![Entity::Edge(edge), Entity::Instance(w.a)]);
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert_eq!(w.editor.selection(), &[Entity::Edge(edge)]);
}

#[test]
fn showing_a_tag_does_not_touch_the_selection() {
    let mut w = world();
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    w.editor.select(vec![Entity::Instance(w.b)]);
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, true));
    assert!(w.model.instance_visible(w.a));
    assert_eq!(w.editor.selection(), &[Entity::Instance(w.b)]);
}

#[test]
fn one_undo_step_with_the_label() {
    let mut w = world();
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert_eq!(w.editor.undo_stack().undo_count(), 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Hide Tag"));
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, true));
    assert_eq!(w.editor.undo_stack().undo_count(), 2);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Show Tag"));
}

#[test]
fn undo_restores_visibility_and_leaves_the_selection_as_it_was() {
    let mut w = world();
    w.editor
        .select(vec![Entity::Instance(w.a), Entity::Instance(w.b)]);
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    let mut stack = w.editor.undo_stack().clone();
    stack.undo(&mut w.model).unwrap();
    assert!(w.model.instance_visible(w.a));
    assert_eq!(w.editor.selection(), &[Entity::Instance(w.b)]);
}

#[test]
fn no_change_and_no_undo_step_when_already_in_that_state() {
    let mut w = world();
    w.editor.select(vec![Entity::Instance(w.a)]);
    // Already visible.
    assert!(!w.editor.set_tag_visible(&mut w.model, w.tag, true));
    assert_eq!(w.editor.undo_stack().undo_count(), 0);
    assert_eq!(w.editor.selection(), &[Entity::Instance(w.a)]);
    // Hide, then hide again.
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert!(!w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert_eq!(w.editor.undo_stack().undo_count(), 1);
}

#[test]
fn an_unknown_tag_changes_nothing() {
    let mut w = world();
    w.editor.select(vec![Entity::Instance(w.a)]);
    assert!(!w.editor.set_tag_visible(&mut w.model, TagId(9999), false));
    assert!(w.model.instance_visible(w.a));
    assert_eq!(w.editor.undo_stack().undo_count(), 0);
    assert_eq!(w.editor.selection(), &[Entity::Instance(w.a)]);
}

#[test]
fn hiding_the_tag_of_the_open_instance_closes_the_path() {
    let mut w = world();
    assert!(w.editor.open_instance(&w.model, w.outer));
    assert!(w.editor.open_instance(&w.model, w.inner));
    assert_eq!(w.editor.path().steps, vec![w.outer, w.inner]);
    // `inner` carries `tag`: back to the level inside `outer`.
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert_eq!(w.editor.path().steps, vec![w.outer]);
}

#[test]
fn hiding_an_outer_tag_closes_to_the_root() {
    let mut w = world();
    assert!(w.editor.open_instance(&w.model, w.outer));
    assert!(w.editor.open_instance(&w.model, w.inner));
    assert!(w.editor.set_tag_visible(&mut w.model, w.other, false));
    assert!(w.editor.path().is_root());
}

#[test]
fn the_path_closes_to_the_outermost_hidden_level_when_two_are_hidden() {
    let mut w = world();
    // One tag on both levels: a single hide hides `outer` and `inner`.
    let mut log = ModelEdit::new();
    w.model.assign_tag(w.outer, Some(w.tag), &mut log).unwrap();
    assert!(w.editor.open_instance(&w.model, w.outer));
    assert!(w.editor.open_instance(&w.model, w.inner));
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert!(w.editor.path().is_root());
}

#[test]
fn a_path_through_visible_instances_stays_open() {
    let mut w = world();
    assert!(w.editor.open_instance(&w.model, w.outer));
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert_eq!(w.editor.path().steps, vec![w.outer]);
}

#[test]
fn closing_the_path_clears_the_selection_inside_it() {
    let mut w = world();
    assert!(w.editor.open_instance(&w.model, w.outer));
    w.editor.select(vec![Entity::Instance(w.inner)]);
    assert!(w.editor.set_tag_visible(&mut w.model, w.other, false));
    assert!(w.editor.path().is_root());
    assert!(w.editor.selection().is_empty());
}

#[test]
fn hiding_inside_an_open_instance_drops_selected_instances_of_that_level() {
    let mut w = world();
    assert!(w.editor.open_instance(&w.model, w.outer));
    w.editor.select(vec![Entity::Instance(w.inner)]);
    // `inner` carries `tag`, and `outer` (the open instance) does not.
    assert!(w.editor.set_tag_visible(&mut w.model, w.tag, false));
    assert_eq!(w.editor.path().steps, vec![w.outer]);
    assert!(w.editor.selection().is_empty());
}
