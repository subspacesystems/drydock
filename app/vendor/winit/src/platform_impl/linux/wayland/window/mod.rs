//! The Wayland window.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rustix::event::{PollFd, PollFlags};

use sctk::reexports::client::protocol::wl_display::WlDisplay;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Proxy, QueueHandle};

use sctk::compositor::{CompositorState, Region, SurfaceData};
use sctk::reexports::protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
use sctk::reexports::protocols::xdg::dialog::v1::client::xdg_dialog_v1::XdgDialogV1;
use sctk::reexports::protocols::xdg::shell::client::xdg_positioner::{
    Anchor, ConstraintAdjustment, Gravity,
};
use sctk::shell::xdg::popup::Popup;
use sctk::shell::xdg::window::WindowDecorations;
use sctk::shell::xdg::{XdgPositioner, XdgShell};
use sctk::shell::WaylandSurface;

use tracing::{debug, warn};

use crate::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOsError};
use crate::event::{Ime, WindowEvent};
use crate::event_loop::AsyncRequestSerial;
use crate::platform::wayland::{
    POPUP_CONFIGURE_TIMEOUT, PopupConfigureWait, PopupDismissed, PopupPositioner, PopupResize,
    popup_configure_wait, popup_positioner_under, popup_resize_action, popup_should_grab,
};
use crate::platform_impl::{
    Fullscreen, MonitorHandle as PlatformMonitorHandle, OsError, PlatformIcon,
};
use crate::window::{
    Cursor, CursorGrabMode, ImePurpose, ResizeDirection, Theme, UserAttentionType,
    WindowAttributes, WindowButtons, WindowLevel,
};

use super::event_loop::sink::EventSink;
use super::output::MonitorHandle;
use super::state::WinitState;
use super::types::xdg_activation::XdgActivationTokenData;
use super::{ActiveEventLoop, WaylandError, WindowId};

/// Local patch (Drydock, PATCHES.md): whether the Wayland pop-up warning was already logged.
static POPUP_WARNED: AtomicBool = AtomicBool::new(false);

pub(crate) mod state;

use state::WindowRole;
pub use state::WindowState;

/// Local patch (Drydock, PATCHES.md): whether the fallback for a pop-up was already announced.
static POPUP_FALLBACK_WARNED: AtomicBool = AtomicBool::new(false);

/// The Wayland window.
pub struct Window {
    /// Reference to the underlying SCTK window or pop-up.
    window: WindowRole,

    /// Window id.
    window_id: WindowId,

    /// The state of the window.
    window_state: Arc<Mutex<WindowState>>,

    /// Local patch (Drydock, PATCHES.md): the pop-up's dismissed flag, shared with the state so
    /// reading it never needs the state lock.
    popup_dismissed: PopupDismissed,

    /// Compositor to handle WlRegion stuff.
    compositor: Arc<CompositorState>,

    /// The wayland display used solely for raw window handle.
    #[allow(dead_code)]
    display: WlDisplay,

    /// Xdg activation to request user attention.
    xdg_activation: Option<XdgActivationV1>,

    /// The state of the requested attention from the `xdg_activation`.
    attention_requested: Arc<AtomicBool>,

    /// Local patch (Drydock, PATCHES.md): the xdg-dialog-v1 object of a modal window.
    xdg_dialog: Option<XdgDialogV1>,

    /// Handle to the main queue to perform requests.
    queue_handle: QueueHandle<WinitState>,

    /// Local patch (Drydock, PATCHES.md): the shell, for repositioning a resized pop-up.
    xdg_shell: Arc<XdgShell>,

    /// Window requests to the event loop.
    window_requests: Arc<WindowRequests>,

    /// Observed monitors.
    monitors: Arc<Mutex<Vec<MonitorHandle>>>,

    /// Source to wake-up the event-loop for window requests.
    event_loop_awakener: calloop::ping::Ping,

    /// The event sink to deliver synthetic events.
    window_events_sink: Arc<Mutex<EventSink>>,
}

