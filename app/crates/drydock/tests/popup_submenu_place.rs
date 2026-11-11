// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-wayland-submenu-place: on Wayland the position asked for a submenu's pop-up is its row's
//! right edge and top in the *menu pop-up's own* points, nothing added (the winit patch takes
//! a pop-up-parented position in the parent pop-up's coordinates). Drives the real viewport
//! path with no window, as `popup_order.rs` does, and records the `ViewportBuilder` position
//! each pop-up viewport was created with.

use drydock::chrome::{
    MenuState, menu_bar_with_toolbars, request_menu, request_submenu, submenu_popup_title,
    submenu_row_rect,
};
use drydock::popup::{force_wayland_windows, popup_viewport_id};
use drydock::toolbars::ToolbarLayout;
use eframe::egui::{self, Pos2, Rect, ViewportId, pos2, vec2};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Positions = Rc<RefCell<HashMap<ViewportId, Option<Pos2>>>>;

/// View > Toolbars open under forced Wayland. `root_inner`: what the platform reports as the
/// main window's inner rectangle (`None` on Wayland; some tests give one anyway).
fn open_toolbars(root_inner: Option<Rect>) -> (egui::Context, Positions) {
    let seen: Positions = Rc::default();
    let record = seen.clone();
    egui::Context::set_immediate_viewport_renderer(move |ctx, viewport| {
        record
            .borrow_mut()
            .insert(viewport.ids.this, viewport.builder.position);
        let mut raw = egui::RawInput {
            viewport_id: viewport.ids.this,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 300.0))),
            ..Default::default()
        };
        raw.viewports
            .insert(viewport.ids.this, egui::ViewportInfo::default());
        let mut cb = viewport.viewport_ui_cb;
        let mut out = ctx.run_ui(raw, |ui| cb(ui));
        out.textures_delta.clear();
    });
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    ctx.set_fonts(drydock::look::font_definitions());
    force_wayland_windows(&ctx, true);
    let mut layout = ToolbarLayout::default();
    let state = MenuState::default();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(700.0, 500.0));
    let mut time = 0.0;
    let mut frame = |ctx: &egui::Context, layout: &mut ToolbarLayout| {
        let mut raw = egui::RawInput {
            screen_rect: Some(screen),
            time: Some(time),
            focused: true,
            ..Default::default()
        };
        raw.viewports.insert(
            ViewportId::ROOT,
            egui::ViewportInfo {
                focused: Some(true),
                inner_rect: root_inner,
                ..Default::default()
            },
        );
        time += 0.1;
        let mut out = ctx.run_ui(raw, |ui| {
            menu_bar_with_toolbars(ui, &state, layout);
        });
        out.textures_delta.clear();
    };
    frame(&ctx, &mut layout);
    frame(&ctx, &mut layout);
    request_menu(&ctx, "View");
    for _ in 0..4 {
        frame(&ctx, &mut layout);
    }
    request_submenu(&ctx, "Toolbars");
    for _ in 0..4 {
        frame(&ctx, &mut layout);
    }
    (ctx, seen)
}

fn toolbars_request(ctx: &egui::Context, seen: &Positions) -> (Rect, Pos2) {
    let row = submenu_row_rect(ctx, "Toolbars").expect("the Toolbars row was drawn");
    let at = seen
        .borrow()
        .get(&popup_viewport_id(&submenu_popup_title("Toolbars")))
        .copied()
        .expect("the Toolbars pop-up was created")
        .expect("the Toolbars pop-up was given a position");
    (row, at)
}

#[test]
fn the_submenu_is_asked_for_at_its_rows_right_and_top_in_the_menu() {
    let (ctx, seen) = open_toolbars(None);
    let (row, at) = toolbars_request(&ctx, &seen);
    // The row is in the View pop-up's own points, so it starts well inside a 300-px viewport.
    assert!(row.right() > 60.0 && row.right() <= 300.0, "row {row:?}");
    assert!(
        (at.x - row.right()).abs() < 0.5 && (at.y - row.top()).abs() < 0.5,
        "asked for {at:?}, the row is {row:?}"
    );
}

#[test]
fn a_reported_window_position_is_not_added_on_wayland() {
    // Wayland reports none, but if a position were known it must not shift the request.
    let root = Rect::from_min_size(pos2(74.0, 26.0), vec2(700.0, 500.0));
    let (ctx, seen) = open_toolbars(Some(root));
    let (row, at) = toolbars_request(&ctx, &seen);
    assert!(
        (at.x - row.right()).abs() < 0.5 && (at.y - row.top()).abs() < 0.5,
        "asked for {at:?}, the row is {row:?}"
    );
}
