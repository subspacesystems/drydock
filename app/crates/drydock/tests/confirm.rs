// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-confirm: confirmation and name dialogs, and the Customize Toolbars window asking
//! before it deletes or resets and prompting for names (`docs/specs/m6-confirm.md`).

use drydock::dialog::{confirm_dialog, name_dialog};
use drydock::toolbars::{
    CustomizePrompt, CustomizeState, Placement, ToolbarLayout, ToolbarsMenuEvent, customize_panel,
    toolbars_menu,
};
use eframe::egui::{self, Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

const DRAWING: usize = 1;
const CAMERA: usize = 4;

// ---- helpers ----

/// The enabled-or-not button whose label is exactly `label` (kittest's own lookup matches
/// substrings, and "Delete" is part of "Delete Toolbar...").
fn button<'a, S>(h: &'a Harness<'_, S>, label: &str) -> egui_kittest::Node<'a> {
    let mut found: Vec<_> = h
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter(|n| n.accesskit_node().label().as_deref() == Some(label))
        .collect();
    assert_eq!(found.len(), 1, "one button labelled {label:?}");
    found.remove(0)
}

fn has_button<S>(h: &Harness<'_, S>, label: &str) -> bool {
    h.query_all_by_role(egui::accesskit::Role::Button)
        .any(|n| n.accesskit_node().label().as_deref() == Some(label))
}

fn has_text<S>(h: &Harness<'_, S>, text: &str) -> bool {
    h.query_all_by_label_contains(text).next().is_some()
}

// ---- confirm_dialog ----

struct Ask {
    answers: Vec<Option<bool>>,
}

fn confirm() -> Harness<'static, Ask> {
    let mut h = Harness::builder()
        .with_size(vec2(600.0, 400.0))
        .build_ui_state(
            |ui, state: &mut Ask| {
                let answer = confirm_dialog(
                    ui.ctx(),
                    egui::Id::new("test-confirm"),
                    "Delete Toolbar",
                    "Delete the toolbar \"Camera\"?",
                    "Delete",
                );
                state.answers.push(answer);
            },
            Ask {
                answers: Vec::new(),
            },
        );
    h.run();
    h.state_mut().answers.clear();
    h
}

fn last(h: &Harness<'_, Ask>) -> Option<bool> {
    h.state().answers.iter().flatten().copied().last()
}

#[test]
fn a_confirmation_shows_its_title_message_and_two_buttons() {
    let h = confirm();
    assert!(has_text(&h, "Delete Toolbar"));
    assert!(has_text(&h, "Delete the toolbar \"Camera\"?"));
    button(&h, "Delete");
    button(&h, "Cancel");
    assert!(h.state().answers.iter().all(Option::is_none), "it waits");
}

#[test]
fn the_action_button_confirms() {
    let mut h = confirm();
    button(&h, "Delete").click();
    h.run();
    assert_eq!(last(&h), Some(true));
}

#[test]
fn cancel_escape_and_enter_all_decline() {
    let mut h = confirm();
    button(&h, "Cancel").click();
    h.run();
    assert_eq!(last(&h), Some(false));

    let mut h = confirm();
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(last(&h), Some(false));

    // Cancel is the default button: the safe answer.
    let mut h = confirm();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(last(&h), Some(false));
}

// ---- name_dialog ----

struct Named {
    name: String,
    answers: Vec<Option<bool>>,
}

fn named(start: &str) -> Harness<'static, Named> {
    let mut h = Harness::builder()
        .with_size(vec2(600.0, 400.0))
        .build_ui_state(
            |ui, state: &mut Named| {
                let answer = name_dialog(
                    ui.ctx(),
                    egui::Id::new("test-name"),
                    "Rename Toolbar",
                    &mut state.name,
                );
                state.answers.push(answer);
            },
            Named {
                name: start.to_owned(),
                answers: Vec::new(),
            },
        );
    h.run();
    h.state_mut().answers.clear();
    h
}

fn named_last(h: &Harness<'_, Named>) -> Option<bool> {
    h.state().answers.iter().flatten().copied().last()
}

#[test]
fn a_name_prompt_has_a_title_a_labelled_box_and_two_buttons() {
    let h = named("Drawing");
    assert!(has_text(&h, "Rename Toolbar"));
    h.get_by_label("Name:");
    button(&h, "OK");
    button(&h, "Cancel");
}

#[test]
fn typing_then_ok_accepts_the_name() {
    let mut h = named("");
    h.get_by_label("Name:").click();
    h.run();
    h.get_by_label("Name:").type_text("Shapes");
    h.run();
    assert_eq!(h.state().name, "Shapes");
    assert_eq!(named_last(&h), None, "typing alone answers nothing");
    button(&h, "OK").click();
    h.run();
    assert_eq!(named_last(&h), Some(true));
}

