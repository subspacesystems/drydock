// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-overhang: the View > Overhangs toggle. See docs/specs/m7-overhang.md.

use drydock::chrome::{MenuState, menu_bar};
use drydock::input::Action;
use drydock::viewstate::{Effect, ViewState};
use drydock_dae::DEFAULT_OVERHANG_DEGREES;
use eframe::egui::accesskit::Toggled;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn overhangs_start_off_at_the_default_angle() {
    let view = ViewState::new();
    assert!(!view.overhangs);
    assert_eq!(view.overhang_degrees, DEFAULT_OVERHANG_DEGREES);
}

#[test]
fn toggling_flips_and_asks_for_a_new_upload() {
    let mut view = ViewState::new();
    let camera = view.camera;
    assert_eq!(
        view.apply(Action::ToggleOverhangs, (800.0, 600.0)),
        Effect::ReuploadScene
    );
    assert!(view.overhangs);
    assert_eq!(view.camera, camera);
    assert_eq!(view.overhang_degrees, DEFAULT_OVERHANG_DEGREES);
    assert_eq!(
        view.apply(Action::ToggleOverhangs, (800.0, 600.0)),
        Effect::ReuploadScene
    );
    assert!(!view.overhangs);
}

#[test]
fn the_axes_toggle_leaves_overhangs_alone() {
    let mut view = ViewState::new();
    view.apply(Action::ToggleOverhangs, (800.0, 600.0));
    view.apply(Action::ToggleAxes, (800.0, 600.0));
    assert!(view.overhangs);
}

fn choose(overhangs: bool, label: &str) -> (Vec<Action>, bool) {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        overhangs,
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
    h.get_by_label(label).click();
    h.run();
    let still_open = h.query_by_label("Zoom Extents").is_some();
    let actions = seen.borrow().clone();
    (actions, still_open)
}

#[test]
fn the_view_menu_toggles_overhangs() {
    assert_eq!(choose(false, "Overhangs").0, vec![Action::ToggleOverhangs]);
    assert_eq!(choose(true, "Overhangs").0, vec![Action::ToggleOverhangs]);
}

#[test]
fn the_menu_closes_after_choosing_overhangs() {
    let (_, still_open) = choose(false, "Overhangs");
    assert!(!still_open, "the View menu should close");
}

#[test]
fn overhangs_comes_right_after_axes() {
    let state = MenuState::default();
    let mut h = Harness::new_ui(move |ui| {
        menu_bar(ui, &state);
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    let axes = h.get_by_label("Axes").rect();
    let overhangs = h.get_by_label("Overhangs").rect();
    assert!(
        overhangs.min.y > axes.min.y,
        "Overhangs ({overhangs:?}) below Axes ({axes:?})"
    );
    assert!(
        overhangs.min.y - axes.max.y < axes.height(),
        "no other item between Axes ({axes:?}) and Overhangs ({overhangs:?})"
    );
}

#[test]
fn the_menu_shows_the_overhangs_check_state() {
    for overhangs in [false, true] {
        let state = MenuState {
            overhangs,
            ..MenuState::default()
        };
        let mut h = Harness::new_ui(move |ui| {
            menu_bar(ui, &state);
        });
        h.run();
        h.get_by_label("View").click();
        h.run();
        let toggled = h.get_by_label("Overhangs").accesskit_node().toggled();
        let want = if overhangs {
            Toggled::True
        } else {
            Toggled::False
        };
        assert_eq!(toggled, Some(want), "overhangs = {overhangs}");
    }
}
