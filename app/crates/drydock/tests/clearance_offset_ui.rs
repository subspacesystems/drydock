// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-clearance-offset-ui: the Tools > Grow by Clearance and Shrink by Clearance items
//! (`docs/specs/m7-clearance-offset-ui.md`).

use drydock::chrome::{MenuState, menu_bar};
use drydock::clearance_offset::Direction;
use drydock::input::Action;
use drydock::viewstate::{Effect, ViewState};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use std::cell::RefCell;
use std::rc::Rc;

const VIEWPORT: (f64, f64) = (800.0, 600.0);

fn tools_harness(can_offset: bool) -> (Harness<'static>, Rc<RefCell<Vec<Action>>>) {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        can_offset,
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
fn the_actions_ask_the_app_and_change_nothing_themselves() {
    let mut view = ViewState::new();
    let camera = view.camera;
    let clearance = view.clearance_m;
    assert_eq!(
        view.apply(Action::GrowByClearance, VIEWPORT),
        Effect::OffsetByClearance(Direction::Grow)
    );
    assert_eq!(
        view.apply(Action::ShrinkByClearance, VIEWPORT),
        Effect::OffsetByClearance(Direction::Shrink)
    );
    assert_eq!(view.camera, camera);
    assert_eq!(view.clearance_m, clearance);
}

#[test]
fn the_tools_menu_has_both_items() {
    let (h, _) = tools_harness(true);
    h.get_by_label("Grow by Clearance");
    h.get_by_label("Shrink by Clearance");
}

#[test]
fn the_items_are_not_in_the_view_menu() {
    let state = MenuState {
        can_offset: true,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        menu_bar(ui, &state);
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    assert!(h.query_by_label("Grow by Clearance").is_none());
    assert!(h.query_by_label("Shrink by Clearance").is_none());
}

#[test]
fn both_items_are_disabled_without_a_part_selected() {
    let (mut h, seen) = tools_harness(false);
    for label in ["Grow by Clearance", "Shrink by Clearance"] {
        assert!(
            h.get_by_label(label).accesskit_node().is_disabled(),
            "{label}"
        );
    }
    h.get_by_label("Grow by Clearance").click();
    h.run();
    assert!(seen.borrow().is_empty(), "{:?}", seen.borrow());
}

#[test]
fn both_items_are_enabled_with_a_part_selected() {
    let (h, _) = tools_harness(true);
    for label in ["Grow by Clearance", "Shrink by Clearance"] {
        assert!(
            !h.get_by_label(label).accesskit_node().is_disabled(),
            "{label}"
        );
    }
}

#[test]
fn clicking_grow_chooses_the_action_and_closes_the_menu() {
    let (mut h, seen) = tools_harness(true);
    h.get_by_label("Grow by Clearance").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::GrowByClearance]);
    assert!(h.query_by_label("Shrink by Clearance").is_none());
}

#[test]
fn clicking_shrink_chooses_the_action() {
    let (mut h, seen) = tools_harness(true);
    h.get_by_label("Shrink by Clearance").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::ShrinkByClearance]);
}

#[test]
fn fit_check_and_print_setup_are_still_there() {
    let (h, _) = tools_harness(true);
    h.get_by_label("Fit Check");
    h.get_by_label("Print Setup...");
}
