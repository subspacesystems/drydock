// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A pop-up as its own OS window (`docs/specs/m12-popup-window.md`): no title bar, placed at a
//! screen position (it may reach past the main window's edge), sized to its content, closed
//! by focus loss or Escape. The menus (m12-menu-popups) use it. Where the platform cannot
//! make such a window, and in UI tests, it is an in-window area.

use eframe::egui::{self, Pos2, Rect, Vec2, pos2, vec2};

/// How a pop-up is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupMode {
    /// Its own OS window (`egui_winit::mark_popup_title`).
    Window,
    /// An `egui::Area` inside the calling viewport.
    Inline,
}

/// What to show and where.
#[derive(Clone, Copy, Debug)]
pub struct PopupWindow<'a> {
    /// Names the viewport (and is the window title, which the platform never shows). Distinct
    /// per pop-up that can be open at once.
    pub title: &'a str,
    /// Where the pop-up's top-left corner goes, in screen points (the same space as
    /// `ViewportInfo::inner_rect` of the root viewport). Inline, it is converted to the
    /// calling viewport's own coordinates.
    pub position: Pos2,
}

/// What [`popup_window`] reports for one frame.
#[derive(Clone, Copy, Debug)]
pub struct PopupOutcome {
    /// How the pop-up was shown this frame.
    pub mode: PopupMode,
    /// Where it is, in screen points; `None` when not known yet (first frame of a window).
    pub rect: Option<Rect>,
}

/// The viewport id of the pop-up titled `title`; equal for equal titles, never `ROOT`.
pub fn popup_viewport_id(title: &str) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("drydock-popup", title))
}

/// Whether `os` (`std::env::consts::OS` values) can make pop-up windows: "windows" yes;
/// "linux" yes on both X11 and Wayland (`wayland` makes no difference there); anything else no.
pub fn popup_supported_on(os: &str, _wayland: bool) -> bool {
    matches!(os, "windows" | "linux")
}

/// The root window's inner rectangle as pop-up positions must see it: on Wayland always `None`
/// (a pop-up is placed relative to the parent's content area, so the root's position, when
/// the platform reports one at all, must not be added); elsewhere `root_inner` unchanged.
/// The menu, submenu and context-menu position functions take this as their `root_inner`.
pub fn popup_root_inner(root_inner: Option<Rect>, wayland: bool) -> Option<Rect> {
    if wayland { None } else { root_inner }
}

/// Whether this session is Wayland: `WAYLAND_DISPLAY` is non-empty and `WINIT_UNIX_BACKEND`
/// is not "x11".
pub fn wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
        && std::env::var_os("WINIT_UNIX_BACKEND").is_none_or(|backend| backend != "x11")
}

/// [`popup_supported_on`] for this machine: `std::env::consts::OS` and [`wayland_session`].
pub fn popup_supported() -> bool {
    popup_supported_on(std::env::consts::OS, wayland_session())
}

/// `Window` only when the viewports are real (`!embed_viewports`) and the platform
/// `supported` it; otherwise `Inline`.
pub fn popup_mode(embed_viewports: bool, supported: bool) -> PopupMode {
    if !embed_viewports && supported {
        PopupMode::Window
    } else {
        PopupMode::Inline
    }
}

/// The window size for content of size `content`: each side rounded up to whole points, at
/// least 1, and at most `monitor` when that is known.
pub fn fit_size(content: Vec2, monitor: Option<Vec2>) -> Vec2 {
    let side = |value: f32, max: Option<f32>| -> f32 {
        let mut size = value.ceil().max(1.0);
        if let Some(max) = max {
            size = size.min(max);
        }
        size
    };
    vec2(
        side(content.x, monitor.map(|m| m.x)),
        side(content.y, monitor.map(|m| m.y)),
    )
}

/// Where an inline pop-up of size `size` goes inside a viewport whose content rectangle is
/// `content_rect`: `position` minus the root's inner top-left when `root_inner` is known
/// (else `position` as it is), then moved left/up until it fits inside `content_rect`, and
/// never above/left of `content_rect.min`.
pub fn inline_position(
    root_inner: Option<Rect>,
    position: Pos2,
    size: Vec2,
    content_rect: Rect,
) -> Pos2 {
    let position = root_inner.map_or(position, |root| position - root.min.to_vec2());
    let mut x = position.x;
    let mut y = position.y;
    if x + size.x > content_rect.max.x {
        x = content_rect.max.x - size.x;
    }
    if y + size.y > content_rect.max.y {
        y = content_rect.max.y - size.y;
    }
    pos2(x.max(content_rect.min.x), y.max(content_rect.min.y))
}

