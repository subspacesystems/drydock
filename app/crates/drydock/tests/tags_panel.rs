// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-panel: the Tags panel, driven with egui_kittest. Actions go through a real
//! `Editor` the way the app applies them. See docs/specs/m10-tags-panel.md.

use drydock::chrome::{MenuState, menu_bar};
use drydock::dock::{DockLayout, PANELS, PanelId, panel_key, title};
use drydock::editor::Editor;
use drydock::input::{Action, EditKey};
use drydock::tags_panel::*;
use drydock_kernel::model::*;
use drydock_kernel::tags::TagId;
use eframe::egui;
use eframe::egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

const WIDTH: f32 = 240.0;

fn model_with(names: &[&str]) -> (Model, Vec<TagId>) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let ids = names
        .iter()
        .map(|name| model.add_tag(name, &mut log).unwrap())
        .collect();
    (model, ids)
}

// --- pure helpers ---

#[test]
fn default_name_is_the_smallest_unused_number() {
    assert_eq!(default_tag_name(&Model::new()), "Tag 1");
    let (model, _) = model_with(&["Tag 1"]);
    assert_eq!(default_tag_name(&model), "Tag 2");
    let (model, _) = model_with(&["Tag 2", "Tag 3"]);
    assert_eq!(default_tag_name(&model), "Tag 1");
    let (model, _) = model_with(&["Tag 1", "Tag 2", "Tag 4"]);
    assert_eq!(default_tag_name(&model), "Tag 3");
    // Compared exactly.
    let (model, _) = model_with(&["tag 1", "Tag 1 "]);
    assert_eq!(default_tag_name(&model), "Tag 1");
}

#[test]
fn listed_orders_by_name_ignoring_case_then_id() {
    let (model, ids) = model_with(&["walls", "Roof", "Doors", "WALLS"]);
    assert_eq!(listed_tags(&model), vec![ids[2], ids[1], ids[0], ids[3]]);
    assert!(listed_tags(&Model::new()).is_empty());
}

// --- the panel ---

struct State {
    panel: TagsPanel,
    model: Model,
    editor: Editor,
    actions: Vec<TagAction>,
}

/// Apply `action` the way `App` does.
fn apply(editor: &mut Editor, model: &mut Model, action: TagAction) {
    match action {
        TagAction::Add => {
            let name = default_tag_name(model);
            editor.add_tag(model, &name);
        }
        TagAction::Rename(id, name) => {
            editor.rename_tag(model, id, &name);
        }
        TagAction::SetVisible(id, visible) => {
            editor.set_tag_visible(model, id, visible);
        }
        TagAction::Delete(id) => {
            editor.remove_tag(model, id);
        }
    }
}

fn harness(model: Model) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                ui.allocate_ui(egui::vec2(WIDTH, 600.0), |ui| {
                    ui.set_max_width(WIDTH);
                    if let Some(action) = tags_panel(ui, &mut state.panel, &state.model) {
                        state.actions.push(action.clone());
                        apply(&mut state.editor, &mut state.model, action);
                    }
                });
            },
            State {
                panel: TagsPanel::default(),
                model,
                editor: Editor::new(),
                actions: Vec::new(),
            },
        );
    h.run();
    h
}

fn steps(h: &Harness<'_, State>) -> usize {
    h.state().editor.undo_stack().undo_count()
}

fn undo_label(h: &Harness<'_, State>) -> Option<String> {
    h.state()
        .editor
        .undo_stack()
        .undo_label()
        .map(str::to_owned)
}

fn visible_toggle(h: &Harness<'_, State>, name: &str) -> Option<Toggled> {
    h.get_by_label(&format!("Visible: {name}"))
        .accesskit_node()
        .toggled()
}

fn undo(h: &mut Harness<'_, State>) {
    let State { editor, model, .. } = h.state_mut();
    assert!(editor.key(EditKey::Undo, model));
    h.run();
}

#[test]
fn an_empty_model_shows_a_message_and_the_add_button() {
    let h = harness(Model::new());
    assert!(h.query_by_label("No tags").is_some());
    assert!(h.query_by_label("Add").is_some());
    assert!(h.state().actions.is_empty(), "nothing without a click");
}

#[test]
fn each_tag_has_a_row_in_order_with_eye_name_and_delete() {
    let (model, _) = model_with(&["walls", "Roof", "Doors"]);
    let h = harness(model);
    assert!(h.query_by_label("No tags").is_none());
    for name in ["Doors", "Roof", "walls"] {
        assert!(h.query_by_label(name).is_some(), "{name}");
        assert!(h.query_by_label(&format!("Visible: {name}")).is_some());
        assert!(h.query_by_label(&format!("Delete {name}")).is_some());
    }
    let y = |name: &str| h.get_by_label(name).rect().top();
    assert!(y("Doors") < y("Roof") && y("Roof") < y("walls"));
}

#[test]
fn the_eye_reflects_the_tag_and_clicking_it_toggles_one_undo_step() {
    let (model, ids) = model_with(&["Walls"]);
    let mut h = harness(model);
    assert_eq!(visible_toggle(&h, "Walls"), Some(Toggled::True));

    h.get_by_label("Visible: Walls").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![TagAction::SetVisible(ids[0], false)]
    );
    assert!(!h.state().model.tag(ids[0]).unwrap().visible);
    assert_eq!(visible_toggle(&h, "Walls"), Some(Toggled::False));
    assert_eq!(steps(&h), 1);
    assert_eq!(undo_label(&h).as_deref(), Some("Hide Tag"));

    h.get_by_label("Visible: Walls").click();
    h.run();
    assert!(h.state().model.tag(ids[0]).unwrap().visible);
    assert_eq!(undo_label(&h).as_deref(), Some("Show Tag"));
    assert_eq!(steps(&h), 2);

    undo(&mut h);
    assert!(!h.state().model.tag(ids[0]).unwrap().visible);
    assert_eq!(visible_toggle(&h, "Walls"), Some(Toggled::False));
}