impl Window {
    pub(crate) fn new(
        event_loop_window_target: &ActiveEventLoop,
        mut attributes: WindowAttributes,
    ) -> Result<Self, RootOsError> {
        let queue_handle = event_loop_window_target.queue_handle.clone();
        let mut state = event_loop_window_target.state.borrow_mut();

        let monitors = state.monitors.clone();

        // Local patch (Drydock, PATCHES.md): a pop-up with a parent in this loop is a real
        // `xdg_popup`; otherwise it falls back to the undecorated toplevel below.
        if attributes.popup() {
            let parent = attributes.transient_parent.filter(|parent_id| {
                state
                    .windows
                    .get_mut()
                    .get(&parent_id.0)
                    .is_some_and(|window| !window.lock().unwrap().is_popup_dismissed())
            });
            if let Some(parent_id) = parent {
                match Self::new_popup(
                    event_loop_window_target,
                    &mut state,
                    &attributes,
                    parent_id.0,
                ) {
                    Ok(window) => return Ok(window),
                    Err(_) => Self::warn_popup_fallback(),
                }
            } else {
                Self::warn_popup_fallback();
            }

            attributes.decorations = false;
        }

        let surface = state.compositor_state.create_surface(&queue_handle);
        let compositor = state.compositor_state.clone();
        let xdg_activation =
            state.xdg_activation.as_ref().map(|activation_state| activation_state.global().clone());
        let display = event_loop_window_target.connection.display();

        let size: Size = attributes.inner_size.unwrap_or(LogicalSize::new(800., 600.).into());

        // Local patch (Drydock, PATCHES.md): pop-ups are not supported on Wayland yet; use a
        // normal undecorated toplevel instead (it keeps the transient parent).
        if attributes.popup && !POPUP_WARNED.swap(true, Ordering::Relaxed) {
            warn!("pop-up windows are not supported on Wayland yet; using an undecorated window");
        }
        let decorations = attributes.decorations && !attributes.popup;

        // We prefer server side decorations, however to not have decorations we ask for client
        // side decorations instead.
        let default_decorations = if decorations {
            WindowDecorations::RequestServer
        } else {
            WindowDecorations::RequestClient
        };

        let window =
            state.xdg_shell.create_window(surface.clone(), default_decorations, &queue_handle);

        let mut window_state = WindowState::new(
            event_loop_window_target.connection.clone(),
            &event_loop_window_target.queue_handle,
            &state,
            size,
            WindowRole::Toplevel(window.clone()),
            attributes.preferred_theme,
        );

        // Set transparency hint.
        window_state.set_transparent(attributes.transparent);

        window_state.set_blur(attributes.blur);

        // Set the decorations hint.
        window_state.set_decorate(decorations);

        // Set the app_id.
        if let Some(name) = attributes.platform_specific.name.map(|name| name.general) {
            window.set_app_id(name);
        }

        // Set the window title.
        window_state.set_title(attributes.title);

        // Set the min and max sizes. We must set the hints upon creating a window, so
        // we use the default `1.` scaling...
        let min_size = attributes.min_inner_size.map(|size| size.to_logical(1.));
        let max_size = attributes.max_inner_size.map(|size| size.to_logical(1.));
        window_state.set_min_inner_size(min_size);
        window_state.set_max_inner_size(max_size);

        // Non-resizable implies that the min and max sizes are set to the same value.
        window_state.set_resizable(attributes.resizable);

        // Local patch (Drydock, PATCHES.md): apply the requested title-bar buttons before the
        // first frame draw.
        window_state.set_enabled_buttons(attributes.enabled_buttons);

        // Set startup mode.
        match attributes.fullscreen.map(Into::into) {
            Some(Fullscreen::Exclusive(_)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland");
            },
            #[cfg_attr(not(x11_platform), allow(clippy::bind_instead_of_map))]
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.and_then(|monitor| match monitor {
                    PlatformMonitorHandle::Wayland(monitor) => Some(monitor.proxy),
                    #[cfg(x11_platform)]
                    PlatformMonitorHandle::X(_) => None,
                });

                window.set_fullscreen(output.as_ref())
            },
            _ if attributes.maximized => window.set_maximized(),
            _ => (),
        };

        match attributes.cursor {
            Cursor::Icon(icon) => window_state.set_cursor(icon),
            Cursor::Custom(cursor) => window_state.set_custom_cursor(cursor),
        }

        // Apply resize increments.
        if let Some(increments) = attributes.resize_increments {
            let increments = increments.to_logical(window_state.scale_factor());
            window_state.set_resize_increments(Some(increments));
        }

        // Activate the window when the token is passed.
        if let (Some(xdg_activation), Some(token)) =
            (xdg_activation.as_ref(), attributes.platform_specific.activation_token)
        {
            xdg_activation.activate(token.token, &surface);
        }