/// The frame of a window pop-up (`docs/specs/m12-popup-paint.md`): square corners, no shadow
/// (the window is rectangular and opaque, so rounding or a shadow would show the clear color;
/// Windows 11 rounds and shadows the window itself), a 1 px border stroke and the menu's
/// fill color, taken from `egui::Frame::menu(style)`, with that frame's inner margin. The
/// inline pop-up keeps egui's own menu frame.
pub fn popup_frame(style: &egui::Style) -> egui::Frame {
    let _ = style;
    unimplemented!("m12-popup-paint")
}

/// The rectangle the pop-up's fill is painted over: the whole viewport rectangle, so no clear
/// color shows at the edges (at fractional scales too). Equal to `viewport_rect`.
pub fn popup_fill_rect(viewport_rect: Rect) -> Rect {
    let _ = viewport_rect;
    unimplemented!("m12-popup-paint")
}

/// The size a pop-up window is created at: `cached` (the content size of the last opening
/// under the same title, already `fit_size`d) when known, else the 120 x 40 placeholder.
pub fn initial_size(cached: Option<Vec2>) -> Vec2 {
    let _ = cached;
    unimplemented!("m12-popup-paint")
}

/// What one frame of a window pop-up does about its size and visibility.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShowStep {
    /// Send `InnerSize(target)`; the window stays as it is (hidden stays hidden) and the frame
    /// painted at the old size does not count as painted.
    Resize(Vec2),
    /// Hidden: paint this frame at the final size and look again on the next one.
    Wait,
    /// Hidden, and a frame was already painted at the final size: send `Visible(true)` now.
    Reveal,
    /// Visible at the right size: nothing to do.
    Keep,
}

/// The show sequence. `visible`: the window was already made visible. `current`: its inner
/// size now. `painted`: the inner size at which a frame was painted while hidden (`None`
/// before the first, and after a `Resize`). `target`: `fit_size` of the content this frame.
/// `frames_hidden`: frames spent hidden so far.
/// - Hidden and `needs_resize(current, target)` (`crate::dialog::needs_resize`, per axis):
///   `Resize(target)`. Hidden at the right size: `Reveal` when `painted` is within a
///   `needs_resize` tolerance of `target`, else `Wait`. After 5 or more `frames_hidden`
///   it is always `Reveal` (a window that never reports its size must still appear).
/// - Visible: `Resize(target)` when the size is off (the content changed), else `Keep`.
///   Never `Wait` or `Reveal`.
pub fn show_step(
    visible: bool,
    current: Vec2,
    painted: Option<Vec2>,
    target: Vec2,
    frames_hidden: u32,
) -> ShowStep {
    let _ = (visible, current, painted, target, frames_hidden);
    unimplemented!("m12-popup-paint")
}

/// Shows `content` as a pop-up while `*open`; nothing is drawn (and a window is closed) when
/// `!*open`. Escape clears `open`, in both modes.
/// - `Window` (`popup_mode(ctx.embed_viewports(), popup_supported())`): a viewport with no
///   decorations and no resize/minimize/maximize, titled `popup.title`, marked with
///   `egui_winit::mark_popup_title` before it is first shown, opened at `popup.position`
///   and resized to `fit_size` of the content as it changes. It clears `open` when it loses
///   focus after having had it, or when the OS asks to close it.
/// - `Inline`: a foreground `egui::Area` at `inline_position`, in a frame like a menu's. A
///   press outside it also clears `open`.
pub fn popup_window(
    ctx: &egui::Context,
    popup: &PopupWindow<'_>,
    open: &mut bool,
    content: impl FnOnce(&mut egui::Ui),
) -> PopupOutcome {
    let viewport_id = popup_viewport_id(popup.title);
    if !*open {
        forget_state(ctx, viewport_id);
        return PopupOutcome {
            mode: PopupMode::Inline,
            rect: None,
        };
    }
    let mode = popup_mode(ctx.embed_viewports(), popup_supported());
    if mode == PopupMode::Inline {
        popup_window_inline(ctx, popup, open, content)
    } else {
        popup_window_window(ctx, popup, open, content)
    }
}

/// Clears the temporary data a pop-up keeps between openings (the inline size, the window's
/// first-measurement flag), so the next opening starts fresh.
fn forget_state(ctx: &egui::Context, viewport_id: egui::ViewportId) {
    ctx.data_mut(|data| {
        data.remove_temp::<Vec2>(viewport_id.0.with("popup-size"));
        data.remove_temp::<bool>(viewport_id.0.with("popup-shown"));
        data.remove_temp::<bool>(viewport_id.0.with("popup-focused"));
        data.remove_temp::<bool>(viewport_id.0.with("popup-has-focus"));
    });
}

