//! # Wayland
//!
//! **Note:** Windows don't appear on Wayland until you draw/present to them.
//!
//! By default, Winit loads system libraries using `dlopen`. This can be
//! disabled by disabling the `"wayland-dlopen"` cargo feature.
//!
//! ## Client-side decorations
//!
//! Winit provides client-side decorations by default, but the behaviour can
//! be controlled with the following feature flags:
//!
//! * `wayland-csd-adwaita` (default).
//! * `wayland-csd-adwaita-crossfont`.
//! * `wayland-csd-adwaita-notitle`.

use std::ffi::c_void;
use std::ptr::NonNull;

use crate::event_loop::{ActiveEventLoop, EventLoop, EventLoopBuilder};
use crate::monitor::MonitorHandle;
use crate::window::{Window, WindowAttributes};

pub use crate::window::Theme;

/// Additional methods on [`ActiveEventLoop`] that are specific to Wayland.
pub trait ActiveEventLoopExtWayland {
    /// True if the [`ActiveEventLoop`] uses Wayland.
    fn is_wayland(&self) -> bool;
}

impl ActiveEventLoopExtWayland for ActiveEventLoop {
    #[inline]
    fn is_wayland(&self) -> bool {
        self.p.is_wayland()
    }
}

/// Additional methods on [`EventLoop`] that are specific to Wayland.
pub trait EventLoopExtWayland {
    /// True if the [`EventLoop`] uses Wayland.
    fn is_wayland(&self) -> bool;
}

impl<T: 'static> EventLoopExtWayland for EventLoop<T> {
    #[inline]
    fn is_wayland(&self) -> bool {
        self.event_loop.is_wayland()
    }
}

/// Additional methods on [`EventLoopBuilder`] that are specific to Wayland.
pub trait EventLoopBuilderExtWayland {
    /// Force using Wayland.
    fn with_wayland(&mut self) -> &mut Self;

    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;
}

impl<T> EventLoopBuilderExtWayland for EventLoopBuilder<T> {
    #[inline]
    fn with_wayland(&mut self) -> &mut Self {
        self.platform_specific.forced_backend = Some(crate::platform_impl::Backend::Wayland);
        self
    }

    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.platform_specific.any_thread = any_thread;
        self
    }
}

/// Additional methods on [`Window`] that are specific to Wayland.
///
/// [`Window`]: crate::window::Window
pub trait WindowExtWayland {
    /// Returns `xdg_toplevel` of the window or [`None`] if the window is X11 window.
    fn xdg_toplevel(&self) -> Option<NonNull<c_void>>;
}

impl WindowExtWayland for Window {
    #[inline]
    fn xdg_toplevel(&self) -> Option<NonNull<c_void>> {
        #[allow(clippy::single_match)]
        match &self.window {
            #[cfg(x11_platform)]
            crate::platform_impl::Window::X(_) => None,
            #[cfg(wayland_platform)]
            crate::platform_impl::Window::Wayland(window) => window.xdg_toplevel(),
        }
    }
}

/// Additional methods on [`WindowAttributes`] that are specific to Wayland.
pub trait WindowAttributesExtWayland {
    /// Build window with the given name.
    ///
    /// The `general` name sets an application ID, which should match the `.desktop`
    /// file distributed with your program. The `instance` is a `no-op`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    fn with_name(self, general: impl Into<String>, instance: impl Into<String>) -> Self;
}

impl WindowAttributesExtWayland for WindowAttributes {
    #[inline]
    fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.platform_specific.name =
            Some(crate::platform_impl::ApplicationName::new(general.into(), instance.into()));
        self
    }
}

/// Additional methods on `MonitorHandle` that are specific to Wayland.
pub trait MonitorHandleExtWayland {
    /// Returns the inner identifier of the monitor.
    fn native_id(&self) -> u32;
}

impl MonitorHandleExtWayland for MonitorHandle {
    #[inline]
    fn native_id(&self) -> u32 {
        self.inner.native_identifier()
    }
}

/// Local patch (Drydock, PATCHES.md): a rectangle in logical pixels (`x`, `y` is the top-left
/// corner), used for pop-up placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopupRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Local patch (Drydock, PATCHES.md): the `xdg_positioner` constraint-adjustment flags a pop-up
/// asks for (the protocol's `constraint_adjustment` bits, as separate booleans).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PopupAdjustment {
    pub slide_x: bool,
    pub slide_y: bool,
    pub flip_x: bool,
    pub flip_y: bool,
}

/// Local patch (Drydock, PATCHES.md): what winit sends to `xdg_positioner` for a pop-up
/// window. The anchor is always the anchor rect's top-left corner and the gravity bottom-right
/// (the pop-up's top-left corner sits on the anchor point and it grows right and down).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopupPositioner {
    /// `set_size`: the pop-up's size (logical pixels).
    pub size: (i32, i32),
    /// `set_anchor_rect`: relative to the parent's window geometry (frame included).
    pub anchor_rect: PopupRect,
    /// `set_offset`.
    pub offset: (i32, i32),
    /// `set_constraint_adjustment`.
    pub adjustment: PopupAdjustment,
}

/// Local patch (Drydock, PATCHES.md): the positioner for a pop-up whose top-left corner is
/// wanted at `requested` (logical pixels from the top-left of the parent's *content area*, the
/// meaning of `WindowAttributes::with_position` for a Wayland pop-up) with the given `size`.
/// `content_offset` is where the parent's content area starts inside its window geometry
/// (the client-side title bar's height as `(0, height)`; `(0, 0)` without one).
///
/// Pure function; this is what `Window::new` feeds to the compositor.
pub fn popup_positioner(
    _content_offset: (i32, i32),
    _requested: (i32, i32),
    _size: (i32, i32),
) -> PopupPositioner {
    unimplemented!("m12-winit-popup-wayland")
}

/// Local patch (Drydock, PATCHES.md): a model of the compositor's placement, following the
/// `xdg_positioner` rules, used only to test the flags (a compositor does the real work).
/// `parent_origin` is the parent's window-geometry top-left on the output and `bounds` the
/// region the pop-up must stay inside, both in output coordinates. Returns the pop-up's
/// top-left corner in output coordinates.
pub fn resolve_popup_position(
    _positioner: &PopupPositioner,
    _parent_origin: (i32, i32),
    _bounds: PopupRect,
) -> (i32, i32) {
    unimplemented!("m12-winit-popup-wayland")
}
