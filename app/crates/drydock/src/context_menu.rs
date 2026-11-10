// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Context menus as pop-ups (`docs/specs/m12-submenu-popups.md`): a right click on a widget
//! opens a menu at the click position, as an OS window where the platform allows
//! (`popup_window`), inline otherwise. Nothing in the app uses one yet; this is the helper.

use eframe::egui::{self, Pos2, Rect};

/// The pop-up title (and viewport name) of the context menu named `id`: distinct per `id`
/// and from the menu and submenu titles.
pub fn context_popup_title(_id: &str) -> String {
    unimplemented!("m12-submenu-popups")
}

/// Where a context menu opened by a click at `click` (the calling viewport's points) goes, in
/// screen points: `click` plus `root_inner.min` when known.
pub fn context_popup_position(_root_inner: Option<Rect>, _click: Pos2) -> Pos2 {
    unimplemented!("m12-submenu-popups")
}

/// Where the context menu `id` was opened, in screen points, while it is open.
pub fn context_menu_open(_ctx: &egui::Context, _id: &str) -> Option<Pos2> {
    unimplemented!("m12-submenu-popups")
}

/// Opens the context menu `id` at `click` (the calling viewport's points) as if `response`
/// had been right-clicked there.
pub fn open_context_menu(_ctx: &egui::Context, _id: &str, _click: Pos2) {
    unimplemented!("m12-submenu-popups")
}

/// Context menu `id` for `response`: a secondary click on it opens the menu at the pointer
/// (a click elsewhere or on another response with its own menu closes it, Escape closes
/// it); while open, `add_contents` shows its rows in a pop-up (`popup_window`, titled
/// `context_popup_title(id)`). `ui.close()` in the rows closes it. Call every frame.
pub fn context_popup(
    _ui: &mut egui::Ui,
    _id: &str,
    _response: &egui::Response,
    _add_contents: impl FnOnce(&mut egui::Ui),
) {
    unimplemented!("m12-submenu-popups")
}
