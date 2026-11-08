// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Shared dialog pieces after the Windows guidelines (`docs/specs/m6-dialogs.md`): text box
//! labels that end with a colon, and a row of buttons at the bottom right with a default
//! button that Enter presses.

use eframe::egui::{self, Color32, Vec2};

use crate::look;

/// Smallest size of a dialog button, in points.
pub const BUTTON_MIN_SIZE: Vec2 = Vec2::new(88.0, 28.0);

/// Space above a button row, in points.
pub const ROW_GAP: f32 = 12.0;

/// One button of a [`button_row`].
#[derive(Clone, Copy, Debug)]
pub struct DialogButton<'a> {
    pub label: &'a str,
    /// Highlighted, and pressed by Enter. At most one button of a row.
    pub default: bool,
    pub enabled: bool,
}

/// `label` with trailing whitespace removed and a ":" at the end (unchanged if it already
/// ends with one).
pub fn field_label(label: &str) -> String {
    let label = label.trim_end();
    if label.ends_with(':') {
        label.to_owned()
    } else {
        format!("{label}:")
    }
}

/// The fill of the default button under `visuals`: the selection colour, which differs from
/// an ordinary button's resting fill.
pub fn default_fill(visuals: &egui::Visuals) -> Color32 {
    visuals.selection.bg_fill
}

/// [`default_fill`] composited over the resting button fill, so the translucent selection
/// tint is painted as an opaque colour that keeps the label readable.
fn default_fill_over(visuals: &egui::Visuals) -> Color32 {
    let [r, g, b, a] = default_fill(visuals).to_srgba_unmultiplied();
    let base = visuals.widgets.inactive.weak_bg_fill;
    let alpha = f32::from(a) / 255.0;
    let mix = |top: u8, under: u8| {
        (f32::from(top) * alpha + f32::from(under) * (1.0 - alpha)).round() as u8
    };
    Color32::from_rgb(mix(r, base.r()), mix(g, base.g()), mix(b, base.b()))
}

/// Draw `buttons` on one line at the right edge of `ui`, left to right in the order given,
/// [`ROW_GAP`] below whatever is above. Each is at least [`BUTTON_MIN_SIZE`] with a visible
/// border; the default one is filled with [`default_fill`]. Returns the index of the button
/// clicked this frame, or of the default one when Enter is pressed and it is enabled.
/// Escape is the caller's.
pub fn button_row(ui: &mut egui::Ui, buttons: &[DialogButton<'_>]) -> Option<usize> {
    ui.add_space(ROW_GAP);
    let count = buttons.len() as f32;
    let spacing = ui.spacing().item_spacing.x;
    let available = ui.available_width();
    let needed = count * BUTTON_MIN_SIZE.x + (count - 1.0).max(0.0) * spacing;
    let width = if needed <= available || count == 0.0 {
        BUTTON_MIN_SIZE.x
    } else {
        ((available - (count - 1.0) * spacing) / count - 0.5).max(1.0)
    };
    let min_size = egui::vec2(width, BUTTON_MIN_SIZE.y);
    let mut clicked = None;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        for (index, button) in buttons.iter().enumerate().rev() {
            let primary = button.default && button.label != "Cancel" && button.label != "Close";
            if primary {
                ui.add_enabled_ui(button.enabled, |ui| {
                    if look::primary_button_sized(ui, button.label, min_size).clicked() {
                        clicked = Some(index);
                    }
                });
            } else {
                let fill = button.default.then(|| default_fill_over(ui.visuals()));
                let response = ui.add_enabled_ui(button.enabled, |ui| {
                    if let Some(fill) = fill {
                        let widgets = &mut ui.visuals_mut().widgets;
                        for widget in [
                            &mut widgets.inactive,
                            &mut widgets.hovered,
                            &mut widgets.active,
                        ] {
                            widget.weak_bg_fill = fill;
                        }
                    }
                    look::sized_button(ui, button.label, min_size)
                });
                if response.inner.clicked() {
                    clicked = Some(index);
                }
            }
        }
    });
    if clicked.is_some() {
        return clicked;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        buttons
            .iter()
            .position(|button| button.default && button.enabled)
    } else {
        None
    }
}

/// A modal confirmation (`docs/specs/m6-confirm.md`): `title`, `message`, then a
/// [`button_row`] of `action` and "Cancel", Cancel the default. `Some(true)` on the frame
/// `action` is clicked, `Some(false)` on Cancel, Enter or Escape, `None` while it waits.
pub fn confirm_dialog(
    ctx: &egui::Context,
    id: egui::Id,
    title: &str,
    message: &str,
    action: &str,
) -> Option<bool> {
    let mut answer = None;
    let response = egui::Modal::new(id).show(ctx, |ui| {
        ui.set_width(360.0);
        dialog_title(ui, title);
        ui.label(message);
        match button_row(
            ui,
            &[
                DialogButton {
                    label: action,
                    default: false,
                    enabled: true,
                },
                DialogButton {
                    label: "Cancel",
                    default: true,
                    enabled: true,
                },
            ],
        ) {
            Some(0) => answer = Some(true),
            Some(1) => answer = Some(false),
            _ => {}
        }
    });
    if answer.is_none() && response.should_close() {
        answer = Some(false);
    }
    answer
}

