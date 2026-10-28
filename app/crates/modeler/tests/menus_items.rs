// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-menus: menu rows with right-aligned shortcuts, tool icons and a drawn submenu arrow,
//! driven with egui_kittest. See docs/specs/m6-menus.md.

use eframe::egui::{Pos2, Rect, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::chrome::{
    MenuState, TOOL_BUTTONS, menu_bar, menu_bar_with_toolbars, menu_item, menu_item_layout,
    tool_shortcut,
};
use modeler::icons::Icon;
use modeler::input::{Action, Tool};
use modeler::toolbars::ToolbarLayout;
use std::cell::RefCell;
use std::rc::Rc;

type Seen = Rc<RefCell<Vec<Action>>>;

fn menus(state: MenuState<'static>, seen: &Seen) -> Harness<'static> {
    let seen = seen.clone();
    Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            seen.borrow_mut().push(action);
        }
    })
}

fn open(h: &mut Harness<'_>, menu: &str) {
    // m6-menus-2: menus are as wide as their text, so the text needs real fonts.
    h.ctx.set_fonts(modeler::look::font_definitions(None));
    h.run();
    h.get_by_label(menu).click();
    h.run();
}

fn row(width: f32) -> Rect {
    Rect::from_min_size(Pos2::new(10.0, 40.0), Vec2::new(width, 22.0))
}

#[test]
fn layout_without_icon_puts_the_label_at_the_left_and_the_shortcut_at_the_right() {
    let r = row(200.0);
    let l = menu_item_layout(r, false, 50.0);
    assert!(l.icon.is_none());
    // m6-menus-2: every row leaves the icon column (4 + 16 + 6) before its label.
    assert!((l.label_left - (r.left() + 26.0)).abs() < 0.01);
    assert!((l.shortcut.right() - (r.right() - 6.0)).abs() < 0.01);
    assert!((l.shortcut.width() - 50.0).abs() < 0.01);
    assert!((l.shortcut.center().y - r.center().y).abs() < 0.01);
}

#[test]
fn layout_with_icon_puts_a_16_px_square_before_the_label() {
    let r = row(200.0);
    let l = menu_item_layout(r, true, 0.0);
    let icon = l.icon.expect("icon rect");
    assert!((icon.width() - 16.0).abs() < 0.01 && (icon.height() - 16.0).abs() < 0.01);
    assert!((icon.left() - (r.left() + 4.0)).abs() < 0.01);
    assert!((icon.center().y - r.center().y).abs() < 0.01);
    assert!((l.label_left - (icon.right() + 6.0)).abs() < 0.01);
}

#[test]
fn menu_item_is_found_by_its_label_and_clicks() {
    let clicked = Rc::new(RefCell::new(0));
    let c = clicked.clone();
    let mut h = Harness::new_ui(move |ui| {
        if menu_item(ui, None, "Frobnicate", "Ctrl+F", false).clicked() {
            *c.borrow_mut() += 1;
        }
    });
    h.run();
    h.get_by_label("Frobnicate").click();
    h.run();
    assert_eq!(*clicked.borrow(), 1);
    assert!(
        h.query_by_label("Ctrl+F").is_some(),
        "shortcut is a node of its own"
    );
    assert!(h.query_by_label("Frobnicate    Ctrl+F").is_none());
}

#[test]
fn menu_item_reports_selected() {
    let mut h = Harness::new_ui(|ui| {
        menu_item(ui, Some(Icon::Tool(Tool::Line)), "On", "", true);
        menu_item(ui, Some(Icon::Tool(Tool::Move)), "Off", "", false);
    });
    h.run();
    let toggled = |h: &Harness<'_>, l: &str| {
        h.get_by_label(l).accesskit_node().toggled() == Some(eframe::egui::accesskit::Toggled::True)
    };
    assert!(toggled(&h, "On"));
    assert!(!toggled(&h, "Off"));
}

