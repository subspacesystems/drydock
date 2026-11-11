// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-wayland-popup-order: on Wayland a pop-up is never destroyed before the pop-up that
//! depends on it, and a menu's pop-up is gone before the next menu's is created
//! (`docs/specs/m12-wayland-popup-order.md`). The test drives the real viewport path with no
//! window: `embed_viewports(false)`, an immediate-viewport renderer that runs each pop-up's
//! callback, and a record, per frame, of the pop-up viewports in `FullOutput::viewport_output`
//! (eframe creates a window when an id first appears there and destroys it when it is gone).

use drydock::chrome::{
    MenuState, menu_bar_with_toolbars, menu_popup_title, menu_title_rect, request_menu,
    request_menu_step, request_submenu, submenu_popup_title,
};
use drydock::popup::{force_wayland_windows, popup_viewport_id, wayland_session};
use drydock::toolbars::ToolbarLayout;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, Rect, ViewportId, pos2, vec2};
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

struct Rig {
    ctx: egui::Context,
    layout: ToolbarLayout,
    time: f64,
    focused: bool,
    escape_in: Rc<Cell<Option<ViewportId>>>,
    log: Vec<HashSet<ViewportId>>,
}

fn menu(label: &str) -> ViewportId {
    popup_viewport_id(&menu_popup_title(label))
}

fn sub(label: &str) -> ViewportId {
    popup_viewport_id(&submenu_popup_title(label))
}

impl Rig {
    fn new(wayland: bool) -> Self {
        let escape_in: Rc<Cell<Option<ViewportId>>> = Rc::default();
        let target = escape_in.clone();
        egui::Context::set_immediate_viewport_renderer(move |ctx, viewport| {
            // As eframe does: the pop-up's callback runs as a pass of its own viewport on the
            // same context. Escape is sent to one of them on request.
            let mut raw = egui::RawInput {
                viewport_id: viewport.ids.this,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 300.0))),
                ..Default::default()
            };
            raw.viewports.insert(
                viewport.ids.this,
                egui::ViewportInfo {
                    focused: None,
                    ..Default::default()
                },
            );
            if target.get() == Some(viewport.ids.this) {
                raw.events.push(Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
            }
            let mut cb = viewport.viewport_ui_cb;
            let mut out = ctx.run_ui(raw, |ui| cb(ui));
            out.textures_delta.clear();
        });
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        ctx.set_fonts(drydock::look::font_definitions());
        force_wayland_windows(&ctx, wayland);
        let mut rig = Rig {
            ctx,
            layout: ToolbarLayout::default(),
            time: 0.0,
            focused: true,
            escape_in,
            log: Vec::new(),
        };
        rig.step(vec![]);
        rig.step(vec![]);
        rig
    }

    /// One frame of the main window with `events`; records the pop-up viewports it outputs.
    fn step(&mut self, events: Vec<Event>) {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(700.0, 500.0));
        let mut raw = egui::RawInput {
            screen_rect: Some(screen),
            time: Some(self.time),
            events,
            focused: self.focused,
            ..Default::default()
        };
        raw.viewports.insert(
            ViewportId::ROOT,
            egui::ViewportInfo {
                focused: Some(self.focused),
                inner_rect: Some(screen),
                ..Default::default()
            },
        );
        self.time += 0.1;
        let state = MenuState::default();
        let layout = &mut self.layout;
        let mut out = self.ctx.run_ui(raw, |ui| {
            menu_bar_with_toolbars(ui, &state, layout);
        });
        out.textures_delta.clear();
        self.escape_in.set(None);
        let ids = out
            .viewport_output
            .keys()
            .copied()
            .filter(|id| *id != ViewportId::ROOT)
            .collect();
        self.log.push(ids);
    }

    fn run(&mut self, frames: usize) {
        for _ in 0..frames {
            self.step(vec![]);
        }
    }

    fn has(&self, frame: usize, id: ViewportId) -> bool {
        self.log[frame].contains(&id)
    }

    /// View > Toolbars open, both pop-ups in the last frame's output.
    fn view_with_toolbars(wayland: bool) -> Self {
        let mut rig = Rig::new(wayland);
        request_menu(&rig.ctx, "View");
        rig.run(4);
        request_submenu(&rig.ctx, "Toolbars");
        rig.run(4);
        let last = rig.log.len() - 1;
        assert!(rig.has(last, menu("View")), "View pop-up is up");
        assert!(rig.has(last, sub("Toolbars")), "Toolbars pop-up is up");
        rig
    }

    fn open_file(wayland: bool) -> Self {
        let mut rig = Rig::new(wayland);
        request_menu(&rig.ctx, "File");
        rig.run(4);
        assert!(rig.has(rig.log.len() - 1, menu("File")));
        rig
    }

    fn title_center(&self, label: &str) -> Pos2 {
        menu_title_rect(&self.ctx, label)
            .unwrap_or_else(|| panic!("no title rect for {label}"))
            .center()
    }

    /// Moves the pointer onto the title of `label` (two moves, so it has a delta), and keeps it
    /// there on later frames.
    fn hover(&mut self, label: &str) {
        let at = self.title_center(label);
        self.step(vec![Event::PointerMoved(at - vec2(3.0, 0.0))]);
        self.step(vec![Event::PointerMoved(at)]);
    }
}