/// A modal name prompt (`docs/specs/m6-confirm.md`): `title`, a "Name:" text box editing
/// `name`, then a [`button_row`] of "OK" (the default, enabled while `name` isn't blank)
/// and "Cancel". `Some(true)` on OK or Enter with a name, `Some(false)` on Cancel or
/// Escape, `None` while it waits.
pub fn name_dialog(
    ctx: &egui::Context,
    id: egui::Id,
    title: &str,
    name: &mut String,
) -> Option<bool> {
    let mut answer = None;
    let response = egui::Modal::new(id).show(ctx, |ui| {
        ui.set_width(360.0);
        dialog_title(ui, title);
        ui.horizontal(|ui| {
            let label = ui.label(field_label("Name"));
            let field = ui
                .add(egui::TextEdit::singleline(name))
                .labelled_by(label.id);
            field.request_focus();
        });
        let enabled = !name.trim().is_empty();
        match button_row(
            ui,
            &[
                DialogButton {
                    label: "OK",
                    default: true,
                    enabled,
                },
                DialogButton {
                    label: "Cancel",
                    default: false,
                    enabled: true,
                },
            ],
        ) {
            Some(0) if enabled => answer = Some(true),
            Some(1) => answer = Some(false),
            _ => {}
        }
    });
    if answer.is_none() && response.should_close() {
        answer = Some(false);
    }
    answer
}

/// The dialog's title: strong, and a little larger than the body text.
fn dialog_title(ui: &mut egui::Ui, title: &str) {
    let size = ui.text_style_height(&egui::TextStyle::Body) + 3.0;
    ui.label(egui::RichText::new(title).strong().size(size));
}

/// How a dialog shows up in its own window (`docs/specs/m11-dialog-window.md`).
#[derive(Clone, Copy, Debug)]
pub struct DialogWindow<'a> {
    /// The window's title; also its identity: two dialogs need two titles.
    pub title: &'a str,
    /// The initial inner size in points.
    pub size: Vec2,
}

/// Where a dialog of `size` goes so it is centered over `root_outer` (the main window's outer
/// rectangle in screen points); each coordinate is at least 0.
pub fn centered_position(root_outer: egui::Rect, size: Vec2) -> egui::Pos2 {
    let offset = (root_outer.size() - size) * 0.5;
    egui::pos2(
        (root_outer.min.x + offset.x).max(0.0),
        (root_outer.min.y + offset.y).max(0.0),
    )
}

/// The viewport id of the dialog titled `title`; the same title always gives the same id.
pub fn dialog_viewport_id(title: &str) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("drydock-dialog-window", title))
}

/// Shows `content` as a dialog while `*open`. With `ctx.embed_viewports()` false it is its own
/// OS window (`show_viewport_immediate`); true, an `egui::Window` with the same title. The
/// title bar's close, and Escape, clear `open`. Nothing is drawn when `!*open`.
pub fn dialog_window(
    ctx: &egui::Context,
    window: &DialogWindow<'_>,
    open: &mut bool,
    content: impl FnOnce(&mut egui::Ui),
) {
    if !*open {
        return;
    }
    let mut content = Some(content);
    let mut close = false;

    if ctx.embed_viewports() {
        egui::Window::new(window.title)
            .collapsible(false)
            .resizable(false)
            .default_size(window.size)
            .open(open)
            .show(ctx, |ui| {
                if let Some(content) = content.take() {
                    content(ui);
                }
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
    } else {
        let root_outer = ctx.input(|i| i.viewport().outer_rect);
        let mut builder = egui::ViewportBuilder::default()
            .with_title(window.title)
            .with_inner_size(window.size)
            .with_resizable(false)
            .with_minimize_button(false)
            .with_maximize_button(false);
        if let Some(root_outer) = root_outer {
            builder = builder.with_position(centered_position(root_outer, window.size));
        }
        ctx.show_viewport_immediate(dialog_viewport_id(window.title), builder, |ui, _class| {
            egui::CentralPanel::default().show(ui, |ui| {
                if let Some(content) = content.take() {
                    content(ui);
                }
            });
            if ui.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });
    }

    if close {
        *open = false;
    }
}

/// Like [`dialog_window`], but modal (`docs/specs/m11-modal-window.md`): while `*open`, the
/// window that called it ignores clicks and keys until the dialog is answered. Real window:
/// the viewport title is marked with `egui_winit::mark_modal_title` before the viewport is
/// first shown, and an invisible layer over the calling viewport blocks its input. Embedded
/// (`ctx.embed_viewports()`): an `egui::Modal` with id `id` showing the title, then `content`.
/// Escape and the title bar's close clear `open`; so does the caller's own flag in `content`.
pub fn modal_dialog_window(
    ctx: &egui::Context,
    id: egui::Id,
    window: &DialogWindow<'_>,
    open: &mut bool,
    content: impl FnOnce(&mut egui::Ui),
) {
    let _ = (ctx, id, window, open, content);
    unimplemented!("m11-modal-window")
}