#[test]
fn enter_accepts_a_name() {
    let mut h = named("");
    h.get_by_label("Name:").click();
    h.run();
    h.get_by_label("Name:").type_text("Shapes");
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(named_last(&h), Some(true));
}

#[test]
fn a_blank_name_cannot_be_accepted() {
    let mut h = named("   ");
    assert!(button(&h, "OK").accesskit_node().is_disabled());
    button(&h, "OK").click();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(named_last(&h), None);
}

#[test]
fn cancel_and_escape_decline_the_name() {
    let mut h = named("Drawing");
    button(&h, "Cancel").click();
    h.run();
    assert_eq!(named_last(&h), Some(false));
    let mut h = named("Drawing");
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(named_last(&h), Some(false));
}

// ---- View > Toolbars ----

#[test]
fn the_menu_reset_asks_and_changes_nothing_itself() {
    let mut layout = ToolbarLayout::default();
    layout.set_visible(0, false);
    let before = layout.clone();
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 400.0))
        .build_ui_state(
            |ui, state: &mut (ToolbarLayout, Vec<ToolbarsMenuEvent>)| {
                if let Some(event) = toolbars_menu(ui, &mut state.0) {
                    state.1.push(event);
                }
            },
            (layout, Vec::new()),
        );
    h.run();
    button(&h, "Reset Toolbars...").click();
    h.run();
    assert_eq!(h.state().1, vec![ToolbarsMenuEvent::Reset]);
    assert_eq!(h.state().0, before);
}

// ---- Customize window ----

struct Panel {
    layout: ToolbarLayout,
    custom: CustomizeState,
    changed: Vec<bool>,
}

fn panel(layout: ToolbarLayout, selected: usize) -> Harness<'static, Panel> {
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 800.0))
        .build_ui_state(
            |ui, state: &mut Panel| {
                let changed = customize_panel(ui, &mut state.layout, &mut state.custom);
                state.changed.push(changed);
            },
            Panel {
                layout,
                custom: CustomizeState {
                    selected,
                    ..CustomizeState::default()
                },
                changed: Vec::new(),
            },
        );
    h.run();
    h
}

fn changed_since(h: &mut Harness<'_, Panel>, action: impl FnOnce(&mut Harness<'_, Panel>)) -> bool {
    h.state_mut().changed.clear();
    action(h);
    h.run();
    h.state().changed.iter().any(|&c| c)
}

fn names(layout: &ToolbarLayout) -> Vec<String> {
    layout.toolbars().iter().map(|t| t.name.clone()).collect()
}

#[test]
fn the_window_has_four_buttons_and_no_name_fields() {
    let h = panel(ToolbarLayout::default(), DRAWING);
    for label in [
        "New Toolbar...",
        "Rename Toolbar...",
        "Delete Toolbar...",
        "Reset Toolbars...",
    ] {
        button(&h, label);
    }
    assert!(h.query_by_label("New toolbar name:").is_none());
    assert!(h.query_by_label("Toolbar name:").is_none());
    assert!(!has_button(&h, "Create toolbar"));
    assert!(!has_button(&h, "Rename toolbar"));
    assert!(h.query_by_label("Name:").is_none(), "no dialog yet");
}

#[test]
fn new_asks_for_a_name_then_creates_and_selects() {
    let mut h = panel(ToolbarLayout::default(), 0);
    let changed = changed_since(&mut h, |h| button(h, "New Toolbar...").click());
    assert!(!changed, "asking changes nothing");
    assert_eq!(
        h.state().custom.prompt,
        Some(CustomizePrompt::New {
            name: String::new()
        })
    );
    assert_eq!(h.state().layout, ToolbarLayout::default());
    h.get_by_label("Name:").click();
    h.run();
    h.get_by_label("Name:").type_text("Mine");
    h.run();
    let changed = changed_since(&mut h, |h| button(h, "OK").click());
    assert!(changed);
    let layout = &h.state().layout;
    assert_eq!(layout.toolbars().len(), 6);
    assert_eq!(layout.toolbars()[5].name, "Mine");
    assert!(layout.toolbars()[5].tools.is_empty());
    assert!(matches!(
        layout.toolbars()[5].placement,
        Placement::Floating { .. }
    ));
    assert_eq!(h.state().custom.selected, 5);
    assert_eq!(h.state().custom.prompt, None);
    h.run();
    assert!(h.query_by_label("Name:").is_none(), "the dialog closed");
}

#[test]
fn cancelling_new_creates_nothing() {
    let mut h = panel(ToolbarLayout::default(), 0);
    button(&h, "New Toolbar...").click();
    h.run();
    let changed = changed_since(&mut h, |h| button(h, "Cancel").click());
    assert!(!changed);
    assert_eq!(h.state().layout, ToolbarLayout::default());
    assert_eq!(h.state().custom.prompt, None);
}

