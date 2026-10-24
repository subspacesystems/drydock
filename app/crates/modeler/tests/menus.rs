// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-ui-menus: File menu (recent files, Exit), Tools menu, About lines, driven with
//! egui_kittest. See docs/specs/m4-ui-menus.md.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::chrome::{
    CONTRIBUTORS, MenuState, TOOL_BUTTONS, TOOL_GROUPS, about_lines, about_text, menu_bar,
    recent_labels, tool_shortcut,
};
use modeler::input::{Action, Tool};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

type Seen = Rc<RefCell<Vec<Action>>>;

fn harness(state: MenuState<'static>, seen: &Seen) -> Harness<'static> {
    let seen = seen.clone();
    Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            seen.borrow_mut().push(action);
        }
    })
}

fn open(h: &mut Harness<'_>, menu: &str) {
    h.run();
    h.get_by_label(menu).click();
    h.run();
}

fn leak(paths: Vec<PathBuf>) -> &'static [PathBuf] {
    Box::leak(paths.into_boxed_slice())
}

fn recent3() -> &'static [PathBuf] {
    leak(vec![
        PathBuf::from("/models/desk.dae"),
        PathBuf::from("/models/shed.modeler"),
        PathBuf::from("/other/house.dae"),
    ])
}

fn tool_label(tool: Tool) -> String {
    let label = TOOL_BUTTONS
        .iter()
        .find(|(t, _)| *t == tool)
        .map(|(_, l)| *l)
        .unwrap();
    format!("{label}    {}", tool_shortcut(tool))
}

fn top(h: &Harness<'_>, label: &str) -> f32 {
    h.get_by_label(label).rect().top()
}

#[test]
fn menu_bar_order_is_file_edit_view_tools_help() {
    let seen = Seen::default();
    let mut h = harness(MenuState::default(), &seen);
    h.run();
    let xs: Vec<f32> = ["File", "Edit", "View", "Tools", "Help"]
        .iter()
        .map(|m| h.get_by_label(m).rect().left())
        .collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
}

#[test]
fn exit_is_the_last_file_item_and_sends_exit() {
    let seen = Seen::default();
    let mut h = harness(
        MenuState {
            recent: recent3(),
            ..MenuState::default()
        },
        &seen,
    );
    open(&mut h, "File");
    let exit = top(&h, "Exit");
    for item in [
        "New    Ctrl+N",
        "Save As...    Ctrl+Shift+S",
        "3  house.dae",
    ] {
        assert!(top(&h, item) < exit, "{item} above Exit");
    }
    h.get_by_label("Exit").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::Exit]);
}

#[test]
fn exit_is_there_without_recent_files() {
    let seen = Seen::default();
    let mut h = harness(MenuState::default(), &seen);
    open(&mut h, "File");
    assert!(h.query_by_label_contains("1  ").is_none());
    h.get_by_label("Exit").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::Exit]);
}

#[test]
fn recent_files_sit_between_save_as_and_exit_in_order() {
    let seen = Seen::default();
    let mut h = harness(
        MenuState {
            recent: recent3(),
            ..MenuState::default()
        },
        &seen,
    );
    open(&mut h, "File");
    let ys: Vec<f32> = [
        "Save As...    Ctrl+Shift+S",
        "1  desk.dae",
        "2  shed.modeler",
        "3  house.dae",
        "Exit",
    ]
    .iter()
    .map(|l| top(&h, l))
    .collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "{ys:?}");
}

#[test]
fn clicking_a_recent_file_sends_its_index() {
    for (index, label) in ["1  desk.dae", "2  shed.modeler", "3  house.dae"]
        .into_iter()
        .enumerate()
    {
        let seen = Seen::default();
        let mut h = harness(
            MenuState {
                recent: recent3(),
                ..MenuState::default()
            },
            &seen,
        );
        open(&mut h, "File");
        h.get_by_label(label).click();
        h.run();
        assert_eq!(*seen.borrow(), vec![Action::OpenRecent(index)], "{label}");
        // The menu closed.
        assert!(h.query_by_label(label).is_none(), "{label}");
    }
}

#[test]
fn file_menu_keeps_its_old_items() {
    for (item, action) in [
        ("New    Ctrl+N", Action::New),
        ("Open...    Ctrl+O", Action::Open),
        ("Save    Ctrl+S", Action::Save),
        ("Save As...    Ctrl+Shift+S", Action::SaveAs),
    ] {
        let seen = Seen::default();
        let mut h = harness(MenuState::default(), &seen);
        open(&mut h, "File");
        h.get_by_label(item).click();
        h.run();
        assert_eq!(*seen.borrow(), vec![action], "{item}");
    }
}

#[test]
fn recent_labels_number_and_name_files() {
    assert_eq!(
        recent_labels(recent3()),
        ["1  desk.dae", "2  shed.modeler", "3  house.dae"]
    );
    assert!(recent_labels(&[]).is_empty());
}

