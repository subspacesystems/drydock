// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-menu-fit: menus open below the menu bar, are capped to the window and scroll.
//! See docs/specs/m10-menu-fit.md.

use drydock::chrome::{MenuState, menu_bar, menu_bar_with_toolbars};
use drydock::input::{Action, Tool};
use drydock::look;
use drydock::toolbars::ToolbarLayout;
use eframe::egui::accesskit::Role;
use eframe::egui::{self, Rect};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use std::cell::RefCell;
use std::rc::Rc;

type Seen = Rc<RefCell<Vec<Action>>>;

/// Room the menu leaves below itself (the spec's "small margin" is at most this).
const MARGIN: f32 = 24.0;

fn menus(w: f32, h: f32, seen: &Seen, toolbars: bool) -> Harness<'static> {
    let seen = seen.clone();
    let layout = Rc::new(RefCell::new(ToolbarLayout::default()));
    let state = MenuState::default();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(w, h))
        .build_ui(move |ui| {
            let action = if toolbars {
                menu_bar_with_toolbars(ui, &state, &mut layout.borrow_mut()).action
            } else {
                menu_bar(ui, &state)
            };
            if let Some(action) = action {
                seen.borrow_mut().push(action);
            }
        });
    harness.ctx.set_fonts(look::font_definitions());
    harness.run();
    harness
}

fn open(h: &mut Harness<'_>, menu: &str) -> f32 {
    let bar_bottom = h.get_by_label(menu).rect().bottom();
    h.get_by_label(menu).click();
    h.run();
    h.run();
    bar_bottom
}

fn rows(h: &Harness<'_>, bar_bottom: f32) -> Vec<(String, Rect)> {
    let mut rows: Vec<(String, Rect)> = h
        .query_all_by_role(Role::Button)
        .chain(h.query_all_by_role(Role::CheckBox))
        .filter(|n| n.rect().top() >= bar_bottom - 0.5 || n.rect().bottom() > bar_bottom + 0.5)
        .filter(|n| n.accesskit_node().label().is_some())
        .map(|n| (n.accesskit_node().label().unwrap().to_string(), n.rect()))
        .collect();
    rows.retain(|(l, _)| !["File", "Edit", "View", "Tools", "Help"].contains(&l.as_str()));
    rows
}

fn row(h: &Harness<'_>, label: &str) -> Rect {
    h.query_all_by_role(Role::Button)
        .chain(h.query_all_by_role(Role::CheckBox))
        .find(|n| n.accesskit_node().label().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("row {label:?}"))
        .rect()
}

fn wheel_down(h: &mut Harness<'static>, over: &str, notches: usize) {
    h.get_by_label(over).hover();
    h.step();
    for _ in 0..notches {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    h.run_steps(30);
}

#[test]
fn a_tall_menu_in_a_short_window_stays_below_the_menu_bar() {
    let mut h = menus(1100.0, 420.0, &Seen::default(), false);
    let bar_bottom = open(&mut h, "Tools");
    let first = row(&h, "Select");
    assert!(
        first.top() >= bar_bottom - 0.5,
        "Tools starts at {} above the menu bar's bottom {bar_bottom}",
        first.top()
    );
    // Titles of the other menus are still where they were (nothing covers them).
    assert!(
        first.top() < bar_bottom + 40.0,
        "menu opens right under its title"
    );
}

#[test]
fn scrolling_the_tools_menu_reaches_and_clicks_its_last_item() {
    let seen = Seen::default();
    let mut h = menus(1100.0, 420.0, &seen, false);
    let bar_bottom = open(&mut h, "Tools");
    wheel_down(&mut h, "Select", 12);
    let last = row(&h, "Print Setup...");
    assert!(
        last.bottom() <= 420.0 + 0.5,
        "last row {last:?} is below the window"
    );
    assert!(
        last.top() >= bar_bottom - 0.5,
        "last row {last:?} under the bar"
    );
    assert!(
        last.bottom() <= 420.0 - 1.0 && 420.0 - last.bottom() <= MARGIN + 40.0,
        "menu leaves only a small margin: {last:?}"
    );
    h.get_by_label("Print Setup...").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::PrintSetup]);
}

