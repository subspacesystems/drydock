// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A floating dock panel as its own OS window (`docs/specs/m11-floating-panels.md`).
//!
//! Positions in a [`DockLayout`](crate::dock::DockLayout) are relative to the main window's
//! inner top-left corner, so a saved layout means the same thing in the embedded fallback
//! (UI tests) and in real windows.

use eframe::egui::{self, Pos2, Rect};

use crate::dock::{PanelEvent, PanelId};

/// What a floating panel reports for one frame.
#[derive(Clone, Debug, Default)]
pub struct FloatingOutcome {
    /// `PanelEvent::Close` when the OS window's title bar close (or, embedded, the header's
    /// close button) was used; `PanelEvent::Dropped` from the embedded header's drag.
    pub events: Vec<PanelEvent>,
    /// The "Dock panel" button was clicked.
    pub dock: bool,
    /// Where the panel is now, relative to the main window's inner top-left corner; `None`
    /// when the platform doesn't say.
    pub rect: Option<Rect>,
}

/// The viewport id of panel `id`'s window; distinct per panel, never `ROOT`.
pub fn floating_viewport_id(id: PanelId) -> egui::ViewportId {
    let _ = id;
    unimplemented!("m11-floating-panels")
}

/// Screen position of a panel whose layout position is `layout_pos`, with the main window's
/// inner rectangle at `root_inner` (both in screen points).
pub fn screen_position(root_inner: Rect, layout_pos: Pos2) -> Pos2 {
    let _ = (root_inner, layout_pos);
    unimplemented!("m11-floating-panels")
}

/// The layout position of a window whose inner top-left corner is at `screen_pos`.
pub fn layout_position(root_inner: Rect, screen_pos: Pos2) -> Pos2 {
    let _ = (root_inner, screen_pos);
    unimplemented!("m11-floating-panels")
}

/// Shows panel `id` floating while the caller calls this each frame; `rect` is the layout
/// rectangle (position relative to the main window, size in points) it opens at.
/// - Real window (`!ctx.embed_viewports()`): an OS window titled `dock::title(id)`, opened
///   at `screen_position(root inner rect, rect.min)` (no position when the root's inner
///   rectangle is unknown), resizable, with `content` below a "Dock panel" button row.
///   The title bar's close reports `PanelEvent::Close(id)`. `rect` in the outcome follows
///   the window as the user moves and resizes it; the builder's position and size are only
///   used when the window is created.
/// - Embedded (`ctx.embed_viewports()`): the `egui::Window` the app has always used (no
///   title bar of its own, `panel_header` as the header, `rect` as position and size), plus
///   the "Dock panel" row below the header.
pub fn floating_panel_window(
    ctx: &egui::Context,
    id: PanelId,
    rect: Rect,
    content: impl FnOnce(&mut egui::Ui),
) -> FloatingOutcome {
    let _ = (ctx, id, rect, content);
    unimplemented!("m11-floating-panels")
}

/// Brings panel `id`'s floating window to the front: a focus command to its viewport, or,
/// embedded, `move_to_top` of its window layer.
pub fn raise_floating_panel(ctx: &egui::Context, id: PanelId) {
    let _ = (ctx, id);
    unimplemented!("m11-floating-panels")
}