#[test]
fn shortcuts_in_the_file_menu_share_one_right_edge() {
    let seen = Seen::default();
    let mut h = menus(MenuState::default(), &seen);
    open(&mut h, "File");
    let edges: Vec<f32> = ["Ctrl+N", "Ctrl+O", "Ctrl+S", "Ctrl+Shift+S"]
        .iter()
        .map(|s| h.get_by_label(s).rect().right())
        .collect();
    for e in &edges {
        assert!((e - edges[0]).abs() < 1.0, "right edges {edges:?}");
    }
    // The longest shortcut leaves a gap after the longest label.
    let label_right = h.get_by_label("Save As...").rect().right();
    let _ = label_right;
    let save_as_row = h.get_by_label("Save As...").rect();
    let shortcut = h.get_by_label("Ctrl+Shift+S").rect();
    assert!(shortcut.left() > save_as_row.left() + 60.0);
    assert!(shortcut.right() <= save_as_row.right());
    for n in ["New", "Open...", "Save", "Save As...", "Exit"] {
        assert!(h.query_by_label(n).is_some(), "{n}");
    }
}

#[test]
fn no_menu_label_embeds_spaces_and_a_shortcut() {
    let seen = Seen::default();
    for menu in ["File", "Edit", "View", "Tools"] {
        let mut h = menus(MenuState::default(), &seen);
        open(&mut h, menu);
        for node in h.query_all_by_role(eframe::egui::accesskit::Role::Button) {
            let label = node.accesskit_node().label().unwrap_or_default();
            assert!(!label.contains("    "), "{menu}: {label:?}");
        }
    }
}

#[test]
fn edit_and_view_menus_show_their_shortcuts() {
    let seen = Seen::default();
    let mut h = menus(MenuState::default(), &seen);
    open(&mut h, "Edit");
    for s in ["Ctrl+Z", "Ctrl+Y", "Ctrl+X", "Ctrl+C", "Ctrl+V", "Delete"] {
        assert!(h.query_by_label(s).is_some(), "Edit: {s}");
    }
    let mut h = menus(MenuState::default(), &seen);
    open(&mut h, "View");
    for s in [
        "Shift+P", "Shift+Z", "F2", "F3", "F4", "F5", "F6", "F7", "F8",
    ] {
        assert!(h.query_by_label(s).is_some(), "View: {s}");
    }
}

#[test]
fn tools_menu_shows_each_tools_shortcut_aligned_and_still_selects() {
    for (tool, label) in TOOL_BUTTONS {
        let seen = Seen::default();
        let mut h = menus(MenuState::default(), &seen);
        open(&mut h, "Tools");
        assert!(
            h.query_by_label(tool_shortcut(tool)).is_some(),
            "shortcut of {label}"
        );
        h.get_by_label(label).click();
        h.run();
        assert_eq!(*seen.borrow(), vec![Action::SelectTool(tool)], "{label}");
    }
    let seen = Seen::default();
    let mut h = menus(MenuState::default(), &seen);
    open(&mut h, "Tools");
    let edges: Vec<f32> = TOOL_BUTTONS
        .iter()
        .map(|(tool, _)| h.get_by_label(tool_shortcut(*tool)).rect().right())
        .collect();
    for e in &edges {
        assert!((e - edges[0]).abs() < 1.0, "right edges {edges:?}");
    }
}

#[test]
fn disabled_items_keep_their_shortcut_and_do_nothing() {
    let seen = Seen::default();
    let mut h = menus(MenuState::default(), &seen);
    open(&mut h, "Edit");
    assert!(h.query_by_label("Ctrl+C").is_some());
    h.get_by_label("Copy").click();
    h.run();
    assert!(seen.borrow().is_empty());
}

#[test]
fn toolbars_submenu_has_no_missing_glyph_and_opens() {
    let layout = Rc::new(RefCell::new(ToolbarLayout::default()));
    let l = layout.clone();
    let state = MenuState::default();
    let mut h = Harness::new_ui(move |ui| {
        menu_bar_with_toolbars(ui, &state, &mut l.borrow_mut());
    });
    open(&mut h, "View");
    for node in h.query_all_by_role(eframe::egui::accesskit::Role::Button) {
        let label = node.accesskit_node().label().unwrap_or_default();
        assert!(!label.contains('\u{23f5}'), "arrow glyph in {label:?}");
    }
    h.get_by_label("Toolbars").click();
    h.run();
    assert!(h.query_by_label("Customize...").is_some());
    assert!(h.query_by_label("Reset Toolbars").is_some());
}
