// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A pop-up as its own OS window (`docs/specs/m12-popup-window.md`): no title bar, placed at a
//! screen position (it may reach past the main window's edge), sized to its content, closed
//! by focus loss or Escape. The menus (m12-menu-popups) use it. Where the platform cannot
//! make such a window, and in UI tests, it is an in-window area.

use eframe::egui::{self, Pos2, Rect, Vec2};

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
pub fn popup_viewport_id(_title: &str) -> egui::ViewportId {
    unimplemented!("m12-popup-window")
}

/// Whether `os` (`std::env::consts::OS` values) can make pop-up windows: "windows" yes;
/// "linux" yes on X11 (`wayland` false) and no on Wayland until m12-winit-popup-wayland;
/// anything else no.
pub fn popup_supported_on(_os: &str, _wayland: bool) -> bool {
    unimplemented!("m12-popup-window")
}

/// [`popup_supported_on`] for this machine: `std::env::consts::OS`, and Wayland when
/// `WAYLAND_DISPLAY` is non-empty and `WINIT_UNIX_BACKEND` is not "x11".
pub fn popup_supported() -> bool {
    unimplemented!("m12-popup-window")
}

/// `Window` only when the viewports are real (`!embed_viewports`) and the platform
/// `supported` it; otherwise `Inline`.
pub fn popup_mode(_embed_viewports: bool, _supported: bool) -> PopupMode {
    unimplemented!("m12-popup-window")
}

/// The window size for content of size `content`: each side rounded up to whole points, at
/// least 1, and at most `monitor` when that is known.
pub fn fit_size(_content: Vec2, _monitor: Option<Vec2>) -> Vec2 {
    unimplemented!("m12-popup-window")
}

/// Where an inline pop-up of size `size` goes inside a viewport whose content rectangle is
/// `content_rect`: `position` minus the root's inner top-left when `root_inner` is known
/// (else `position` as it is), then moved left/up until it fits inside `content_rect`, and
/// never above/left of `content_rect.min`.
pub fn inline_position(
    _root_inner: Option<Rect>,
    _position: Pos2,
    _size: Vec2,
    _content_rect: Rect,
) -> Pos2 {
    unimplemented!("m12-popup-window")
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
    _ctx: &egui::Context,
    _popup: &PopupWindow<'_>,
    _open: &mut bool,
    _content: impl FnOnce(&mut egui::Ui),
) -> PopupOutcome {
    unimplemented!("m12-popup-window")
}