        // Local patch (Drydock, PATCHES.md): a dialog names the window it belongs to.
        if let Some(parent_id) = attributes.transient_parent {
            match state.windows.get_mut().get(&parent_id.0) {
                Some(parent) => {
                    let parent = parent.lock().unwrap();
                    if let Some(parent) = parent.window.toplevel() {
                        window.set_parent(Some(parent));
                    }
                },
                None => warn!("transient parent {parent_id:?} is not a window of this event loop"),
            }
        }

        // Local patch (Drydock, PATCHES.md): a modal dialog says so when the compositor offers
        // xdg-dialog-v1; otherwise the hint is dropped.
        let xdg_dialog = match (attributes.modal, state.xdg_dialog.as_ref()) {
            (true, Some(dialogs)) => {
                Some(dialogs.modal_dialog(window.xdg_toplevel(), &queue_handle))
            },
            _ => None,
        };

        // XXX Do initial commit.
        window.commit();

        // Add the window and window requests into the state.
        let window_state = Arc::new(Mutex::new(window_state));
        let window_id = super::make_wid(&surface);
        state.windows.get_mut().insert(window_id, window_state.clone());

        let window_requests = WindowRequests {
            redraw_requested: AtomicBool::new(true),
            closed: AtomicBool::new(false),
        };
        let window_requests = Arc::new(window_requests);
        state.window_requests.get_mut().insert(window_id, window_requests.clone());

        // Setup the event sync to insert `WindowEvents` right from the window.
        let window_events_sink = state.window_events_sink.clone();

        let mut wayland_source = event_loop_window_target.wayland_dispatcher.as_source_mut();
        let event_queue = wayland_source.queue();

        // Do a roundtrip.
        event_queue.roundtrip(&mut state).map_err(|error| {
            os_error!(OsError::WaylandError(Arc::new(WaylandError::Dispatch(error))))
        })?;

        // XXX Wait for the initial configure to arrive.
        while !window_state.lock().unwrap().is_configured() {
            event_queue.blocking_dispatch(&mut state).map_err(|error| {
                os_error!(OsError::WaylandError(Arc::new(WaylandError::Dispatch(error))))
            })?;
        }

        // Wake-up event loop, so it'll send initial redraw requested.
        let event_loop_awakener = event_loop_window_target.event_loop_awakener.clone();
        event_loop_awakener.ping();

        let popup_dismissed = window_state.lock().unwrap().popup_dismissed_flag();