#[test]
fn rename_asks_with_the_current_name_then_renames() {
    let mut h = panel(ToolbarLayout::default(), DRAWING);
    button(&h, "Rename Toolbar...").click();
    h.run();
    assert_eq!(
        h.state().custom.prompt,
        Some(CustomizePrompt::Rename {
            index: DRAWING,
            name: "Drawing".to_owned()
        })
    );
    h.state_mut().custom.prompt = Some(CustomizePrompt::Rename {
        index: DRAWING,
        name: "Sketch".to_owned(),
    });
    h.run();
    let changed = changed_since(&mut h, |h| button(h, "OK").click());
    assert!(changed);
    assert_eq!(h.state().layout.toolbars()[DRAWING].name, "Sketch");
    assert_eq!(h.state().custom.prompt, None);
    h.run();
    h.get_by_label("Sketch");
}

#[test]
fn renaming_to_a_taken_name_numbers_it() {
    let mut h = panel(ToolbarLayout::default(), DRAWING);
    button(&h, "Rename Toolbar...").click();
    h.run();
    h.state_mut().custom.prompt = Some(CustomizePrompt::Rename {
        index: DRAWING,
        name: "Camera".to_owned(),
    });
    h.run();
    button(&h, "OK").click();
    h.run();
    assert_eq!(h.state().layout.toolbars()[DRAWING].name, "Camera 2");
}

#[test]
fn cancelling_rename_keeps_the_name() {
    let mut h = panel(ToolbarLayout::default(), DRAWING);
    button(&h, "Rename Toolbar...").click();
    h.run();
    h.get_by_label("Name:").type_text(" tools");
    h.run();
    let changed = changed_since(&mut h, |h| h.key_press(Key::Escape));
    assert!(!changed);
    assert_eq!(h.state().layout, ToolbarLayout::default());
    assert_eq!(h.state().custom.prompt, None);
}

#[test]
fn delete_asks_first_and_names_the_toolbar() {
    let mut h = panel(ToolbarLayout::default(), CAMERA);
    let changed = changed_since(&mut h, |h| button(h, "Delete Toolbar...").click());
    assert!(!changed, "asking deletes nothing");
    assert_eq!(
        h.state().custom.prompt,
        Some(CustomizePrompt::Delete { index: CAMERA })
    );
    assert_eq!(h.state().layout, ToolbarLayout::default());
    assert!(has_text(&h, "\"Camera\""), "the message names the toolbar");
    let changed = changed_since(&mut h, |h| button(h, "Delete").click());
    assert!(changed);
    assert_eq!(h.state().layout.toolbars().len(), 4);
    assert!(!names(&h.state().layout).contains(&"Camera".to_owned()));
    assert_eq!(h.state().custom.selected, 3, "selection clamped");
    assert_eq!(h.state().custom.prompt, None);
}

#[test]
fn cancelling_delete_keeps_the_toolbar() {
    let mut h = panel(ToolbarLayout::default(), CAMERA);
    button(&h, "Delete Toolbar...").click();
    h.run();
    let changed = changed_since(&mut h, |h| button(h, "Cancel").click());
    assert!(!changed);
    assert_eq!(h.state().layout, ToolbarLayout::default());
    assert_eq!(h.state().custom.prompt, None);
}

#[test]
fn reset_asks_first_then_restores_the_default() {
    let mut layout = ToolbarLayout::default();
    layout.add_toolbar("Mine");
    layout.remove_tool(DRAWING, 0);
    let before = layout.clone();
    let mut h = panel(layout, 5);
    let changed = changed_since(&mut h, |h| button(h, "Reset Toolbars...").click());
    assert!(!changed);
    assert_eq!(h.state().custom.prompt, Some(CustomizePrompt::Reset));
    assert_eq!(h.state().layout, before);
    let changed = changed_since(&mut h, |h| button(h, "Reset").click());
    assert!(changed);
    assert_eq!(h.state().layout, ToolbarLayout::default());
    assert!(h.state().custom.selected < 5, "selection clamped");
    assert_eq!(h.state().custom.prompt, None);
}

#[test]
fn cancelling_reset_keeps_the_layout() {
    let mut layout = ToolbarLayout::default();
    layout.add_toolbar("Mine");
    let before = layout.clone();
    let mut h = panel(layout, 0);
    button(&h, "Reset Toolbars...").click();
    h.run();
    let changed = changed_since(&mut h, |h| button(h, "Cancel").click());
    assert!(!changed);
    assert_eq!(h.state().layout, before);
}

#[test]
fn rename_and_delete_are_off_without_toolbars() {
    let mut layout = ToolbarLayout::default();
    while !layout.toolbars().is_empty() {
        layout.remove_toolbar(0);
    }
    let h = panel(layout, 0);
    assert!(
        button(&h, "Rename Toolbar...")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        button(&h, "Delete Toolbar...")
            .accesskit_node()
            .is_disabled()
    );
    assert!(!button(&h, "New Toolbar...").accesskit_node().is_disabled());
}
