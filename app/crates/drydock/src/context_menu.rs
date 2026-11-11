// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Context menus as pop-ups (`docs/specs/m12-submenu-popups.md`): a right click on a widget
//! opens a menu at the click position, as an OS window where the platform allows
//! (`popup_window`), inline otherwise. Nothing in the app uses one yet; this is the helper.

use crate::chrome::{popup_max_height, popup_rows};
use crate::popup::{
    PopupWindow, popup_mode, popup_root_inner, popup_supported, popup_window, wayland_session,
};
use eframe::egui::{self, Id, Pos2, Rect};

/// The pop-up title (and viewport name) of the context menu named `id`: distinct per `id`
/// and from the menu and submenu titles.
pub fn context_popup_title(id: &str) -> String {
    format!("drydock-context-menu-{id}")
}

/// Where a context menu opened by a click at `click` (the calling viewport's points) goes, in
/// screen points: `click` plus `root_inner.min` when known.
pub fn context_popup_position(root_inner: Option<Rect>, click: Pos2) -> Pos2 {
    root_inner.map_or(click, |root| click + root.min.to_vec2())
}

/// The label of the context menu's open position in the temporary data.
fn open_id(id: &str) -> Id {
    Id::new(("drydock-context-menu", id))
}

/// Where the context menu `id` was opened, in screen points, while it is open.
pub fn context_menu_open(ctx: &egui::Context, id: &str) -> Option<Pos2> {
    ctx.data(|data| data.get_temp::<Pos2>(open_id(id)))
}

/// Opens the context menu `id` at `click` (the calling viewport's points) as if `response`
/// had been right-clicked there.
pub fn open_context_menu(ctx: &egui::Context, id: &str, click: Pos2) {
    let root_inner = ctx.input(|input| input.viewport().inner_rect);
    let position = context_popup_position(popup_root_inner(root_inner, wayland_session()), click);
    ctx.data_mut(|data| data.insert_temp(open_id(id), position));
}

/// Context menu `id` for `response`: a secondary click on it opens the menu at the pointer
/// (a click elsewhere or on another response with its own menu closes it, Escape closes
/// it); while open, `add_contents` shows its rows in a pop-up (`popup_window`, titled
/// `context_popup_title(id)`). `ui.close()` in the rows closes it. Call every frame.
pub fn context_popup(
    ui: &mut egui::Ui,
    id: &str,
    response: &egui::Response,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let ctx = ui.ctx().clone();
    if response.secondary_clicked()
        && let Some(click) = response.interact_pointer_pos()
    {
        open_context_menu(&ctx, id, click);
    }

    let Some(position) = context_menu_open(&ctx, id) else {
        return;
    };
    let title = context_popup_title(id);
    let mode = popup_mode(ctx.embed_viewports(), popup_supported());
    let root_inner = ctx.input(|input| input.viewport().inner_rect);
    let inline_top = root_inner.map_or(position.y, |root| position.y - root.min.y);
    let max_height = popup_max_height(ui, mode, position.y, inline_top);
    let popup = PopupWindow {
        title: &title,
        position,
        parent: None,
    };
    let mut open = true;
    let mut closed = false;
    popup_window(&ctx, &popup, &mut open, |ui| {
        let (_, asked_to_close) = popup_rows(ui, max_height, add_contents);
        closed = asked_to_close;
    });
    if !open || closed {
        ctx.data_mut(|data| data.remove_temp::<Pos2>(open_id(id)));
    }
}