        Ok(Self {
            window: WindowRole::Toplevel(window),
            display,
            monitors,
            window_id,
            compositor,
            window_state,
            popup_dismissed,
            queue_handle,
            xdg_shell: state.xdg_shell.clone(),
            xdg_activation,
            attention_requested: Arc::new(AtomicBool::new(false)),
            xdg_dialog,
            event_loop_awakener,
            window_requests,
            window_events_sink,
        })
    }

    fn warn_popup_fallback() {
        if !POPUP_FALLBACK_WARNED.swap(true, Ordering::Relaxed) {
            warn!(
                "a pop-up window needs a transient parent in this event loop; using an \
                 undecorated toplevel"
            );
        }
    }

    /// Local patch (Drydock, PATCHES.md): build the `xdg_positioner` for a placement.
    fn make_positioner(
        shell: &XdgShell,
        placement: &PopupPositioner,
    ) -> Result<XdgPositioner, RootOsError> {
        let positioner = XdgPositioner::new(shell)
            .map_err(|_| os_error!(OsError::Misc("failed to create an xdg_positioner")))?;
        positioner.set_size(placement.size.0, placement.size.1);
        positioner.set_anchor_rect(
            placement.anchor_rect.x,
            placement.anchor_rect.y,
            placement.anchor_rect.width,
            placement.anchor_rect.height,
        );
        positioner.set_anchor(Anchor::TopLeft);
        positioner.set_gravity(Gravity::BottomRight);
        positioner.set_offset(placement.offset.0, placement.offset.1);

        let mut adjustment = ConstraintAdjustment::empty();
        if placement.adjustment.slide_x {
            adjustment |= ConstraintAdjustment::SlideX;
        }
        if placement.adjustment.slide_y {
            adjustment |= ConstraintAdjustment::SlideY;
        }
        if placement.adjustment.flip_x {
            adjustment |= ConstraintAdjustment::FlipX;
        }
        if placement.adjustment.flip_y {
            adjustment |= ConstraintAdjustment::FlipY;
        }
        positioner.set_constraint_adjustment(adjustment);
        Ok(positioner)
    }

    /// Local patch (Drydock, PATCHES.md): create a pop-up as an `xdg_popup` of its parent.
    fn new_popup(
        event_loop_window_target: &ActiveEventLoop,
        state: &mut WinitState,
        attributes: &WindowAttributes,
        parent_id: WindowId,
    ) -> Result<Self, RootOsError> {
        let queue_handle = event_loop_window_target.queue_handle.clone();
        let display = event_loop_window_target.connection.display();
        let monitors = state.monitors.clone();

        let parent = state
            .windows
            .get_mut()
            .get(&parent_id)
            .cloned()
            .ok_or_else(|| os_error!(OsError::Misc("pop-up parent disappeared")))?;
        let (parent_surface, content_offset, parent_scale, parent_is_popup) = {
            let parent = parent.lock().unwrap();
            (
                parent.window.xdg_surface().clone(),
                parent.content_offset(),
                parent.scale_factor(),
                matches!(parent.window, WindowRole::Popup(_)),
            )
        };

        let initial_size: Size =
            attributes.inner_size.unwrap_or(LogicalSize::new(800., 600.).into());
        let logical_size: LogicalSize<u32> = initial_size.to_logical(parent_scale);
        let requested = attributes
            .position
            .map(|position| position.to_logical::<i32>(parent_scale))
            .unwrap_or(LogicalPosition::new(0, 0));
        let requested = (requested.x, requested.y);

        // Local patch (Drydock, PATCHES.md): a position under a parent pop-up is already in that
        // pop-up's coordinates. A root parent keeps today's content-relative positioner.
        let parent_origin = parent_is_popup.then_some((0, 0));

        let placement = popup_positioner_under(
            parent_origin,
            content_offset,
            requested,
            (logical_size.width as i32, logical_size.height as i32),
        );

        let positioner = Self::make_positioner(state.xdg_shell.as_ref(), &placement)?;

        let surface = state.compositor_state.create_surface(&queue_handle);
        let popup = Popup::from_surface(
            Some(&parent_surface),
            &positioner,
            &queue_handle,
            surface,
            state.xdg_shell.as_ref(),
        )
        .map_err(|_| os_error!(OsError::Misc("failed to create an xdg_popup")))?;

        let mut window_state = WindowState::new(
            event_loop_window_target.connection.clone(),
            &queue_handle,
            state,
            initial_size,
            WindowRole::Popup(popup.clone()),
            attributes.preferred_theme,
        );
        window_state.set_popup_placement(placement);
        let window_state = Arc::new(Mutex::new(window_state));
        let window_id = super::make_wid(popup.wl_surface());
        state.windows.get_mut().insert(window_id, window_state.clone());

        let window_requests = Arc::new(WindowRequests {
            redraw_requested: AtomicBool::new(true),
            closed: AtomicBool::new(false),
        });
        state.window_requests.get_mut().insert(window_id, window_requests.clone());

        // Local patch (Drydock, PATCHES.md): the xdg-shell grab rule. A pop-up may take a grab
        // only when its parent is the topmost pop-up (the root while the stack is empty).
        let grab_parent = parent_is_popup.then_some(parent_id);
        let parent_ok = state.popups.may_grab_under(grab_parent);
        state.popups.push(window_id);

        // Grab before the first commit, so the compositor sees it in the initial state.
        match state.seats.values().find_map(|seat| seat.popup_grab()) {
            Some((seat, serial)) if popup_should_grab(Some(serial), parent_ok) => {
                popup.xdg_popup().grab(&seat, serial);
            },
            Some(_) => debug!(
                "the parent pop-up is not topmost; creating the pop-up without a grab"
            ),
            None => debug!("no pointer button serial; creating the pop-up without a grab"),
        }
        popup.wl_surface().commit();

        let mut wayland_source = event_loop_window_target.wayland_dispatcher.as_source_mut();
        let event_queue = wayland_source.queue();
        event_queue.roundtrip(state).map_err(|error| {
            os_error!(OsError::WaylandError(Arc::new(WaylandError::Dispatch(error))))
        })?;

        // Local patch (Drydock, PATCHES.md): a compositor may dismiss the pop-up instead of
        // configuring it, so bound the wait and never block forever.
        let wait_started = Instant::now();
        loop {
            let (configured, dismissed) = {
                let window_state = window_state.lock().unwrap();
                (window_state.is_configured(), window_state.is_popup_dismissed())
            };
            match popup_configure_wait(
                configured,
                dismissed,
                wait_started.elapsed(),
                POPUP_CONFIGURE_TIMEOUT,
            ) {
                PopupConfigureWait::Wait => {
                    event_queue.flush().map_err(|error| {
                        os_error!(OsError::WaylandError(Arc::new(WaylandError::Wire(error))))
                    })?;
                    let remaining =
                        POPUP_CONFIGURE_TIMEOUT.saturating_sub(wait_started.elapsed());
                    if let Some(guard) = event_queue.prepare_read() {
                        let poll_fd =
                            PollFd::from_borrowed_fd(guard.connection_fd(), PollFlags::IN);
                        let timeout_ms = remaining.as_millis().min(i32::MAX as u128) as i32;
                        let _ = rustix::event::poll(&mut [poll_fd], timeout_ms);
                        let _ = guard.read();
                    }
                    event_queue.dispatch_pending(state).map_err(|error| {
                        os_error!(OsError::WaylandError(Arc::new(WaylandError::Dispatch(error))))
                    })?;
                },
                PopupConfigureWait::Configured => break,
                PopupConfigureWait::Dismissed => break,
                PopupConfigureWait::TimedOut => {
                    window_state.lock().unwrap().set_popup_dismissed(true);
                    let mut sink = state.window_events_sink.lock().unwrap();
                    sink.push_window_event(WindowEvent::Focused(false), window_id);
                    sink.push_window_event(WindowEvent::CloseRequested, window_id);
                    drop(sink);
                    if let Some(requests) =
                        state.window_requests.get_mut().get_mut(&window_id)
                    {
                        requests.closed.store(true, Ordering::Relaxed);
                    }
                    state.dispatched_events = true;
                    break;
                },
            }
        }

        let event_loop_awakener = event_loop_window_target.event_loop_awakener.clone();
        event_loop_awakener.ping();

        let popup_dismissed = window_state.lock().unwrap().popup_dismissed_flag();

        Ok(Self {
            window: WindowRole::Popup(popup),
            display,
            monitors,
            window_id,
            compositor: state.compositor_state.clone(),
            window_state,
            popup_dismissed,
            queue_handle,
            xdg_shell: state.xdg_shell.clone(),
            xdg_activation: None,
            attention_requested: Arc::new(AtomicBool::new(false)),
            xdg_dialog: None,
            event_loop_awakener,
            window_requests,
            window_events_sink: state.window_events_sink.clone(),
        })
    }

    pub(crate) fn xdg_toplevel(&self) -> Option<NonNull<c_void>> {
        self.window
            .xdg_toplevel()
            .map(|toplevel| NonNull::new(toplevel.id().as_ptr().cast()).unwrap())
    }
}

