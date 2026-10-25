// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-purge-confirm: Purge Unused asks before it removes anything, driven with egui_kittest.
//! See docs/specs/m4-purge-confirm.md.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::DVec3;
use modeler::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use modeler_kernel::model::{BodyId, Material, MaterialId, Model, ModelEdit, UndoStack};
use modeler_kernel::paint::{Side, paint};
use modeler_kernel::select::Entity;

const QUESTION: &str = "The following unused materials will be removed from the model:";
const NOTHING: &str = "No unused materials to remove.";

struct State {
    panel: MaterialsPanel,
    model: Model,
    actions: Vec<MaterialAction>,
}

fn add(model: &mut Model, name: &str) -> MaterialId {
    let mut log = ModelEdit::new();
    model.add_material(Material::new(name, [0.5, 0.4, 0.3, 1.0]), &mut log)
}

/// A square face at the root painted with `material` on its front.
fn paint_square(model: &mut Model, material: MaterialId) {
    let mut log = ModelEdit::new();
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(1.0, 1.0, 0.0),
        DVec3::new(0.0, 1.0, 0.0),
    ];
    model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
    let face = model
        .body(BodyId::Root)
        .unwrap()
        .geometry()
        .faces()
        .next()
        .unwrap()
        .0;
    let mut undo = UndoStack::new();
    paint(
        model,
        BodyId::Root,
        &[Entity::Face(face)],
        Side::Front,
        Some(material),
        &mut undo,
        "Paint",
    )
    .unwrap();
}

/// "Brick" (unused), "Oak" (on a face), "Glass" (unused), in that order.
fn mixed() -> Model {
    let mut model = Model::new();
    add(&mut model, "Brick");
    let oak = add(&mut model, "Oak");
    add(&mut model, "Glass");
    paint_square(&mut model, oak);
    model
}

/// Only "Oak", on a face.
fn all_used() -> Model {
    let mut model = Model::new();
    let oak = add(&mut model, "Oak");
    paint_square(&mut model, oak);
    model
}

fn harness(model: Model) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .build_ui_state(
            |ui, state: &mut State| {
                if let Some(action) = materials_panel(ui, &mut state.panel, &state.model, None) {
                    state.actions.push(action);
                }
            },
            State {
                panel: MaterialsPanel::default(),
                model,
                actions: Vec::new(),
            },
        );
    h.run();
    h
}

fn count(h: &Harness<'_, State>, label: &str) -> usize {
    h.query_all_by_label(label).count()
}

fn ask(h: &mut Harness<'_, State>) {
    h.get_by_label("Purge Unused").click();
    h.run();
}

#[test]
fn purge_unused_asks_first() {
    let mut h = harness(mixed());
    assert!(!h.state().panel.confirming());
    assert!(h.query_by_label(QUESTION).is_none());
    ask(&mut h);
    assert!(h.state().actions.is_empty(), "nothing is removed yet");
    assert!(h.state().panel.confirming());
    h.get_by_label(QUESTION);
    h.get_by_label("Purge");
    h.get_by_label("Cancel");
}

#[test]
fn the_question_names_the_unused_materials_only() {
    let mut h = harness(mixed());
    let (brick, oak, glass) = (count(&h, "Brick"), count(&h, "Oak"), count(&h, "Glass"));
    ask(&mut h);
    assert_eq!(count(&h, "Brick"), brick + 1, "Brick is listed");
    assert_eq!(count(&h, "Glass"), glass + 1, "Glass is listed");
    assert_eq!(count(&h, "Oak"), oak, "Oak is in use and not listed");
}

#[test]
fn the_unused_materials_are_listed_in_model_order() {
    let mut h = harness(mixed());
    ask(&mut h);
    let question = h.get_by_label(QUESTION).rect();
    // The new labels are the last ones of each name (the panel's list comes first).
    let brick = h.query_all_by_label("Brick").last().unwrap().rect();
    let glass = h.query_all_by_label("Glass").last().unwrap().rect();
    assert!(
        question.center().y < brick.center().y,
        "{question:?} {brick:?}"
    );
    assert!(brick.center().y < glass.center().y, "{brick:?} {glass:?}");
}

#[test]
fn purge_confirms_and_sends_purge_once() {
    let mut h = harness(mixed());
    ask(&mut h);
    h.get_by_label("Purge").click();
    h.run();
    assert_eq!(h.state().actions, vec![MaterialAction::Purge]);
    assert!(!h.state().panel.confirming());
    assert!(h.query_by_label(QUESTION).is_none());
    h.run();
    assert_eq!(h.state().actions, vec![MaterialAction::Purge]);
}

#[test]
fn cancel_closes_without_purging() {
    let mut h = harness(mixed());
    ask(&mut h);
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().actions.is_empty());
    assert!(!h.state().panel.confirming());
    assert!(h.query_by_label(QUESTION).is_none());
}

#[test]
fn escape_cancels() {
    let mut h = harness(mixed());
    ask(&mut h);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.state().actions.is_empty());
    assert!(!h.state().panel.confirming());
    assert!(h.query_by_label(QUESTION).is_none());
}

#[test]
fn with_nothing_unused_it_says_so() {
    let mut h = harness(all_used());
    ask(&mut h);
    assert!(h.state().panel.confirming());
    h.get_by_label(NOTHING);
    assert!(h.query_by_label(QUESTION).is_none());
    assert!(h.query_by_label("Purge").is_none());
    h.get_by_label("OK").click();
    h.run();
    assert!(h.state().actions.is_empty());
    assert!(!h.state().panel.confirming());
    assert!(h.query_by_label(NOTHING).is_none());
}

#[test]
fn a_model_with_no_materials_has_nothing_unused() {
    let mut h = harness(Model::new());
    ask(&mut h);
    h.get_by_label(NOTHING);
    h.get_by_label("OK").click();
    h.run();
    assert!(h.state().actions.is_empty());
}

#[test]
fn the_list_does_not_take_clicks_while_asking() {
    let mut h = harness(mixed());
    ask(&mut h);
    // The panel's own "Brick" entry is the first one; the dialog blocks it.
    h.query_all_by_label("Brick").next().unwrap().click();
    h.run();
    assert!(h.state().actions.is_empty(), "{:?}", h.state().actions);
}