fn click(at: Pos2) -> Vec<Event> {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    vec![Event::PointerMoved(at), button(true), button(false)]
}

fn escape() -> Event {
    Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// Frame indices (from `from`) where `id` is first missing, within 30 frames of stepping.
fn first_missing(rig: &mut Rig, from: usize, id: ViewportId) -> Option<usize> {
    for _ in 0..30 {
        if let Some(at) = (from..rig.log.len()).find(|f| !rig.has(*f, id)) {
            return Some(at);
        }
        rig.step(vec![]);
    }
    None
}

/// Checks the rule for a close that started at frame `from`: a frame without the submenu's
/// pop-up but with the menu's comes before the frame without the menu's.
fn assert_child_first(rig: &mut Rig, from: usize, why: &str) {
    let menu_gone =
        first_missing(rig, from, menu("View")).unwrap_or_else(|| panic!("{why}: never closed"));
    let sub_gone = first_missing(rig, from, sub("Toolbars"))
        .unwrap_or_else(|| panic!("{why}: sub never closed"));
    assert!(
        sub_gone < menu_gone,
        "{why}: submenu gone at frame {sub_gone}, menu at {menu_gone}: {:?}",
        rig.log
            .iter()
            .map(|f| (f.contains(&menu("View")), f.contains(&sub("Toolbars"))))
            .collect::<Vec<_>>()
    );
    assert!(
        rig.has(sub_gone, menu("View")),
        "{why}: menu still up while the submenu goes"
    );
}

// ---- (a) closing a menu with its submenu shown ------------------------------------------

/// Not a close of the menu: the first Escape in the main window closes only the submenu (already
/// the rule), the second the menu.
#[test]
fn escape_in_the_main_window_closes_the_submenu_then_the_menu() {
    let mut rig = Rig::view_with_toolbars(true);
    let from = rig.log.len();
    rig.step(vec![escape()]);
    rig.run(3);
    let last = rig.log.len() - 1;
    assert!(!rig.has(last, sub("Toolbars")), "submenu closed");
    assert!(rig.has(last, menu("View")), "menu stays");
    let from2 = rig.log.len();
    rig.step(vec![escape()]);
    rig.run(3);
    assert!(
        !rig.has(rig.log.len() - 1, menu("View")),
        "menu closed {from} {from2}"
    );
}

#[test]
fn escape_in_the_submenu_pop_up() {
    let mut rig = Rig::view_with_toolbars(true);
    let from = rig.log.len();
    rig.escape_in.set(Some(sub("Toolbars")));
    rig.step(vec![]);
    assert_child_first(&mut rig, from, "Escape in the submenu");
}

#[test]
fn a_click_in_the_main_window() {
    let mut rig = Rig::view_with_toolbars(true);
    let from = rig.log.len();
    rig.step(click(pos2(400.0, 450.0)));
    assert_child_first(&mut rig, from, "click in main");
}

#[test]
fn losing_focus() {
    let mut rig = Rig::view_with_toolbars(true);
    let from = rig.log.len();
    rig.focused = false;
    rig.step(vec![Event::WindowFocused(false)]);
    assert_child_first(&mut rig, from, "focus loss");
}

#[test]
fn hovering_another_title() {
    let mut rig = Rig::view_with_toolbars(true);
    let from = rig.log.len();
    rig.hover("Edit");
    assert_child_first(&mut rig, from, "hover to Edit");
}

// ---- (b) switching menus: nothing between the old pop-up and the new -------------------

/// After the switch started, some frame has neither File's nor Edit's pop-up between the last
/// frame with File's and the first with Edit's.
fn assert_gap(rig: &mut Rig, from: usize, why: &str) {
    for _ in 0..20 {
        rig.step(vec![]);
    }
    let edit = (from..rig.log.len())
        .find(|f| rig.has(*f, menu("Edit")))
        .unwrap_or_else(|| panic!("{why}: Edit never opened"));
    let file = (from..edit)
        .rev()
        .find(|f| rig.has(*f, menu("File")))
        .unwrap_or(from.saturating_sub(1));
    assert!(
        (file + 1..edit).any(|f| !rig.has(f, menu("File")) && !rig.has(f, menu("Edit"))),
        "{why}: File last at {file}, Edit first at {edit}: {:?}",
        rig.log
            .iter()
            .map(|f| (f.contains(&menu("File")), f.contains(&menu("Edit"))))
            .collect::<Vec<_>>()
    );
    assert!(
        rig.has(rig.log.len() - 1, menu("Edit")),
        "{why}: Edit stays open"
    );
    assert!(
        !rig.has(rig.log.len() - 1, menu("File")),
        "{why}: File stays gone"
    );
}

#[test]
fn a_hover_switch_destroys_the_old_menu_before_the_new_is_created() {
    let mut rig = Rig::open_file(true);
    let from = rig.log.len();
    rig.hover("Edit");
    assert_gap(&mut rig, from, "hover File -> Edit");
}

#[test]
fn a_right_arrow_switch_destroys_the_old_menu_before_the_new_is_created() {
    let mut rig = Rig::open_file(true);
    let from = rig.log.len();
    request_menu_step(&rig.ctx, 1);
    rig.step(vec![]);
    assert_gap(&mut rig, from, "Right File -> Edit");
}

#[test]
fn a_hover_switch_with_a_submenu_open_keeps_the_order() {
    let mut rig = Rig::view_with_toolbars(true);
    let from = rig.log.len();
    rig.hover("Edit");
    for _ in 0..20 {
        rig.step(vec![]);
    }
    let edit = (from..rig.log.len())
        .find(|f| rig.has(*f, menu("Edit")))
        .expect("Edit opened");
    let view = (from..edit)
        .rev()
        .find(|f| rig.has(*f, menu("View")))
        .unwrap();
    assert!(
        (view + 1..edit).any(|f| !rig.has(f, menu("View")) && !rig.has(f, menu("Edit"))),
        "View last at {view}, Edit first at {edit}"
    );
}

// ---- (c) X11 and the rest keep closing in one frame --------------------------------------

#[test]
fn without_wayland_everything_closes_in_one_frame() {
    if wayland_session() {
        return; // the platform check would say Wayland whatever the test forces
    }
    let mut rig = Rig::view_with_toolbars(false);
    let from = rig.log.len();
    rig.step(click(pos2(400.0, 450.0)));
    let menu_gone = first_missing(&mut rig, from, menu("View")).expect("closed");
    // egui keeps a child viewport in the output for one pass after its parent's: the menu
    // goes on the click's own frame, which is what "one frame" means here.
    assert_eq!(
        menu_gone, from,
        "closed on the click's frame: {:?}",
        rig.log
    );
}
