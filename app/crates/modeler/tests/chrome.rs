// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m3-app-ui: toolbar and menus, driven with egui_kittest. See docs/specs/m3-app-ui.md.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::chrome::{MenuState, TOOL_BUTTONS, menu_bar, toolbar};
use modeler::input::{Action, EditKey, Tool};
use modeler_view::StandardView;
use std::cell::RefCell;
use std::rc::Rc;

type Seen = Rc<RefCell<Vec<Action>>>;

fn toolbar_harness(current: Tool, seen: &Seen) -> Harness<'static> {
    let seen = seen.clone();
    Harness::new_ui(move |ui| {
        if let Some(action) = toolbar(ui, current) {
            seen.borrow_mut().push(action);
        }
    })
}

fn menu_harness(state: MenuState<'static>, seen: &Seen) -> Harness<'static> {
    let seen = seen.clone();
    Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            seen.borrow_mut().push(action);
        }
    })
}

fn state() -> MenuState<'static> {
    MenuState {
        undo: Some("Line"),
        redo: Some("Move"),
        parallel: false,
        has_selection: true,
        can_explode: false,
        hide_rest: false,
        ..MenuState::default()
    }
}

fn choose(h: &mut Harness<'_>, menu: &str, item: &str) {
    h.get_by_label(menu).click();
    h.run();
    if let Some(exact) = h.query_by_label(item) {
        exact.click();
    } else {
        h.get_by_label_contains(item).click();
    }
    h.run();
}

#[test]
fn button_table_lists_every_tool_once() {
    let tools: Vec<Tool> = TOOL_BUTTONS.iter().map(|(t, _)| *t).collect();
    for tool in [
        Tool::Select,
        Tool::Line,
        Tool::Rectangle,
        Tool::Circle,
        Tool::Arc,
        Tool::Arc3,
        Tool::PushPull,
        Tool::Move,
        Tool::Rotate,
        Tool::Scale,
        Tool::Tape,
        Tool::Orbit,
        Tool::Pan,
        Tool::Zoom,
    ] {
        assert_eq!(tools.iter().filter(|t| **t == tool).count(), 1, "{tool:?}");
    }
    let labels: Vec<&str> = TOOL_BUTTONS.iter().map(|(_, l)| *l).collect();
    for label in ["Push/Pull", "Move", "Rotate", "Scale", "Tape Measure"] {
        assert!(labels.contains(&label), "{label}");
    }
}

#[test]
fn clicking_each_toolbar_button_selects_its_tool() {
    for (tool, label) in TOOL_BUTTONS {
        let seen = Seen::default();
        let mut h = toolbar_harness(Tool::Orbit, &seen);
        h.run();
        h.get_by_label(label).click();
        h.run();
        assert_eq!(*seen.borrow(), vec![Action::SelectTool(tool)], "{label}");
    }
}

#[test]
fn toolbar_without_clicks_chooses_nothing() {
    let seen = Seen::default();
    let mut h = toolbar_harness(Tool::PushPull, &seen);
    h.run();
    h.run();
    assert!(seen.borrow().is_empty());
}

#[test]
fn toolbar_marks_only_the_current_tool_selected() {
    let seen = Seen::default();
    let mut h = toolbar_harness(Tool::Rotate, &seen);
    h.run();
    for (tool, label) in TOOL_BUTTONS {
        let node = h.get_by_label(label);
        assert_eq!(
            node.accesskit_node().toggled() == Some(eframe::egui::accesskit::Toggled::True),
            tool == Tool::Rotate,
            "{label}"
        );
    }
}

#[test]
fn flip_items_send_flip_for_each_axis() {
    for (axis, item) in ["Flip Along Red", "Flip Along Green", "Flip Along Blue"]
        .into_iter()
        .enumerate()
    {
        let seen = Seen::default();
        let mut h = menu_harness(state(), &seen);
        h.run();
        choose(&mut h, "Edit", item);
        assert_eq!(*seen.borrow(), vec![Action::Flip(axis)], "{item}");
    }
}

#[test]
fn flip_items_are_disabled_without_a_selection() {
    let seen = Seen::default();
    let mut h = menu_harness(
        MenuState {
            has_selection: false,
            ..state()
        },
        &seen,
    );
    h.run();
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label_contains("Flip Along Red").click();
    h.run();
    assert!(seen.borrow().is_empty());
}