#[test]
fn add_makes_tag_1_then_tag_2_each_one_undo_step() {
    let mut h = harness(Model::new());
    h.get_by_label("Add").click();
    h.run();
    assert_eq!(h.state().actions, vec![TagAction::Add]);
    assert!(h.query_by_label("Tag 1").is_some());
    assert!(h.query_by_label("No tags").is_none());
    assert_eq!(undo_label(&h).as_deref(), Some("New Tag"));
    h.get_by_label("Add").click();
    h.run();
    assert!(h.query_by_label("Tag 2").is_some());
    assert_eq!(steps(&h), 2);
    assert_eq!(h.state().model.tags().len(), 2);

    undo(&mut h);
    assert!(h.query_by_label("Tag 2").is_none());
    assert!(h.query_by_label("Tag 1").is_some());
    undo(&mut h);
    assert!(h.query_by_label("No tags").is_some());
}

#[test]
fn delete_removes_the_tag_in_one_undo_step() {
    let (model, ids) = model_with(&["Walls", "Roof"]);
    let mut h = harness(model);
    h.get_by_label("Delete Walls").click();
    h.run();
    assert_eq!(h.state().actions, vec![TagAction::Delete(ids[0])]);
    assert!(h.state().model.tag(ids[0]).is_none());
    assert!(h.query_by_label("Walls").is_none());
    assert!(h.query_by_label("Roof").is_some());
    assert_eq!(undo_label(&h).as_deref(), Some("Delete Tag"));
    assert_eq!(steps(&h), 1);

    undo(&mut h);
    assert_eq!(h.state().model.tag(ids[0]).unwrap().name, "Walls");
    assert!(h.query_by_label("Walls").is_some());
}

/// Start a rename the way a double-click does (kittest can't send a double-click): the
/// panel's `editing` holds the tag and the text so far. The tests start from an empty text
/// so what they type is the whole new name; that a real double-click starts with the old
/// name selected is checked in the window at sign-off.
fn start_rename(h: &mut Harness<'_, State>, id: TagId, text: &str) {
    h.state_mut().panel.editing = Some((id, text.to_string()));
    h.run();
}

#[test]
fn rename_shows_a_text_box_and_enter_confirms_one_undo_step() {
    let (model, ids) = model_with(&["Walls"]);
    let mut h = harness(model);
    start_rename(&mut h, ids[0], "");
    assert!(h.query_by_role(Role::TextInput).is_some());
    h.get_by_role(Role::TextInput).type_text("Roof");
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(
        h.state().actions,
        vec![TagAction::Rename(ids[0], "Roof".to_string())]
    );
    assert_eq!(h.state().model.tag(ids[0]).unwrap().name, "Roof");
    assert_eq!(h.state().panel.editing, None);
    assert!(h.query_by_role(Role::TextInput).is_none());
    assert!(h.query_by_label("Roof").is_some());
    assert_eq!(undo_label(&h).as_deref(), Some("Rename Tag"));
    assert_eq!(steps(&h), 1);

    undo(&mut h);
    assert_eq!(h.state().model.tag(ids[0]).unwrap().name, "Walls");
}

#[test]
fn escape_cancels_a_rename() {
    let (model, ids) = model_with(&["Walls"]);
    let mut h = harness(model);
    start_rename(&mut h, ids[0], "");
    h.get_by_role(Role::TextInput).type_text("Roof");
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().panel.editing, None);
    assert_eq!(h.state().model.tag(ids[0]).unwrap().name, "Walls");
    assert!(h.state().actions.is_empty());
    assert_eq!(steps(&h), 0);
    assert!(h.query_by_label("Walls").is_some());
}

#[test]
fn an_empty_or_unchanged_name_ends_the_rename_without_a_step() {
    let (model, ids) = model_with(&["Walls"]);
    let mut h = harness(model);

    start_rename(&mut h, ids[0], "Walls");
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().panel.editing, None, "unchanged");

    start_rename(&mut h, ids[0], "  ");
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().panel.editing, None, "blank");
    assert_eq!(h.state().model.tag(ids[0]).unwrap().name, "Walls");
    assert!(h.state().actions.is_empty(), "no action for either");
    assert_eq!(steps(&h), 0);
}

#[test]
fn a_rename_of_a_tag_that_went_away_is_dropped() {
    let (model, ids) = model_with(&["Walls", "Roof"]);
    let mut h = harness(model);
    start_rename(&mut h, ids[0], "Walls");
    {
        let State { editor, model, .. } = h.state_mut();
        assert!(editor.remove_tag(model, ids[0]));
    }
    h.run();
    assert_eq!(h.state().panel.editing, None);
    assert!(h.query_by_role(Role::TextInput).is_none());
}

// --- dock and menu ---

#[test]
fn the_panel_is_registered_between_components_and_shadows() {
    assert_eq!(title(PanelId::Tags), "Tags");
    assert_eq!(panel_key(PanelId::Tags), "tags");
    let at = |id| PANELS.iter().position(|p| *p == id).unwrap();
    assert!(at(PanelId::Components) < at(PanelId::Tags));
    assert!(at(PanelId::Tags) < at(PanelId::Shadows));
    assert!(!DockLayout::default().is_open(PanelId::Tags));
}

#[test]
fn the_view_menu_offers_tags() {
    let seen: std::rc::Rc<std::cell::RefCell<Vec<Action>>> = Default::default();
    let sink = seen.clone();
    let state = MenuState::default();
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    h.get_by_label("Tags").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::TogglePanel(PanelId::Tags)]);
}
