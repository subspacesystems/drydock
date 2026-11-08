// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A floating dock panel as its own OS window (`docs/specs/m11-floating-panels.md`).
//!
//! Positions in a [`DockLayout`](crate::dock::DockLayout) are relative to the main window's
//! inner top-left corner, so a saved layout means the same thing in the embedded fallback
//! (UI tests) and in real windows.

use eframe::egui::{self, Pos2, Rect, Vec2, vec2};

use crate::dock::{self, PanelEvent, PanelId};

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
    egui::ViewportId::from_hash_of(("drydock-floating-panel", dock::panel_key(id)))
}

/// Screen position of a panel whose layout position is `layout_pos`, with the main window's
/// inner rectangle at `root_inner` (both in screen points).
pub fn screen_position(root_inner: Rect, layout_pos: Pos2) -> Pos2 {
    root_inner.min + layout_pos.to_vec2()
}

/// The layout position of a window whose inner top-left corner is at `screen_pos`.
pub fn layout_position(root_inner: Rect, screen_pos: Pos2) -> Pos2 {
    screen_pos - root_inner.min.to_vec2()
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
    let mut outcome = FloatingOutcome::default();
    let mut content = Some(content);

    if ctx.embed_viewports() {
        let response = egui::Window::new(dock::title(id))
            .id(egui::Id::new(("floating-panel", id)))
            .title_bar(false)
            .collapsible(false)
            .movable(false)
            .resizable(true)
            .vscroll(true)
            .current_pos(rect.min)
            .default_size(rect.size())
            .min_size(vec2(dock::MIN_FLOAT_SIZE.0, dock::MIN_FLOAT_SIZE.1))
            .show(ctx, |ui| {
                if let Some(event) = dock::panel_header(ui, id, false, false) {
                    outcome.events.push(event);
                }
                ui.horizontal(|ui| {
                    if ui.button("Dock panel").clicked() {
                        outcome.dock = true;
                    }
                });
                ui.separator();
                if let Some(content) = content.take() {
                    content(ui);
                }
            });
        if let Some(response) = response {
            outcome.rect = Some(response.response.rect);
        }
        return outcome;
    }

    let root_inner = ctx.input(|input| input.viewport().inner_rect);
    let viewport_id = floating_viewport_id(id);
    let known = ctx.input(|input| input.raw.viewports.contains_key(&viewport_id));
    let mut builder = crate::window_icon::with_app_identity(
        egui::ViewportBuilder::default()
            .with_title(dock::title(id))
            .with_visible(known)
            .with_resizable(true)
            .with_min_inner_size(Vec2::new(dock::MIN_FLOAT_SIZE.0, dock::MIN_FLOAT_SIZE.1)),
    );
    if !known {
        builder = builder.with_inner_size(rect.size());
        if let Some(root_inner) = root_inner {
            builder = builder.with_position(screen_position(root_inner, rect.min));
        }
    }

    let mut inner_rect = None;
    ctx.show_viewport_immediate(viewport_id, builder, |ui, _class| {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Dock panel").clicked() {
                    outcome.dock = true;
                }
            });
            ui.separator();
            if let Some(content) = content.take() {
                content(ui);
            }
        });
        if ui.input(|input| input.viewport().close_requested()) {
            outcome.events.push(PanelEvent::Close(id));
        }
        inner_rect = ui.input(|input| input.viewport().inner_rect);
    });

    if let (Some(root_inner), Some(inner_rect)) = (root_inner, inner_rect) {
        outcome.rect = Some(Rect::from_min_size(
            layout_position(root_inner, inner_rect.min),
            inner_rect.size(),
        ));
    }
    outcome
}

/// Brings panel `id`'s floating window to the front: a focus command to its viewport, or,
/// embedded, `move_to_top` of its window layer.
pub fn raise_floating_panel(ctx: &egui::Context, id: PanelId) {
    if ctx.embed_viewports() {
        ctx.move_to_top(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new(("floating-panel", id)),
        ));
    } else {
        ctx.send_viewport_cmd_to(floating_viewport_id(id), egui::ViewportCommand::Focus);
    }
}