#[test]
fn undo_and_redo_items_carry_the_step_labels() {
    let seen = Seen::default();
    let mut h = menu_harness(state(), &seen);
    h.run();
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label_contains("Undo Line");
    h.get_by_label_contains("Redo Move");
}

#[test]
fn undo_and_redo_send_edit_keys_and_are_disabled_when_empty() {
    let seen = Seen::default();
    let mut h = menu_harness(state(), &seen);
    h.run();
    choose(&mut h, "Edit", "Undo Line");
    choose(&mut h, "Edit", "Redo Move");
    assert_eq!(
        *seen.borrow(),
        vec![Action::Edit(EditKey::Undo), Action::Edit(EditKey::Redo)]
    );
    let seen = Seen::default();
    let mut h = menu_harness(
        MenuState {
            undo: None,
            redo: None,
            ..state()
        },
        &seen,
    );
    h.run();
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label_contains("Undo").click();
    h.run();
    assert!(seen.borrow().is_empty());
}

#[test]
fn file_menu_items_send_their_actions() {
    for (item, action) in [
        ("New", Action::New),
        ("Open...", Action::Open),
        ("Save", Action::Save),
        ("Save As...", Action::SaveAs),
    ] {
        let seen = Seen::default();
        let mut h = menu_harness(state(), &seen);
        h.run();
        choose(&mut h, "File", item);
        assert_eq!(*seen.borrow(), vec![action], "{item}");
    }
}

#[test]
fn view_menu_toggles_projection_and_names_the_other_mode() {
    for (parallel, item) in [(false, "Parallel"), (true, "Perspective")] {
        let seen = Seen::default();
        let mut h = menu_harness(
            MenuState {
                parallel,
                ..state()
            },
            &seen,
        );
        h.run();
        choose(&mut h, "View", item);
        assert_eq!(*seen.borrow(), vec![Action::ToggleProjection], "{item}");
    }
}

#[test]
fn view_menu_zoom_extents_and_standard_views() {
    let seen = Seen::default();
    let mut h = menu_harness(state(), &seen);
    h.run();
    choose(&mut h, "View", "Zoom Extents");
    assert_eq!(*seen.borrow(), vec![Action::ZoomExtents]);
    for (item, view) in [
        ("Iso", StandardView::Iso),
        ("Top", StandardView::Top),
        ("Front", StandardView::Front),
        ("Right", StandardView::Right),
        ("Back", StandardView::Back),
        ("Left", StandardView::Left),
        ("Bottom", StandardView::Bottom),
    ] {
        let seen = Seen::default();
        let mut h = menu_harness(state(), &seen);
        h.run();
        choose(&mut h, "View", item);
        assert_eq!(*seen.borrow(), vec![Action::View(view)], "{item}");
    }
}

#[test]
fn menus_choose_nothing_until_clicked() {
    let seen = Seen::default();
    let mut h = menu_harness(state(), &seen);
    h.run();
    h.run();
    assert!(seen.borrow().is_empty());
}

#[test]
fn help_about_returns_the_about_action() {
    let seen = Seen::default();
    let mut h = menu_harness(state(), &seen);
    h.run();
    choose(&mut h, "Help", "About modeler");
    assert_eq!(*seen.borrow(), vec![Action::About]);
}

#[test]
fn about_text_names_the_version_and_commit() {
    use modeler::chrome::{GIT_HASH, about_text};
    let text = about_text();
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
    assert!(!GIT_HASH.is_empty());
    assert!(text.contains(GIT_HASH), "{text}");
}

#[test]
fn about_window_shows_the_text_and_closes() {
    use modeler::chrome::{about_text, about_window};
    let open = Rc::new(RefCell::new(true));
    let flag = open.clone();
    let mut h = Harness::new_ui(move |ui| {
        let mut o = *flag.borrow();
        about_window(ui.ctx(), &mut o);
        *flag.borrow_mut() = o;
    });
    h.run();
    h.get_by_label(&about_text());
    h.get_by_label("Close").click();
    h.run();
    assert!(!*open.borrow());
}
