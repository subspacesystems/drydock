// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m9-followme-tool: the Tools > Follow Me menu item and the tool's wiring
//! (`docs/specs/m9-followme-tool.md`).

use std::cell::RefCell;
use std::rc::Rc;

use drydock::chrome::{MenuState, TOOL_BUTTONS, TOOL_GROUPS, menu_bar, tool_shortcut, toolbar};
use drydock::icons::TOOLS;
use drydock::input::{Action, Modifiers, Tool, key};
use drydock::toolbars::{ToolbarLayout, tool_from_key, tool_key};
use drydock::viewstate::ViewState;
use eframe::egui::Key;
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
fn it_has_a_toolbar_button_right_after_offset() {
    let at = TOOL_BUTTONS
        .iter()
        .position(|(tool, _)| *tool == Tool::FollowMe)
        .expect("Follow Me button");
    assert_eq!(TOOL_BUTTONS[at].1, "Follow Me");
    assert_eq!(TOOL_BUTTONS[at - 1].0, Tool::Offset);
    assert_eq!(
        TOOLS[at],
        Tool::FollowMe,
        "icons::TOOLS follows the buttons"
    );
    // The modify group (and so the default toolbar) holds it after Offset.
    let group = TOOL_GROUPS[2];
    let offset = group.iter().position(|t| *t == Tool::Offset).unwrap();
    assert_eq!(group[offset + 1], Tool::FollowMe);
    assert!(
        ToolbarLayout::default().toolbars()[2]
            .tools
            .contains(&Tool::FollowMe)
    );
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
    h.get_by_label("Follow Me").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::SelectTool(Tool::FollowMe)]);
}

#[test]
fn the_f_key_picks_it_and_shift_f_stays_unbound() {
    assert_eq!(tool_shortcut(Tool::FollowMe), "F");
    let none = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
    };
    assert_eq!(
        key(Key::F.name(), none),
        Some(Action::SelectTool(Tool::FollowMe))
    );
    for mods in [
        Modifiers {
            shift: true,
            ..none
        },
        Modifiers { ctrl: true, ..none },
        Modifiers { alt: true, ..none },
    ] {
        assert_eq!(key("F", mods), None, "{mods:?}");
    }
}

#[test]
fn the_tools_menu_shows_the_icon_row_with_its_key_once() {
    let (h, _) = tools_harness(Tool::Select);
    assert!(h.query_by_label("F").is_some(), "shortcut shown");
    assert_eq!(
        h.query_all_by_label("Follow Me").count(),
        1,
        "one Follow Me item, in the tool list"
    );
}

#[test]
fn a_toolbar_layout_saved_before_the_button_gets_it_after_offset() {
    let old = "modeler toolbars 1
toolbar 1 top 0 Modify
tools pushpull offset move rotate scale
toolbar 1 top 1 Mine
tools zoom
";
    let layout = ToolbarLayout::parse(old);
    let tools = &layout.toolbars()[0].tools;
    assert_eq!(
        *tools,
        vec![
            Tool::PushPull,
            Tool::Offset,
            Tool::FollowMe,
            Tool::Move,
            Tool::Rotate,
            Tool::Scale
        ]
    );
    assert_eq!(layout.toolbars()[1].tools, vec![Tool::Zoom]);
    // A layout that already has it, or has no Offset, is left as written.
    let kept = "modeler toolbars 1
toolbar 1 top 0 Modify
tools followme pushpull offset
";
    assert_eq!(
        ToolbarLayout::parse(kept).toolbars()[0].tools,
        vec![Tool::FollowMe, Tool::PushPull, Tool::Offset]
    );
    let no_offset = "modeler toolbars 1
toolbar 1 top 0 Drawing
tools line
";
    assert_eq!(ToolbarLayout::parse(no_offset).to_text(), no_offset);
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
