// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-app-components-ui: Edit menu Make Component / Make Group / Explode, and the name
//! prompt, driven with egui_kittest. See docs/specs/m4-app-components-ui.md.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::chrome::{MenuState, NamePrompt, PromptOutcome, menu_bar, name_prompt};
use modeler::input::Action;
use modeler::viewstate::{Effect, ViewState};
use std::cell::RefCell;
use std::rc::Rc;

type Seen = Rc<RefCell<Vec<Action>>>;

fn menu_harness(state: MenuState<'static>, seen: &Seen) -> Harness<'static> {
    let seen = seen.clone();
    Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            seen.borrow_mut().push(action);
        }
    })
}

fn state(has_selection: bool, can_explode: bool) -> MenuState<'static> {
    MenuState {
        undo: None,
        redo: None,
        parallel: false,
        has_selection,
        can_explode,
    }
}

fn open_edit(h: &mut Harness<'_>) {
    h.get_by_label("Edit").click();
    h.run();
}

fn choose(state: MenuState<'static>, item: &str) -> Vec<Action> {
    let seen = Seen::default();
    let mut h = menu_harness(state, &seen);
    h.run();
    open_edit(&mut h);
    h.get_by_label_contains(item).click();
    h.run();
    seen.borrow().clone()
}

#[test]
fn edit_menu_lists_the_component_items() {
    let seen = Seen::default();
    let mut h = menu_harness(state(true, true), &seen);
    h.run();
    open_edit(&mut h);
    h.get_by_label("Make Component...");
    h.get_by_label("Make Group...");
    h.get_by_label("Explode");
}

#[test]
fn make_component_and_group_send_their_actions() {
    assert_eq!(
        choose(state(true, false), "Make Component"),
        vec![Action::MakeComponent]
    );
    assert_eq!(
        choose(state(true, false), "Make Group"),
        vec![Action::MakeGroup]
    );
}

#[test]
fn explode_sends_its_action() {
    assert_eq!(choose(state(true, true), "Explode"), vec![Action::Explode]);
}

#[test]
fn make_items_need_a_selection() {
    for item in ["Make Component", "Make Group"] {
        let seen = Seen::default();
        let mut h = menu_harness(state(false, false), &seen);
        h.run();
        open_edit(&mut h);
        let node = h.get_by_label_contains(item);
        assert!(node.accesskit_node().is_disabled(), "{item} is disabled");
        node.click();
        h.run();
        assert!(seen.borrow().is_empty(), "{item}: {:?}", seen.borrow());
    }
}

#[test]
fn explode_needs_one_instance_selected() {
    let seen = Seen::default();
    let mut h = menu_harness(state(true, false), &seen);
    h.run();
    open_edit(&mut h);
    let node = h.get_by_label("Explode");
    assert!(node.accesskit_node().is_disabled());
    node.click();
    h.run();
    assert!(seen.borrow().is_empty(), "{:?}", seen.borrow());
}

#[test]
fn actions_become_effects() {
    let mut view = ViewState::new();
    assert_eq!(
        view.apply(Action::MakeComponent, (800.0, 600.0)),
        Effect::MakeComponent { group: false }
    );
    assert_eq!(
        view.apply(Action::MakeGroup, (800.0, 600.0)),
        Effect::MakeComponent { group: true }
    );
    assert_eq!(view.apply(Action::Explode, (800.0, 600.0)), Effect::Explode);
}

// ---------------------------------------------------------------------------------------
// Name prompt

struct PromptState {
    prompt: NamePrompt,
    answers: Vec<PromptOutcome>,
}

fn prompt_harness(group: bool) -> Harness<'static, PromptState> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut PromptState| {
            if let Some(answer) = name_prompt(ui.ctx(), &mut state.prompt) {
                state.answers.push(answer);
            }
        },
        PromptState {
            prompt: NamePrompt::new(group),
            answers: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn prompt_title_says_what_is_made() {
    let h = prompt_harness(false);
    h.get_by_label("Make Component");
    h.get_by_label("OK");
    h.get_by_label("Cancel");
    let h = prompt_harness(true);
    h.get_by_label("Make Group");
}

#[test]
fn name_field_has_focus_when_the_prompt_opens() {
    let mut h = prompt_harness(false);
    // No click on the field: typing goes straight into it.
    h.get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("Shelf");
    h.run();
    assert_eq!(h.state().prompt.name, "Shelf");
    assert!(h.state().answers.is_empty(), "nothing answered yet");
}

#[test]
fn ok_answers_with_the_trimmed_name() {
    let mut h = prompt_harness(false);
    h.get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("  Window frame ");
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(
        h.state().answers,
        vec![PromptOutcome::Ok("Window frame".to_string())]
    );
}

#[test]
fn enter_answers_ok() {
    let mut h = prompt_harness(true);
    h.get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("Legs");
    h.run();
    h.key_press(eframe::egui::Key::Enter);
    h.run();
    assert_eq!(
        h.state().answers,
        vec![PromptOutcome::Ok("Legs".to_string())]
    );
}

#[test]
fn empty_name_is_allowed() {
    let mut h = prompt_harness(false);
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().answers, vec![PromptOutcome::Ok(String::new())]);
}

#[test]
fn cancel_and_escape_answer_cancel() {
    let mut h = prompt_harness(false);
    h.get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("Shelf");
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().answers, vec![PromptOutcome::Cancel]);

    let mut h = prompt_harness(false);
    h.get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("Shelf");
    h.run();
    h.key_press(eframe::egui::Key::Escape);
    h.run();
    assert_eq!(h.state().answers, vec![PromptOutcome::Cancel]);
}

#[test]
fn prompt_answers_once() {
    let mut h = prompt_harness(false);
    h.get_by_label("OK").click();
    h.run();
    h.run();
    assert_eq!(h.state().answers.len(), 1, "{:?}", h.state().answers);
}
