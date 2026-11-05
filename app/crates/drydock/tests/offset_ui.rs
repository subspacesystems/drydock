// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Offset tool's button, key, icon, menu entry and tooltip (`docs/specs/m9-offset-tool.md`),
//! driven with egui_kittest.

use drydock::chrome::{MenuState, TOOL_BUTTONS, TOOL_GROUPS, menu_bar, tool_shortcut, toolbar};
use drydock::icons::{self, Icon, TOOLS};
use drydock::input::{Action, Modifiers, Tool, key};
use drydock::toolbars::{ToolbarLayout, tool_from_key, tool_key, tool_tooltip};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::RefCell;
use std::rc::Rc;

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

#[test]
fn the_button_table_has_offset_right_after_extrude() {
    let at = TOOL_BUTTONS
        .iter()
        .position(|(t, _)| *t == Tool::Offset)
        .expect("Offset button");
    assert_eq!(TOOL_BUTTONS[at].1, "Offset");
    assert_eq!(TOOL_BUTTONS[at - 1].0, Tool::PushPull);
    assert_eq!(TOOL_BUTTONS.len(), 18);
    assert_eq!(TOOLS.len(), 18);
    assert_eq!(TOOLS[at], Tool::Offset);
}

#[test]
fn offset_sits_in_the_modification_group() {
    let group = TOOL_GROUPS[2];
    assert_eq!(group[0], Tool::PushPull);
    assert_eq!(group[1], Tool::Offset);
    let layout = ToolbarLayout::default();
    assert!(layout.toolbars()[2].tools.contains(&Tool::Offset));
    let total: usize = TOOL_GROUPS.iter().map(|g| g.len()).sum();
    assert_eq!(total, 18);
}

#[test]
fn i_picks_offset_and_nothing_else_moved() {
    assert_eq!(tool_shortcut(Tool::Offset), "I");
    assert_eq!(key("I", NONE), Some(Action::SelectTool(Tool::Offset)));
    let shift = Modifiers {
        shift: true,
        ..NONE
    };
    assert_eq!(key("I", shift), None);
    // Every tool's shortcut is still its own.
    let mut keys: Vec<&str> = TOOL_BUTTONS
        .iter()
        .map(|(t, _)| tool_shortcut(*t))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), 18, "shortcuts are unique");
}

#[test]
fn the_layout_file_word_round_trips() {
    assert_eq!(tool_key(Tool::Offset), "offset");
    assert_eq!(tool_from_key("offset"), Some(Tool::Offset));
}

#[test]
fn the_tooltip_names_the_key() {
    assert_eq!(tool_tooltip(Tool::Offset), "Offset (I)");
}

#[test]
fn the_icon_is_drawn_and_is_its_own() {
    assert!(icons::all().contains(&Icon::Tool(Tool::Offset)));
}

#[test]
fn clicking_the_toolbar_button_selects_the_tool() {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = toolbar(ui, Tool::Select) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("Offset").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::SelectTool(Tool::Offset)]);
}

#[test]
fn the_tools_menu_lists_offset_and_picks_it() {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState::default();
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("Tools").click();
    h.run();
    h.get_by_label_contains("Offset").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::SelectTool(Tool::Offset)]);
}