impl Window {
    #[inline]
    pub fn id(&self) -> WindowId {
        self.window_id
    }

    #[inline]
    pub fn set_title(&self, title: impl ToString) {
        let new_title = title.to_string();
        self.window_state.lock().unwrap().set_title(new_title);
    }

    #[inline]
    pub fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        None
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn set_outer_position(&self, _: Position) {
        // Not possible on Wayland.
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let window_state = self.window_state.lock().unwrap();
        let scale_factor = window_state.scale_factor();
        super::logical_to_physical_rounded(window_state.inner_size(), scale_factor)
    }

    #[inline]
    pub fn request_redraw(&self) {
        // Local patch (Drydock, PATCHES.md): a dismissed pop-up must not be drawn.
        if self.popup_dismissed.is_set() {
            return;
        }
        // NOTE: try to not wake up the loop when the event was already scheduled and not yet
        // processed by the loop, because if at this point the value was `true` it could only
        // mean that the loop still haven't dispatched the value to the client and will do
        // eventually, resetting it to `false`.
        if self
            .window_requests
            .redraw_requested
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            self.event_loop_awakener.ping();
        }
    }

    #[inline]
    pub fn pre_present_notify(&self) {
        // Local patch (Drydock, PATCHES.md): a dismissed pop-up must not be drawn.
        if self.popup_dismissed.is_set() {
            return;
        }
        let mut window_state = self.window_state.lock().unwrap();
        window_state.request_frame_callback();
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        let window_state = self.window_state.lock().unwrap();
        let scale_factor = window_state.scale_factor();
        super::logical_to_physical_rounded(window_state.outer_size(), scale_factor)
    }

    #[inline]
    pub fn request_inner_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let mut window_state = self.window_state.lock().unwrap();
        let old_size = window_state.inner_size();
        let new_size = window_state.request_inner_size(size);
        let new_logical = window_state.inner_size();

        // Local patch (Drydock, PATCHES.md): tell the compositor about a resized pop-up so its
        // slide and flip use the new size; otherwise a tall menu runs off the screen.
        if let WindowRole::Popup(popup) = &self.window {
            if let Some(placement) = window_state.popup_placement() {
                let old = (old_size.width as i32, old_size.height as i32);
                let new = (new_logical.width as i32, new_logical.height as i32);
                let version = self.xdg_shell.xdg_wm_base().version();
                if popup_resize_action(version, old, new) == PopupResize::Reposition {
                    if let Ok(positioner) = Self::make_positioner(
                        self.xdg_shell.as_ref(),
                        &PopupPositioner { size: new, ..placement },
                    ) {
                        let token = window_state.next_popup_reposition_token();
                        popup.reposition(&positioner, token);
                    }
                }
            }
        }

        drop(window_state);
        self.request_redraw();
        Some(new_size)
    }

    /// Set the minimum inner size for the window.
    #[inline]
    pub fn set_min_inner_size(&self, min_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let min_size = min_size.map(|size| size.to_logical(scale_factor));
        self.window_state.lock().unwrap().set_min_inner_size(min_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    /// Set the maximum inner size for the window.
    #[inline]
    pub fn set_max_inner_size(&self, max_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let max_size = max_size.map(|size| size.to_logical(scale_factor));
        self.window_state.lock().unwrap().set_max_inner_size(max_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    #[inline]
    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        let window_state = self.window_state.lock().unwrap();
        let scale_factor = window_state.scale_factor();
        window_state
            .resize_increments()
            .map(|size| super::logical_to_physical_rounded(size, scale_factor))
    }

    #[inline]
    pub fn set_resize_increments(&self, increments: Option<Size>) {
        let mut window_state = self.window_state.lock().unwrap();
        let scale_factor = window_state.scale_factor();
        let increments = increments.map(|size| size.to_logical(scale_factor));
        window_state.set_resize_increments(increments);
    }

    #[inline]
    pub fn set_transparent(&self, transparent: bool) {
        self.window_state.lock().unwrap().set_transparent(transparent);
    }

    #[inline]
    pub fn has_focus(&self) -> bool {
        self.window_state.lock().unwrap().has_focus()
    }

    #[inline]
    pub fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        None
    }

    #[inline]
    pub fn show_window_menu(&self, position: Position) {
        let scale_factor = self.scale_factor();
        let position = position.to_logical(scale_factor);
        self.window_state.lock().unwrap().show_window_menu(position);
    }

    #[inline]
    pub fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), ExternalError> {
        self.window_state.lock().unwrap().drag_resize_window(direction)
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        if self.window_state.lock().unwrap().set_resizable(resizable) {
            // NOTE: Requires commit to be applied.
            self.request_redraw();
        }
    }

    #[inline]
    pub fn is_resizable(&self) -> bool {
        self.window_state.lock().unwrap().resizable()
    }

    #[inline]
    pub fn set_enabled_buttons(&self, buttons: WindowButtons) {
        // Local patch (Drydock, PATCHES.md): store the choice and redraw when it changed.
        if self.window_state.lock().unwrap().set_enabled_buttons(buttons) {
            self.request_redraw();
        }
    }

    #[inline]
    pub fn enabled_buttons(&self) -> WindowButtons {
        // Local patch (Drydock, PATCHES.md).
        self.window_state.lock().unwrap().enabled_buttons()
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.window_state.lock().unwrap().scale_factor()
    }

    #[inline]
    pub fn set_blur(&self, blur: bool) {
        self.window_state.lock().unwrap().set_blur(blur);
    }

    #[inline]
    pub fn set_decorations(&self, decorate: bool) {
        self.window_state.lock().unwrap().set_decorate(decorate)
    }

    #[inline]
    pub fn is_decorated(&self) -> bool {
        self.window_state.lock().unwrap().is_decorated()
    }

    #[inline]
    pub fn set_window_level(&self, _level: WindowLevel) {}

    #[inline]
    pub(crate) fn set_window_icon(&self, _window_icon: Option<PlatformIcon>) {}

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        // You can't unminimize the window on Wayland.
        if !minimized {
            warn!("Unminimizing is ignored on Wayland.");
            return;
        }

        if let WindowRole::Toplevel(window) = &self.window {
            window.set_minimized();
        }
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.window_state
            .lock()
            .unwrap()
            .last_configure
            .as_ref()
            .map(|last_configure| last_configure.is_maximized())
            .unwrap_or_default()
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let WindowRole::Toplevel(window) = &self.window else {
            return;
        };
        if maximized {
            window.set_maximized()
        } else {
            window.unset_maximized()
        }
    }

    #[inline]
    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        let is_fullscreen = self
            .window_state
            .lock()
            .unwrap()
            .last_configure
            .as_ref()
            .map(|last_configure| last_configure.is_fullscreen())
            .unwrap_or_default();

        if is_fullscreen {
            let current_monitor = self.current_monitor().map(PlatformMonitorHandle::Wayland);
            Some(Fullscreen::Borderless(current_monitor))
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        match fullscreen {
            Some(Fullscreen::Exclusive(_)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland");
            },
            #[cfg_attr(not(x11_platform), allow(clippy::bind_instead_of_map))]
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.and_then(|monitor| match monitor {
                    PlatformMonitorHandle::Wayland(monitor) => Some(monitor.proxy),
                    #[cfg(x11_platform)]
                    PlatformMonitorHandle::X(_) => None,
                });

                if let WindowRole::Toplevel(window) = &self.window {
                    window.set_fullscreen(output.as_ref())
                }
            },
            None => {
                if let WindowRole::Toplevel(window) = &self.window {
                    window.unset_fullscreen()
                }
            },
        }
    }

    #[inline]
    pub fn set_cursor(&self, cursor: Cursor) {
        let window_state = &mut self.window_state.lock().unwrap();

        match cursor {
            Cursor::Icon(icon) => window_state.set_cursor(icon),
            Cursor::Custom(cursor) => window_state.set_custom_cursor(cursor),
        }
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        self.window_state.lock().unwrap().set_cursor_visible(visible);
    }

    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let xdg_activation = match self.xdg_activation.as_ref() {
            Some(xdg_activation) => xdg_activation,
            None => {
                warn!("`request_user_attention` isn't supported");
                return;
            },
        };

        // Urgency is only removed by the compositor and there's no need to raise urgency when it
        // was already raised.
        if request_type.is_none() || self.attention_requested.load(Ordering::Relaxed) {
            return;
        }

        self.attention_requested.store(true, Ordering::Relaxed);
        let surface = self.surface().clone();
        let data = XdgActivationTokenData::Attention((
            surface.clone(),
            Arc::downgrade(&self.attention_requested),
        ));
        let xdg_activation_token = xdg_activation.get_activation_token(&self.queue_handle, data);
        xdg_activation_token.set_surface(&surface);
        xdg_activation_token.commit();
    }

    pub fn request_activation_token(&self) -> Result<AsyncRequestSerial, NotSupportedError> {
        let xdg_activation = match self.xdg_activation.as_ref() {
            Some(xdg_activation) => xdg_activation,
            None => return Err(NotSupportedError::new()),
        };

        let serial = AsyncRequestSerial::get();

        let data = XdgActivationTokenData::Obtain((self.window_id, serial));
        let xdg_activation_token = xdg_activation.get_activation_token(&self.queue_handle, data);
        xdg_activation_token.set_surface(self.surface());
        xdg_activation_token.commit();

        Ok(serial)
    }

    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        self.window_state.lock().unwrap().set_cursor_grab(mode)
    }

    #[inline]
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
        let scale_factor = self.scale_factor();
        let position = position.to_logical(scale_factor);
        let result = self.window_state.lock().unwrap().set_cursor_position(position);
        // Request redraw on success, since the state is double buffered.
        result.map(|_| self.request_redraw())
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        self.window_state.lock().unwrap().drag_window()
    }

    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        let surface = self.window.wl_surface();

        if hittest {
            surface.set_input_region(None);
            Ok(())
        } else {
            let region = Region::new(&*self.compositor).map_err(|_| {
                ExternalError::Os(os_error!(OsError::Misc("failed to set input region.")))
            })?;
            region.add(0, 0, 0, 0);
            surface.set_input_region(Some(region.wl_region()));
            Ok(())
        }
    }

    #[inline]
    pub fn set_ime_cursor_area(&self, position: Position, size: Size) {
        let window_state = self.window_state.lock().unwrap();
        if window_state.ime_allowed() {
            let scale_factor = window_state.scale_factor();
            let position = position.to_logical(scale_factor);
            let size = size.to_logical(scale_factor);
            window_state.set_ime_cursor_area(position, size);
        }
    }

    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        let mut window_state = self.window_state.lock().unwrap();

        if window_state.ime_allowed() != allowed && window_state.set_ime_allowed(allowed) {
            let event = WindowEvent::Ime(if allowed { Ime::Enabled } else { Ime::Disabled });
            self.window_events_sink.lock().unwrap().push_window_event(event, self.window_id);
            self.event_loop_awakener.ping();
        }
    }

    #[inline]
    pub fn set_ime_purpose(&self, purpose: ImePurpose) {
        self.window_state.lock().unwrap().set_ime_purpose(purpose);
    }

    #[inline]
    pub fn focus_window(&self) {}

    #[inline]
    pub fn surface(&self) -> &WlSurface {
        self.window.wl_surface()
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        let data = self.window.wl_surface().data::<SurfaceData>()?;
        data.outputs().next().map(MonitorHandle::new)
    }

    #[inline]
    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        self.monitors.lock().unwrap().clone()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        // XXX there's no such concept on Wayland.
        None
    }

    #[cfg(feature = "rwh_04")]
    #[inline]
    pub fn raw_window_handle_rwh_04(&self) -> rwh_04::RawWindowHandle {
        let mut window_handle = rwh_04::WaylandHandle::empty();
        window_handle.surface = self.window.wl_surface().id().as_ptr() as *mut _;
        window_handle.display = self.display.id().as_ptr() as *mut _;
        rwh_04::RawWindowHandle::Wayland(window_handle)
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_window_handle_rwh_05(&self) -> rwh_05::RawWindowHandle {
        let mut window_handle = rwh_05::WaylandWindowHandle::empty();
        window_handle.surface = self.window.wl_surface().id().as_ptr() as *mut _;
        rwh_05::RawWindowHandle::Wayland(window_handle)
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        let mut display_handle = rwh_05::WaylandDisplayHandle::empty();
        display_handle.display = self.display.id().as_ptr() as *mut _;
        rwh_05::RawDisplayHandle::Wayland(display_handle)
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_window_handle_rwh_06(&self) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        Ok(rwh_06::WaylandWindowHandle::new({
            let ptr = self.window.wl_surface().id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        })
        .into())
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        })
        .into())
    }

    #[inline]
    pub fn set_theme(&self, theme: Option<Theme>) {
        self.window_state.lock().unwrap().set_theme(theme)
    }

    #[inline]
    pub fn theme(&self) -> Option<Theme> {
        self.window_state.lock().unwrap().theme()
    }

    pub fn set_content_protected(&self, _protected: bool) {}

    #[inline]
    pub fn title(&self) -> String {
        self.window_state.lock().unwrap().title().to_owned()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // Local patch (Drydock, PATCHES.md): drop the modal hint before the toplevel goes.
        if let Some(dialog) = self.xdg_dialog.take() {
            dialog.destroy();
        }
        self.window_requests.closed.store(true, Ordering::Relaxed);
        self.event_loop_awakener.ping();
    }
}

/// The request from the window to the event loop.
#[derive(Debug)]
pub struct WindowRequests {
    /// The window was closed.
    pub closed: AtomicBool,

    /// Redraw Requested.
    pub redraw_requested: AtomicBool,
}

impl WindowRequests {
    pub fn take_closed(&self) -> bool {
        self.closed.swap(false, Ordering::Relaxed)
    }

    pub fn take_redraw_requested(&self) -> bool {
        self.redraw_requested.swap(false, Ordering::Relaxed)
    }
}

impl TryFrom<&str> for Theme {
    type Error = ();

    /// ```
    /// use winit::window::Theme;
    ///
    /// assert_eq!("dark".try_into(), Ok(Theme::Dark));
    /// assert_eq!("lIghT".try_into(), Ok(Theme::Light));
    /// ```
    fn try_from(theme: &str) -> Result<Self, Self::Error> {
        if theme.eq_ignore_ascii_case("dark") {
            Ok(Self::Dark)
        } else if theme.eq_ignore_ascii_case("light") {
            Ok(Self::Light)
        } else {
            Err(())
        }
    }
}