#[test]
fn scrolling_does_not_move_the_menu_itself() {
    let mut h = menus(1100.0, 420.0, &Seen::default(), false);
    let bar_bottom = open(&mut h, "Tools");
    let before = row(&h, "Select");
    wheel_down(&mut h, "Select", 12);
    let after = row(&h, "Print Setup...");
    // The scrolled rows moved up inside a menu that stays put: the last row now ends within
    // the window while the title bar's buttons are untouched.
    assert!(row(&h, "Tools").top() < bar_bottom);
    assert!(after.bottom() <= 420.0 + 0.5);
    assert!(before.left() <= after.left() + 0.5 && after.left() <= before.left() + 0.5);
}

#[test]
fn a_menu_that_fits_has_no_scroll_and_looks_like_in_a_tall_window() {
    for menu in ["File", "Edit", "Help"] {
        let mut short = menus(1100.0, 420.0, &Seen::default(), false);
        let bb = open(&mut short, menu);
        let mut tall = menus(1100.0, 1600.0, &Seen::default(), false);
        let tb = open(&mut tall, menu);
        let (a, b) = (rows(&short, bb), rows(&tall, tb));
        assert_eq!(a.len(), b.len(), "{menu}");
        for ((la, ra), (lb, rb)) in a.iter().zip(&b) {
            assert_eq!(la, lb);
            assert!(
                (ra.min - rb.min).abs().max_elem() < 0.5
                    && (ra.size() - rb.size()).abs().max_elem() < 0.5,
                "{menu}/{la}: {ra:?} vs {rb:?}"
            );
        }
        // Scrolling a menu that fits changes nothing.
        let first = a[0].0.clone();
        wheel_down(&mut short, &first, 6);
        assert_eq!(row(&short, &first), a[0].1, "{menu} moved when scrolled");
    }
}

#[test]
fn in_a_normal_window_every_menu_looks_as_before() {
    for menu in ["File", "Edit", "View", "Tools", "Help"] {
        let mut normal = menus(1280.0, 800.0, &Seen::default(), false);
        let nb = open(&mut normal, menu);
        let mut tall = menus(1280.0, 1600.0, &Seen::default(), false);
        let tb = open(&mut tall, menu);
        let (a, b) = (rows(&normal, nb), rows(&tall, tb));
        assert_eq!(a.len(), b.len(), "{menu}");
        for ((la, ra), (lb, rb)) in a.iter().zip(&b) {
            assert_eq!(la, lb);
            assert!(
                (ra.min - rb.min).abs().max_elem() < 0.5
                    && (ra.size() - rb.size()).abs().max_elem() < 0.5,
                "{menu}/{la}: {ra:?} vs {rb:?}"
            );
            assert!(ra.bottom() <= 800.0, "{menu}/{la} below the window");
        }
    }
}

#[test]
fn the_view_menu_scrolls_in_a_short_window_and_its_submenu_stays_below_the_bar() {
    let seen = Seen::default();
    let mut h = menus(1100.0, 300.0, &seen, true);
    let bar_bottom = open(&mut h, "View");
    let first = rows(&h, bar_bottom)[0].1;
    assert!(
        first.top() >= bar_bottom - 0.5,
        "View covers the bar: {first:?}"
    );
    let first_label = rows(&h, bar_bottom)[0].0.clone();
    wheel_down(&mut h, &first_label, 12);
    let toolbars = row(&h, "Toolbars");
    assert!(toolbars.top() >= bar_bottom - 0.5 && toolbars.bottom() <= 300.0);
    h.get_by_label("Toolbars").click();
    h.run();
    h.run();
    let sub = rows(&h, bar_bottom)
        .into_iter()
        .find(|(l, _)| l == "Customize...")
        .expect("submenu open")
        .1;
    assert!(
        sub.top() >= bar_bottom - 0.5,
        "submenu above the bar: {sub:?}"
    );
    let reset = row(&h, "Reset Toolbars...");
    // Its last row can be scrolled into the window.
    wheel_down(&mut h, "Customize...", 12);
    let reset_after = row(&h, "Reset Toolbars...");
    assert!(
        reset_after.bottom() <= 300.0 + 0.5,
        "{reset:?} -> {reset_after:?}"
    );
}

#[test]
fn the_tool_rows_still_select_their_tool() {
    let seen = Seen::default();
    let mut h = menus(1100.0, 420.0, &seen, false);
    open(&mut h, "Tools");
    h.get_by_label("Select").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::SelectTool(Tool::Select)]);
}
