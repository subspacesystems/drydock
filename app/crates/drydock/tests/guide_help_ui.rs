// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m8-guide-help: the Help menu entry, driven with egui_kittest.
//! See docs/specs/m8-guide-help.md.

use drydock::chrome::{MenuState, menu_bar};
use drydock::input::Action;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::RefCell;
use std::rc::Rc;

type Seen = Rc<RefCell<Vec<Action>>>;

fn harness(seen: &Seen) -> Harness<'static> {
    let seen = seen.clone();
    let state = MenuState::default();
    Harness::builder()
        .with_size(eframe::egui::vec2(1000.0, 800.0))
        .build_ui(move |ui| {
            if let Some(action) = menu_bar(ui, &state) {
                seen.borrow_mut().push(action);
            }
        })
}

fn open_help(h: &mut Harness<'_>) {
    h.run();
    h.get_by_label("Help").click();
    h.run();
}

#[test]
fn help_menu_lists_user_guide_with_f1_above_about() {
    let seen = Seen::default();
    let mut h = harness(&seen);
    open_help(&mut h);
    assert!(h.query_by_label("F1").is_some(), "shortcut shown");
    let guide = h.get_by_label("User Guide").rect().top();
    let about = h.get_by_label("About Drydock...").rect().top();
    assert!(guide < about);
}

#[test]
fn clicking_user_guide_sends_the_action() {
    let seen = Seen::default();
    let mut h = harness(&seen);
    open_help(&mut h);
    h.get_by_label("User Guide").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::UserGuide]);
}
