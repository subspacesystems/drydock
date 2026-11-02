// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-app-edit-view: the View menu's hide-the-rest item and its action.
//! See docs/specs/m4-app-edit-view.md.

use drydock::chrome::{MenuState, menu_bar};
use drydock::input::Action;
use drydock::viewstate::{Effect, ViewState};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::RefCell;
use std::rc::Rc;

fn choose(hide_rest: bool, item: &str) -> Vec<Action> {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        hide_rest,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    h.get_by_label_contains(item).click();
    h.run();
    seen.borrow().clone()
}

#[test]
fn the_view_menu_offers_to_hide_the_rest() {
    assert_eq!(
        choose(false, "Hide Rest of Model"),
        vec![Action::ToggleHideRest]
    );
}

#[test]
fn when_hidden_it_offers_to_show_it_again() {
    assert_eq!(
        choose(true, "Show Rest of Model"),
        vec![Action::ToggleHideRest]
    );
}

#[test]
fn the_other_wording_is_not_shown() {
    for (hide_rest, absent) in [(false, "Show Rest of Model"), (true, "Hide Rest of Model")] {
        let state = MenuState {
            hide_rest,
            ..MenuState::default()
        };
        let mut h = Harness::new_ui(move |ui| {
            menu_bar(ui, &state);
        });
        h.run();
        h.get_by_label("View").click();
        h.run();
        assert!(h.query_by_label_contains(absent).is_none(), "{absent}");
    }
}

#[test]
fn the_action_becomes_its_effect() {
    let mut view = ViewState::new();
    assert_eq!(
        view.apply(Action::ToggleHideRest, (800.0, 600.0)),
        Effect::ToggleHideRest
    );
}