/// The window pop-up: its own viewport with no decorations, opened at `popup.position`,
/// resized to its content, and closed by Escape, a close request, or focus loss after it had
/// focus. It stays hidden until the content is measured so the small placeholder does not
/// flash, but is visible by the second frame.
fn popup_window_window(
    ctx: &egui::Context,
    popup: &PopupWindow<'_>,
    open: &mut bool,
    content: impl FnOnce(&mut egui::Ui),
) -> PopupOutcome {
    let viewport_id = popup_viewport_id(popup.title);
    egui_winit::mark_popup_title(ctx, popup.title);
    let placeholder = vec2(120.0, 40.0);
    let shown_id = viewport_id.0.with("popup-shown");
    let focus_id = viewport_id.0.with("popup-focused");
    let shown = ctx.data(|data| data.get_temp::<bool>(shown_id).unwrap_or(false));
    let builder = crate::window_icon::with_app_identity(
        egui::ViewportBuilder::default()
            .with_title(popup.title)
            .with_decorations(false)
            .with_resizable(false)
            .with_minimize_button(false)
            .with_maximize_button(false)
            .with_taskbar(false)
            .with_visible(shown)
            .with_position(popup.position)
            .with_inner_size(placeholder),
    );
    let mut close = false;
    let mut rect: Option<Rect> = None;
    let mut content = Some(content);
    ctx.show_viewport_immediate(viewport_id, builder, |ui, _class| {
        let frame = egui::Frame::menu(ui.style());
        let margin = frame.total_margin().sum();
        let content_size = egui::CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| {
                let start = ui.min_rect().min;
                if let Some(content) = content.take() {
                    content(ui);
                }
                let end = ui.min_rect().max;
                end - start
            })
            .inner
            + margin;
        let monitor = ui.input(|input| input.viewport().monitor_size);
        let target = fit_size(content_size, monitor);
        let current = ui
            .input(|input| input.viewport().inner_rect)
            .map_or(placeholder, |r| r.size());
        if crate::dialog::needs_resize(current.x, target.x)
            || crate::dialog::needs_resize(current.y, target.y)
        {
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::InnerSize(target));
            ctx.request_repaint();
        }
        if !shown {
            ctx.data_mut(|data| data.insert_temp(shown_id, true));
            ctx.request_repaint();
        }
        let focused = ui.input(|input| input.viewport().focused);
        ctx.data_mut(|data| {
            data.insert_temp(viewport_id.0.with("popup-has-focus"), focused == Some(true));
        });
        if ctx.data(|data| data.get_temp::<bool>(focus_id).unwrap_or(false))
            && focused == Some(false)
        {
            close = true;
        }
        if focused == Some(true) {
            ctx.data_mut(|data| data.insert_temp(focus_id, true));
            let step = if ui
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
            {
                Some(1)
            } else if ui
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft))
            {
                Some(-1)
            } else {
                None
            };
            if let Some(step) = step {
                crate::chrome::request_menu_step(ctx, step);
            }
        }
        if ui.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        rect = ui.input(|input| input.viewport().inner_rect);
    });
    if close {
        forget_state(ctx, viewport_id);
        *open = false;
    }
    PopupOutcome {
        mode: PopupMode::Window,
        rect,
    }
}

/// The inline pop-up: a foreground `egui::Area` at [`inline_position`], framed like a menu.
/// Escape and a primary press outside it clear `open`; a press inside does not.
fn popup_window_inline(
    ctx: &egui::Context,
    popup: &PopupWindow<'_>,
    open: &mut bool,
    content: impl FnOnce(&mut egui::Ui),
) -> PopupOutcome {
    let content_rect = ctx.content_rect();
    let root_inner = ctx.input(|input| input.viewport().inner_rect);
    let viewport_id = popup_viewport_id(popup.title);
    let size = ctx.data(|data| data.get_temp::<Vec2>(viewport_id.0.with("popup-size")));
    let size = size.unwrap_or_default();
    let position = inline_position(root_inner, popup.position, size, content_rect);
    let mut close = false;
    let mut content = Some(content);
    let response = egui::Area::new(egui::Id::new(("drydock-popup-area", popup.title)))
        .order(egui::Order::Foreground)
        .fixed_pos(position)
        .interactable(true)
        .show(ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                if let Some(content) = content.take() {
                    content(ui);
                }
            });
        });
    let rect = Some(response.response.rect);
    let new_size = response.response.rect.size();
    let outside = ctx.input(|input| {
        input.pointer.button_pressed(egui::PointerButton::Primary)
            && input
                .pointer
                .interact_pos()
                .is_some_and(|pos| !response.response.rect.contains(pos))
    });
    if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) || outside
    {
        close = true;
    }
    ctx.data_mut(|data| data.insert_temp(viewport_id.0.with("popup-size"), new_size));
    if close {
        *open = false;
    }
    PopupOutcome {
        mode: PopupMode::Inline,
        rect,
    }
}
