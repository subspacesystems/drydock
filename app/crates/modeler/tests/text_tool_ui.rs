// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-text-3d-ui: the Tools > 3D Text... item and its dialog
//! (`docs/specs/m7-text-3d-ui.md`).

use std::cell::RefCell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::chrome::{MenuState, menu_bar};
use modeler::input::Action;
use modeler::text_tool::*;
use modeler::viewstate::{Effect, ViewState};
use modeler_kernel::text3d::TextMode;

fn tools_harness(can_text: bool) -> (Harness<'static>, Rc<RefCell<Vec<Action>>>) {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        can_text,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("Tools").click();
    h.run();
    (h, seen)
}

#[test]
fn the_action_asks_the_app_and_changes_nothing_itself() {
    let mut view = ViewState::new();
    let camera = view.camera;
    assert_eq!(
        view.apply(Action::Text3d, (800.0, 600.0)),
        Effect::Text3dDialog
    );
    assert_eq!(view.camera, camera);
}

#[test]
fn the_tools_menu_has_the_item_and_the_others_are_still_there() {
    let (h, _) = tools_harness(true);
    h.get_by_label("3D Text...");
    h.get_by_label("Fit Check");
    h.get_by_label("Grow by Clearance");
    h.get_by_label("Print Setup...");
}

#[test]
fn the_item_is_not_in_the_view_menu() {
    let state = MenuState {
        can_text: true,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        menu_bar(ui, &state);
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    assert!(h.query_by_label("3D Text...").is_none());
}

#[test]
fn the_item_is_disabled_without_a_model_and_does_nothing() {
    let (mut h, seen) = tools_harness(false);
    assert!(h.get_by_label("3D Text...").accesskit_node().is_disabled());
    h.get_by_label("3D Text...").click();
    h.run();
    assert!(seen.borrow().is_empty(), "{:?}", seen.borrow());
}

#[test]
fn clicking_the_item_chooses_the_action_and_closes_the_menu() {
    let (mut h, seen) = tools_harness(true);
    assert!(!h.get_by_label("3D Text...").accesskit_node().is_disabled());
    h.get_by_label("3D Text...").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::Text3d]);
    assert!(h.query_by_label("Fit Check").is_none());
}

// ---- the dialog ----

struct State {
    dialog: TextDialog,
    outcomes: Vec<TextOutcome>,
}

fn dialog_harness() -> Harness<'static, State> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            if let Some(outcome) = text_dialog(ui, &mut state.dialog) {
                state.outcomes.push(outcome);
            }
        },
        State {
            dialog: TextDialog::new(),
            outcomes: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn the_dialog_has_the_fields_the_modes_and_the_buttons() {
    let h = dialog_harness();
    for label in [
        "Text",
        "Font size (mm)",
        "Height (mm)",
        "Emboss",
        "Engrave",
        "OK",
        "Cancel",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.state().outcomes.is_empty());
}

#[test]
fn the_mode_radios_set_the_mode() {
    let mut h = dialog_harness();
    assert_eq!(h.state().dialog.mode, TextMode::Emboss);
    h.get_by_label("Engrave").click();
    h.run();
    assert_eq!(h.state().dialog.mode, TextMode::Engrave);
    h.get_by_label("Emboss").click();
    h.run();
    assert_eq!(h.state().dialog.mode, TextMode::Emboss);
}

#[test]
fn ok_waits_for_text_and_numbers_that_parse() {
    let mut h = dialog_harness();
    assert!(h.get_by_label("OK").accesskit_node().is_disabled());
    h.get_by_label("OK").click();
    h.run();
    assert!(h.state().outcomes.is_empty(), "no text yet");

    h.state_mut().dialog.text = "Label".into();
    h.state_mut().dialog.size_text = "big".into();
    h.run();
    assert!(h.get_by_label("OK").accesskit_node().is_disabled());

    h.state_mut().dialog.size_text = "12 mm".into();
    h.run();
    assert!(!h.get_by_label("OK").accesskit_node().is_disabled());
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![TextOutcome::Ok]);
    let options = h.state().dialog.options().expect("options");
    assert_eq!(options.text, "Label");
    assert_eq!(options.size_m, 0.012);
}

#[test]
fn cancel_closes_the_dialog() {
    let mut h = dialog_harness();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![TextOutcome::Cancel]);
}