#[test]
fn recent_labels_add_the_folder_when_names_clash() {
    let paths = [
        PathBuf::from("/a/desk/model.dae"),
        PathBuf::from("/b/chair.modeler"),
        PathBuf::from("/c/house/model.dae"),
    ];
    assert_eq!(
        recent_labels(&paths),
        [
            "1  model.dae (desk)",
            "2  chair.modeler",
            "3  model.dae (house)",
        ]
    );
}

#[test]
fn recent_labels_count_past_nine() {
    let paths: Vec<PathBuf> = (0..10)
        .map(|i| PathBuf::from(format!("/m/f{i}.dae")))
        .collect();
    let labels = recent_labels(&paths);
    assert_eq!(labels[8], "9  f8.dae");
    assert_eq!(labels[9], "10  f9.dae");
}

#[test]
fn tool_groups_cover_every_tool_once_in_toolbar_order() {
    let flat: Vec<Tool> = TOOL_GROUPS.iter().flat_map(|g| g.iter().copied()).collect();
    let toolbar: Vec<Tool> = TOOL_BUTTONS.iter().map(|(t, _)| *t).collect();
    assert_eq!(flat, toolbar);
}

#[test]
fn tools_menu_lists_every_tool_with_its_key_in_order() {
    let seen = Seen::default();
    let mut h = harness(MenuState::default(), &seen);
    open(&mut h, "Tools");
    let ys: Vec<f32> = TOOL_BUTTONS
        .iter()
        .map(|(tool, _)| top(&h, &tool_label(*tool)))
        .collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "{ys:?}");
    assert!(h.query_by_label("Push/Pull    P").is_some());
    assert!(h.query_by_label("3-Point Arc    Shift+A").is_some());
    assert!(h.query_by_label("Select    Space").is_some());
}

#[test]
fn tools_menu_separates_groups() {
    // A separator adds space: the gap across a group boundary is larger than inside.
    let seen = Seen::default();
    let mut h = harness(MenuState::default(), &seen);
    open(&mut h, "Tools");
    let inside = top(&h, &tool_label(Tool::Rectangle)) - top(&h, &tool_label(Tool::Line));
    let across = top(&h, &tool_label(Tool::Line)) - top(&h, &tool_label(Tool::Select));
    assert!(across > inside + 1.0, "inside {inside}, across {across}");
    let across = top(&h, &tool_label(Tool::Orbit)) - top(&h, &tool_label(Tool::Paint));
    assert!(across > inside + 1.0, "inside {inside}, across {across}");
}

#[test]
fn clicking_each_tools_item_selects_its_tool() {
    for (tool, _) in TOOL_BUTTONS {
        let seen = Seen::default();
        let mut h = harness(MenuState::default(), &seen);
        open(&mut h, "Tools");
        let label = tool_label(tool);
        h.get_by_label(&label).click();
        h.run();
        assert_eq!(*seen.borrow(), vec![Action::SelectTool(tool)], "{label}");
        assert!(
            h.query_by_label(&label).is_none(),
            "menu closed after {label}"
        );
    }
}

#[test]
fn tools_menu_marks_only_the_current_tool() {
    for current in [Tool::Move, Tool::Select, Tool::Zoom] {
        let seen = Seen::default();
        let mut h = harness(
            MenuState {
                tool: current,
                ..MenuState::default()
            },
            &seen,
        );
        open(&mut h, "Tools");
        for (tool, _) in TOOL_BUTTONS {
            let label = tool_label(tool);
            let node = h.get_by_label(&label);
            assert_eq!(
                node.accesskit_node().toggled() == Some(eframe::egui::accesskit::Toggled::True),
                tool == current,
                "{label} with {current:?} current"
            );
        }
    }
}

#[test]
fn about_lines_name_version_license_and_contributors() {
    let lines = about_lines();
    assert_eq!(lines[0], about_text());
    assert_eq!(lines[1], "License: GPL-3.0-or-later");
    assert_eq!(lines[2], "Copyright (c) 2026 Subspace Systems");
    assert_eq!(lines[3], "Contributors:");
    let names: Vec<&str> = CONTRIBUTORS
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert!(!names.is_empty());
    assert_eq!(&lines[4..], names.as_slice());
    assert!(lines.iter().any(|l| l == "Tyrel Haveman"));
    assert!(lines.iter().all(|l| !l.starts_with('#') && !l.is_empty()));
}

#[test]
fn about_window_shows_every_line() {
    let mut h = Harness::new_ui(|ui| {
        let mut open = true;
        modeler::chrome::about_window(ui.ctx(), &mut open);
    });
    h.run();
    for line in about_lines() {
        assert!(h.query_by_label(&line).is_some(), "{line}");
    }
    assert!(h.query_by_label("Close").is_some());
}

#[test]
fn help_menu_still_opens_about() {
    let seen = Seen::default();
    let mut h = harness(MenuState::default(), &seen);
    open(&mut h, "Help");
    h.get_by_label("About modeler").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::About]);
}
