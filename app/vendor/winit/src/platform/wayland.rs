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
    content_offset: (i32, i32),
    requested: (i32, i32),
    size: (i32, i32),
) -> PopupPositioner {
    PopupPositioner {
        size,
        anchor_rect: PopupRect {
            x: content_offset.0 + requested.0,
            y: content_offset.1 + requested.1,
            width: 1,
            height: 1,
        },
        offset: (0, 0),
        adjustment: PopupAdjustment {
            slide_x: true,
            slide_y: true,
            flip_x: true,
            flip_y: true,
        },
    }
}

/// Local patch (Drydock, PATCHES.md): an input event that can carry the serial an `xdg_popup`
/// grab needs. Only presses are valid grab serials (mutter dismisses a grab made with a release
/// serial); releases are listed so the tracker can be fed every event unfiltered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupGrabInput {
    ButtonPress(u32),
    ButtonRelease(u32),
    KeyPress(u32),
    KeyRelease(u32),
}

/// Local patch (Drydock, PATCHES.md): remembers the serial a pop-up should grab with: that of
/// the most recent button press or key press, whichever came last. Releases never count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PopupGrabSerial {
    latest: Option<u32>,
}

impl PopupGrabSerial {
    /// Feeds one input event (in arrival order).
    pub fn record(&mut self, input: PopupGrabInput) {
        let serial = match input {
            PopupGrabInput::ButtonPress(serial) | PopupGrabInput::KeyPress(serial) => serial,
            PopupGrabInput::ButtonRelease(_) | PopupGrabInput::KeyRelease(_) => return,
        };
        if serial != 0 {
            self.latest = Some(serial);
        }
    }

    /// The serial to pass to `xdg_popup.grab`, or `None` when no press has been seen yet.
    pub fn serial(&self) -> Option<u32> {
        self.latest
    }
}

/// Local patch (Drydock, PATCHES.md): how long window creation waits for a pop-up's first
/// configure before it treats the pop-up as dismissed.
pub const POPUP_CONFIGURE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

/// Local patch (Drydock, PATCHES.md): what the wait for a pop-up's first configure does next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupConfigureWait {
    /// Keep dispatching events.
    Wait,
    /// Configured and alive: attach the first buffer.
    Configured,
    /// The compositor sent `popup_done` first: the pop-up stays unmapped, no buffer is attached.
    Dismissed,
    /// Neither happened within the timeout: treated like `Dismissed`.
    TimedOut,
}

/// Local patch (Drydock, PATCHES.md): decides the wait for a pop-up's first configure. A
/// dismissed pop-up wins over a configured one (a buffer must never be attached to a pop-up
/// that is already done); then configured; then the timeout (`elapsed >= timeout`).
pub fn popup_configure_wait(
    configured: bool,
    dismissed: bool,
    elapsed: std::time::Duration,
    timeout: std::time::Duration,
) -> PopupConfigureWait {
    if dismissed {
        PopupConfigureWait::Dismissed
    } else if configured {
        PopupConfigureWait::Configured
    } else if elapsed >= timeout {
        PopupConfigureWait::TimedOut
    } else {
        PopupConfigureWait::Wait
    }
}

/// Local patch (Drydock, PATCHES.md): a model of the compositor's placement, following the
/// `xdg_positioner` rules, used only to test the flags (a compositor does the real work).
/// `parent_origin` is the parent's window-geometry top-left on the output and `bounds` the
/// region the pop-up must stay inside, both in output coordinates. Returns the pop-up's
/// top-left corner in output coordinates.
pub fn resolve_popup_position(
    positioner: &PopupPositioner,
    parent_origin: (i32, i32),
    bounds: PopupRect,
) -> (i32, i32) {
    fn resolve_axis(
        parent: i32,
        anchor: i32,
        anchor_size: i32,
        offset: i32,
        size: i32,
        bounds_start: i32,
        bounds_size: i32,
        flip: bool,
        slide: bool,
    ) -> i32 {
        let bounds_end = bounds_start + bounds_size;
        let position = parent + anchor + offset;
        let inside = |position: i32| position >= bounds_start && position + size <= bounds_end;

        if inside(position) {
            return position;
        }

        if flip {
            let flipped = parent + anchor + anchor_size - size + offset;
            if inside(flipped) {
                return flipped;
            }
        }

        if slide {
            if position < bounds_start || size >= bounds_size {
                return bounds_start;
            }
            return bounds_end - size;
        }

        position
    }

    let x = resolve_axis(
        parent_origin.0,
        positioner.anchor_rect.x,
        positioner.anchor_rect.width,
        positioner.offset.0,
        positioner.size.0,
        bounds.x,
        bounds.width,
        positioner.adjustment.flip_x,
        positioner.adjustment.slide_x,
    );
    let y = resolve_axis(
        parent_origin.1,
        positioner.anchor_rect.y,
        positioner.anchor_rect.height,
        positioner.offset.1,
        positioner.size.1,
        bounds.y,
        bounds.height,
        positioner.adjustment.flip_y,
        positioner.adjustment.slide_y,
    );

    (x, y)
}
