// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-clearance-ui: the Tools > Fit Check item, the clearance field and the view state
//! behind them (`docs/specs/m7-clearance-ui.md`).

use eframe::egui::Key;
use eframe::egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::chrome::{MenuState, clearance_field, menu_bar};
use modeler::input::Action;
use modeler::viewstate::{Effect, ViewState};
use modeler_dae::DEFAULT_CLEARANCE_M;
use std::cell::RefCell;
use std::rc::Rc;

const VIEWPORT: (f64, f64) = (800.0, 600.0);

#[test]
fn the_clearance_starts_at_the_default() {
    assert_eq!(ViewState::new().clearance_m, DEFAULT_CLEARANCE_M);
}

#[test]
fn the_action_asks_the_app_and_changes_nothing_itself() {
    let mut view = ViewState::new();
    let camera = view.camera;
    assert_eq!(view.apply(Action::FitCheck, VIEWPORT), Effect::FitCheck);
    assert_eq!(view.camera, camera);
    assert_eq!(view.clearance_m, DEFAULT_CLEARANCE_M);
    assert!(!view.overhangs);
}

#[test]
fn a_new_clearance_is_stored_in_meters_and_recomputes() {
    let mut view = ViewState::new();
    assert_eq!(view.set_clearance_mm(0.5), Effect::RecomputeFit);
    assert!((view.clearance_m - 0.0005).abs() < 1e-12);
    assert_eq!(view.set_clearance_mm(0.5), Effect::None, "same value");
    assert_eq!(view.set_clearance_mm(5.0), Effect::RecomputeFit);
    assert!((view.clearance_m - 0.005).abs() < 1e-12);
    assert_eq!(view.set_clearance_mm(0.01), Effect::RecomputeFit);
    assert!((view.clearance_m - 0.00001).abs() < 1e-12);
}

#[test]
fn a_clearance_out_of_range_changes_nothing() {
    let mut view = ViewState::new();
    for mm in [0.0, 0.009, 5.01, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(view.set_clearance_mm(mm), Effect::None, "{mm}");
        assert_eq!(view.clearance_m, DEFAULT_CLEARANCE_M, "{mm}");
    }
}

// ---------------------------------------------------------------------------------------
// Menu

fn tools_harness(
    fit_check: bool,
    can_fit_check: bool,
) -> (Harness<'static>, Rc<RefCell<Vec<Action>>>) {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        fit_check,
        can_fit_check,
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
fn the_tools_menu_has_fit_check() {
    let (h, _) = tools_harness(false, true);
    h.get_by_label("Fit Check");
    // It is a Tools item, not a View item.
    drop(h);
    let state = MenuState {
        can_fit_check: true,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        menu_bar(ui, &state);
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    assert!(h.query_by_label("Fit Check").is_none());
}

#[test]
fn fit_check_needs_two_parts_selected() {
    let (mut h, seen) = tools_harness(false, false);
    let node = h.get_by_label("Fit Check");
    assert!(node.accesskit_node().is_disabled());
    node.click();
    h.run();
    assert!(seen.borrow().is_empty(), "{:?}", seen.borrow());
}

#[test]
fn fit_check_with_two_parts_selected_is_enabled() {
    let (mut h, seen) = tools_harness(false, true);
    let node = h.get_by_label("Fit Check");
    assert!(!node.accesskit_node().is_disabled());
    node.click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::FitCheck]);
    assert!(
        h.query_by_label("Select").is_none(),
        "the Tools menu closes after choosing"
    );
}

#[test]
fn a_showing_check_can_always_be_switched_off() {
    // The selection may be anything by then; the item stays enabled while checked.
    let (mut h, seen) = tools_harness(true, false);
    let node = h.get_by_label("Fit Check");
    assert!(!node.accesskit_node().is_disabled());
    node.click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::FitCheck]);
}

#[test]
fn the_menu_shows_the_check_state() {
    for fit_check in [false, true] {
        let (h, _) = tools_harness(fit_check, true);
        let want = if fit_check {
            Toggled::True
        } else {
            Toggled::False
        };
        assert_eq!(
            h.get_by_label("Fit Check").accesskit_node().toggled(),
            Some(want),
            "fit_check = {fit_check}"
        );
    }
}

#[test]
fn the_tool_items_are_still_there() {
    let (h, _) = tools_harness(false, false);
    for label in ["Select", "Line", "Rectangle", "Extrude"] {
        h.get_by_label(label);
    }
}

// ---------------------------------------------------------------------------------------
// Clearance field

struct Field {
    text: String,
    current_m: f64,
    committed: Vec<Option<f64>>,
}

fn field_harness(text: &str) -> Harness<'static, Field> {
    let state = Field {
        text: text.to_string(),
        current_m: DEFAULT_CLEARANCE_M,
        committed: vec![],
    };
    let mut h = Harness::new_ui_state(
        |ui, s: &mut Field| {
            if let Some(m) = clearance_field(ui, &mut s.text, s.current_m) {
                s.committed.push(Some(m));
            }
        },
        state,
    );
    h.run();
    h
}

/// Click into the (empty) box, then type.
fn type_into(h: &mut Harness<'static, Field>, text: &str) {
    h.get_by_role(Role::TextInput).click();
    h.run();
    h.get_by_role(Role::TextInput).type_text(text);
    h.run();
}

#[test]
fn the_field_is_labelled_in_millimeters() {
    let h = field_harness("0.2");
    h.get_by_label("Clearance");
    h.get_by_label("mm");
    assert_eq!(h.state().text, "0.2");
}

#[test]
fn typing_alone_commits_nothing() {
    let mut h = field_harness("");
    type_into(&mut h, "0.25");
    assert_eq!(h.state().text, "0.25");
    assert!(h.state().committed.is_empty());
}

#[test]
fn enter_commits_a_good_value_in_meters() {
    let mut h = field_harness("");
    type_into(&mut h, "0.25");
    h.key_press(Key::Enter);
    h.run();
    // Reported at least once, always the same value.
    assert!(!h.state().committed.is_empty());
    for m in &h.state().committed {
        let m = m.expect("a value");
        assert!((m - 0.00025).abs() < 1e-12, "{m}");
    }
    assert_eq!(h.state().text, "0.25");
}

#[test]
fn enter_on_a_bad_value_puts_the_old_one_back() {
    let mut h = field_harness("");
    type_into(&mut h, "x");
    assert_eq!(h.state().text, "x");
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().committed.is_empty());
    assert_eq!(h.state().text, "0.2");
}

#[test]
fn a_value_out_of_range_is_put_back_too() {
    let mut h = field_harness("");
    type_into(&mut h, "50");
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().committed.is_empty());
    assert_eq!(h.state().text, "0.2");
}
