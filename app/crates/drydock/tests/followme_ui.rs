// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m9-followme-tool: the Tools > Follow Me menu item and the tool's wiring
//! (`docs/specs/m9-followme-tool.md`).

use std::cell::RefCell;
use std::rc::Rc;

use drydock::chrome::{MenuState, TOOL_BUTTONS, menu_bar};
use drydock::input::{Action, Tool};
use drydock::toolbars::{tool_from_key, tool_key};
use drydock::viewstate::ViewState;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

fn tools_harness(tool: Tool) -> (Harness<'static>, Rc<RefCell<Vec<Action>>>) {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        tool,
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
fn the_tools_menu_has_follow_me_beside_the_other_tools() {
    let (h, _) = tools_harness(Tool::Select);
    assert!(!h.get_by_label("Follow Me").accesskit_node().is_disabled());
    h.get_by_label("Paint Bucket");
    h.get_by_label("Fit Check");
}

#[test]
fn clicking_it_selects_the_tool_and_closes_the_menu() {
    let (mut h, seen) = tools_harness(Tool::Select);
    h.get_by_label("Follow Me").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::SelectTool(Tool::FollowMe)]);
    assert!(h.query_by_label("Fit Check").is_none());
}

#[test]
fn the_item_is_in_the_tools_menu_only() {
    let state = MenuState::default();
    let mut h = Harness::new_ui(move |ui| {
        menu_bar(ui, &state);
    });
    h.run();
    h.get_by_label("Edit").click();
    h.run();
    assert!(h.query_by_label("Follow Me").is_none());
}

#[test]
fn it_has_no_toolbar_button() {
    assert!(TOOL_BUTTONS.iter().all(|(tool, _)| *tool != Tool::FollowMe));
}

#[test]
fn the_action_selects_the_tool_in_the_view() {
    let mut view = ViewState::new();
    view.apply(Action::SelectTool(Tool::FollowMe), (800.0, 600.0));
    assert_eq!(view.tool, Tool::FollowMe);
}

#[test]
fn the_layout_file_word_round_trips() {
    assert_eq!(tool_key(Tool::FollowMe), "followme");
    assert_eq!(tool_from_key("followme"), Some(Tool::FollowMe));
}
