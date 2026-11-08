// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Shared dialog pieces after the Windows guidelines (`docs/specs/m6-dialogs.md`): text box
//! labels that end with a colon, and a row of buttons at the bottom right with a default
//! button that Enter presses.

use eframe::egui::{self, Color32, Vec2};

use crate::look;

/// Focus the text field `id` once its window has the keyboard, so an X11 input context is
/// not created for a window that does not have focus yet (which would stop the window from
/// ever getting its focus event). Call it before adding the field, so a key that arrives in
/// the frame the window gains focus is not dropped. Before the window reports focus (`None`,
/// as in UI tests and inline dialogs) this behaves like
/// [`egui::Memory::request_focus`].
pub fn focus_when_window_active(ui: &mut egui::Ui, id: egui::Id) {
    if ui.memory(|memory| memory.has_focus(id)) {
        return;
    }
    if !ui.memory(|memory| memory.allows_interaction(ui.layer_id())) {
        return;
    }
    let window_active = ui.ctx().input(|i| i.viewport().focused != Some(false));
    if window_active {
        ui.memory_mut(|memory| memory.request_focus(id));
    }
}

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
    let mut open = true;
    let window = DialogWindow {
        title,
        size: Vec2::new(380.0, 130.0),
    };
    modal_prompt_window(ctx, id, &window, &mut open, |ui| {
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
    if answer.is_none() && !open {
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
    let mut open = true;
    let window = DialogWindow {
        title,
        size: Vec2::new(360.0, 120.0),
    };
    modal_prompt_window(ctx, id, &window, &mut open, |ui| {
        ui.horizontal(|ui| {
            let field_id = id.with("name");
            focus_when_window_active(ui, field_id);
            let label = ui.label(field_label("Name"));
            ui.add(egui::TextEdit::singleline(name).id(field_id))
                .labelled_by(label.id);
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
    if answer.is_none() && !open {
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

/// Space kept free of a dialog window's height on the monitor (title bar, panels), in points.
pub const SCREEN_MARGIN: f32 = 80.0;

/// A dialog window's height and whether its content scrolls (`docs/specs/m11-dialog-fit-content.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogFit {
    pub height: f32,
    pub scroll: bool,
}

/// The window height for content that needs `content_height` points (panel margins
/// included): the content's height, never below 0; but when `monitor_height` is known (and
/// above [`SCREEN_MARGIN`]) at most `monitor_height - SCREEN_MARGIN`, in which case `scroll`
/// is true. Unknown or unusable monitor heights do not clamp.
pub fn fit_height(content_height: f32, monitor_height: Option<f32>) -> DialogFit {
    let height = content_height.max(0.0);
    if let Some(monitor_height) = monitor_height
        && monitor_height > SCREEN_MARGIN
        && height > monitor_height - SCREEN_MARGIN
    {
        return DialogFit {
            height: monitor_height - SCREEN_MARGIN,
            scroll: true,
        };
    }
    DialogFit {
        height,
        scroll: false,
    }
}

/// Whether a window whose inner height is `current` should be sent `target`: only when they
/// differ by more than 1 point, so a settled window is not resized every frame.
pub fn needs_resize(current: f32, target: f32) -> bool {
    (current - target).abs() > 1.0
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
    dialog_window_impl(ctx, egui::Id::NULL, window, open, false, false, content);
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
    dialog_window_impl(ctx, id, window, open, true, false, content);
}

/// A modal for the shared prompts: embedded, a click outside answers it too.
pub(crate) fn modal_prompt_window(
    ctx: &egui::Context,
    id: egui::Id,
    window: &DialogWindow<'_>,
    open: &mut bool,
    content: impl FnOnce(&mut egui::Ui),
) {
    dialog_window_impl(ctx, id, window, open, true, true, content);
}

/// The shared body of [`dialog_window`] and [`modal_dialog_window`].
fn dialog_window_impl(
    ctx: &egui::Context,
    id: egui::Id,
    window: &DialogWindow<'_>,
    open: &mut bool,
    modal: bool,
    close_on_outside: bool,
    content: impl FnOnce(&mut egui::Ui),
) {
    if !*open {
        return;
    }
    let mut content = Some(content);
    let mut close = false;

    if ctx.embed_viewports() {
        if modal {
            let response = egui::Modal::new(id).show(ctx, |ui| {
                ui.set_width(window.size.x);
                dialog_title(ui, window.title);
                if let Some(content) = content.take() {
                    content(ui);
                }
            });
            let should_close = if close_on_outside {
                response.should_close()
            } else {
                response.is_top_modal
                    && !response.any_popup_open
                    && ctx.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    })
            };
            if should_close {
                close = true;
            }
        } else {
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
        }
    } else {
        let viewport_id = dialog_viewport_id(window.title);
        let shown_id = viewport_id.0.with("dialog-shown");
        if !ctx.data_mut(|data| *data.get_temp_mut_or(shown_id, false)) {
            ctx.data_mut(|data| data.insert_temp(shown_id, true));
            ctx.request_repaint();
            return;
        }
        if modal {
            egui_winit::mark_modal_title(ctx, window.title);
            let rect = ctx.content_rect();
            let area = egui::Area::new(id.with("modal-blocker"))
                .order(egui::Order::Foreground)
                .fixed_pos(rect.min)
                .interactable(true);
            ctx.memory_mut(|memory| memory.set_modal_layer(area.layer()));
            area.show(ctx, |ui| {
                ui.allocate_rect(rect, egui::Sense::click_and_drag());
            });
        }

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
        let scroll_id = viewport_id.0.with("dialog-fit-scroll");
        let mut scroll = ctx.data_mut(|data| *data.get_temp_mut_or(scroll_id, false));
        ctx.show_viewport_immediate(viewport_id, builder, |ui, _class| {
            let monitor_height = ui.input(|input| input.viewport().monitor_size.map(|size| size.y));
            let frame = egui::Frame::central_panel(ui.style());
            let margin_height = frame.total_margin().sum().y;
            let panel = egui::CentralPanel::default().frame(frame).show(ui, |ui| {
                if scroll {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            if let Some(content) = content.take() {
                                content(ui);
                            }
                        })
                        .content_size
                        .y
                } else {
                    ui.scope(|ui| {
                        if let Some(content) = content.take() {
                            content(ui);
                        }
                    })
                    .response
                    .rect
                    .height()
                }
            });
            let content_height = panel.inner + margin_height;
            let fit = fit_height(content_height, monitor_height);
            if fit.scroll && !scroll {
                scroll = true;
                ctx.data_mut(|data| data.insert_temp(scroll_id, true));
                ctx.request_repaint();
            }
            let current = ui
                .input(|input| input.viewport().inner_rect)
                .map_or(window.size.y, |rect| rect.height());
            if needs_resize(current, fit.height) {
                ctx.send_viewport_cmd_to(
                    viewport_id,
                    egui::ViewportCommand::InnerSize(egui::vec2(window.size.x, fit.height)),
                );
                ctx.request_repaint();
            }
            if ui.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });

        if modal {
            ctx.memory_mut(|memory| memory.stop_text_input());
        }
    }

    if close {
        *open = false;
    }
}
