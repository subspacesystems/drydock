# Local changes to winit 0.30.13

This directory is the crates.io source of winit 0.30.13 (git e9809ef), wired in with
`[patch.crates-io]` in `app/.cargo/config.toml` and the repository's `.cargo/config.toml`.
The published `Cargo.lock` is left out (cargo ignores it in a dependency). Only the
changes below differ from the published crate; each is marked
`Local patch (Drydock, PATCHES.md)` in the source. Keep this list complete when you change
anything here, and carry it forward when winit is upgraded.

Five groups of changes: modal dialogs, dialog parents, pop-up windows, event loop recreation,
then enabled title-bar buttons.

## Modal dialogs

A prompt tells the desktop that it has to be answered before going back to the window it
belongs to (its transient parent, below).

- `src/window.rs`: `WindowAttributes` gets a `modal: bool` field, `WindowAttributes::with_modal`
  and the getter `WindowAttributes::modal`.
- `src/platform_impl/linux/wayland/types/xdg_dialog.rs` (new), `types/mod.rs` and `state.rs`:
  binds the xdg-dialog-v1 global `xdg_wm_dialog_v1` when the compositor offers it
  (`WinitState::xdg_dialog`).
- `src/platform_impl/linux/wayland/window/mod.rs` (`Window::new`, `Drop for Window`): a modal
  window gets an `xdg_dialog_v1` with `set_modal` before the first commit, destroyed when the
  window is dropped. Without the global nothing is sent.
- `src/platform_impl/windows/window.rs` (`init`, `Drop for Window`): a modal window with an owner
  disables the owner (`EnableWindow`) when it is created and enables it again when it is dropped,
  before it is destroyed.
- X11: in `src/platform_impl/linux/x11/window.rs` (`UnownedWindow::new`) a modal window adds
  `_NET_WM_STATE_MODAL` to the `_NET_WM_STATE` property before it is mapped; the atom is defined
  in `src/platform_impl/linux/x11/atoms.rs`.

## Dialog parents

Goal: a dialog or tool window tells the desktop which window it belongs to, so the desktop
keeps it above that window and places it over it. winit's own `with_parent_window` embeds a
child window instead (and is unsupported on Wayland), so it is not reused for this.

- `src/window.rs`: `WindowAttributes` gets a `transient_parent: Option<WindowId>` field,
  `WindowAttributes::with_transient_parent` and the getter `WindowAttributes::transient_parent`.
- `src/platform_impl/linux/wayland/window/mod.rs` (`Window::new`): when the parent is a window
  of the same event loop, `xdg_toplevel.set_parent` (sctk `Window::set_parent`) before the
  first commit; otherwise a warning and no parent.
- `src/platform_impl/windows/window.rs` (`init`): the transient parent becomes the owner window,
  as `WindowAttributesExtWindows::with_owner_window` does; an explicit owner wins.
- `src/platform_impl/linux/x11/window.rs` (`UnownedWindow::new`): `WM_TRANSIENT_FOR` is set to
  the parent X window id, and a still-default `Normal` window type becomes `Dialog`.

## Pop-up windows

Goal: a menu is a real OS window that can reach past the main window's edge.

- `src/window.rs`: `WindowAttributes` gets a `popup: bool` field,
  `WindowAttributes::with_popup` and the getter `WindowAttributes::popup`.
- X11: in `src/platform_impl/linux/x11/window.rs` (`UnownedWindow::new`) a pop-up gets the
  `PopupMenu` window type (unless the caller chose another one), `_NET_WM_STATE_SKIP_TASKBAR`
  and `_NET_WM_STATE_SKIP_PAGER` before mapping, and no decorations; the two state atoms are
  defined in `src/platform_impl/linux/x11/atoms.rs`.
- `src/platform_impl/windows/window.rs` (`init`) and
  `src/platform_impl/windows/window_state.rs` (`WindowFlags::to_window_styles`): a pop-up gets
  `WS_POPUP` without caption, border or thick frame, plus
  `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`; the owner comes from the transient parent.
- `src/platform_impl/linux/wayland/window/mod.rs` (`Window::new`): a pop-up that has no usable
  parent or grab falls back to a normal undecorated toplevel. When it can, it is a real
  `xdg_popup` (see the Wayland pop-up section below).
- X11: `UnownedWindow` remembers `popup`, and `focus_window` on a pop-up also sets the X input
  focus to it (`SetInputFocus`, revert to parent) after the `_NET_ACTIVE_WINDOW` request, so a
  menu the application opens from one of its focused windows gets the keyboard even without a
  window manager (`m12-menu-focus-loss`).

## Event loop recreation

Goal: a test binary can run more than one real-window test, each starting its own eframe
application after the previous one has finished. The event loop can still only be created
once at a time.

- `src/event_loop.rs`: `EventLoop` holds an `EventLoopRecreationGuard` whose `Drop` clears
  `EVENT_LOOP_CREATED`, so a later `EventLoopBuilder::build` succeeds once the previous event
  loop is gone.

## X display connection retry

Goal: a test that starts its window just as the shared Xvfb resets (it resets when its last
client leaves) connects instead of exiting at once with `XOpenDisplayFailed`.

- `src/platform_impl/linux/x11/xdisplay.rs` (`XConnection::new`): retries `XOpenDisplay` for
  about a second before reporting the server as missing.

## Enabled title-bar buttons

Goal: a window can ask for no Minimize/Maximize button. On GNOME Wayland the client-side
Adwaita frame draws the title bar, and winit previously ignored `WindowAttributes::enabled_buttons`
and left the `Window::set_enabled_buttons`/`enabled_buttons` methods as TODO no-ops.

- `src/platform_impl/linux/wayland/window/state.rs`: `WindowState` stores `enabled_buttons:
  WindowButtons` (default all) with `enabled_buttons` and `set_enabled_buttons`; the setter
  updates the Adwaita frame and returns whether it changed. When the frame is created on
  configure, the stored resizable and button state are applied to it.
- `src/platform_impl/linux/wayland/window/mod.rs` (`Window::new`): applies
  `attributes.enabled_buttons` before the first frame draw.
  `Window::set_enabled_buttons` stores the value, updates the frame and requests a redraw when
  it changed; `Window::enabled_buttons` returns the stored value.
- X11 is unchanged: Mutter draws those frames and drops the buttons for DIALOG-typed windows.

## Pop-up windows

A window with `WindowAttributes::with_popup` is a menu. X11 and Windows support was added by
`m12-winit-popup`; on Wayland it is now a real `xdg_popup` instead of an undecorated toplevel.

- `src/platform/wayland.rs`: new `PopupRect`, `PopupAdjustment` and `PopupPositioner` types plus
  the pure functions `popup_positioner` (builds the positioner from a content-relative request)
  and `resolve_popup_position` (models the compositor's flip/slide rules for the tests). Also
  `PopupGrabInput`/`PopupGrabSerial` (tracks the latest button or key *press* serial, since
  mutter rejects a grab made with a release serial) and `PopupConfigureWait` /
  `popup_configure_wait` / `POPUP_CONFIGURE_TIMEOUT` (the bounded wait for the first configure).
- `src/platform_impl/linux/wayland/window/mod.rs` (`Window::new`, new `Window::new_popup`,
  `Window::warn_popup_fallback`, `Window::xdg_toplevel`, `Window::set_minimized`,
  `Window::set_maximized`, `Window::set_fullscreen`): a pop-up with a transient parent in the
  same event loop gets its own `wl_surface` and an `xdg_popup` with a positioner and an input
  grab from the parent's latest button serial; without a usable parent or grab it falls back to
  the warning and undecorated toplevel. The `m12-popup-window` helper must pass a position
  relative to the parent's content area on Wayland, not screen coordinates.
- `src/platform_impl/linux/wayland/window/state.rs` (`WindowRole`, `WindowState::new`,
  `WindowState::configure_popup`, `WindowState::content_offset`, `WindowState::is_configured`,
  `WindowState::is_decorated`, and the toplevel-only setters): stores either the toplevel or the
  pop-up, applies the compositor's pop-up size, and reports the parent's client-side title-bar
  offset for the anchor rect. Pop-ups have no frame, decorations or title.
- `src/platform_impl/linux/wayland/state.rs` (`PopupHandler for WinitState`,
  `delegate_xdg_popup!`): `configure` applies the chosen size and requests a redraw; `done`
  reports `Focused(false)` and `CloseRequested` so the application closes the pop-up like on X11.
- `src/platform_impl/linux/wayland/seat/mod.rs` (`WinitSeatState::popup_grab`): supplies the
  seat and latest press serial for the grab, from a `PopupGrabSerial` kept per seat. The pointer
  and keyboard handlers feed it presses and releases (`latest_button_serial` is unchanged for
  move, resize and the window menu).
- `src/platform_impl/linux/wayland/window/mod.rs` (`Window::new_popup`, `Window::request_redraw`,
  `Window::pre_present_notify`): the wait for the first configure is bounded and really times
  out; on `Dismissed` or `TimedOut` it stops, marks the pop-up dismissed and reports
  `Focused(false)`/`CloseRequested`, and a dismissed pop-up never gets a redraw or a frame
  callback. `Window` keeps a clone of the shared `PopupDismissed` flag, so `request_redraw` reads
  the flag without the `window_state` lock; it must never need that lock (the state's
  `request_inner_size` calls it while holding the guard).
- `src/platform_impl/linux/wayland/window/state.rs` (`WindowState::is_popup_dismissed`,
  `WindowState::set_popup_dismissed`, `WindowState::popup_dismissed_flag`): a `PopupDismissed`
  flag over an `Arc<AtomicBool>`, shared with the window handle, remembers that the pop-up was
  dismissed before its first configure, so no buffer is attached to it.
- `src/platform_impl/linux/wayland/state.rs` (`PopupHandler::done`): also marks the window
  dismissed, besides the focus loss and close it already reports.
- `src/window.rs` (`WindowAttributes::with_popup` docs): the Wayland line describes the
  `xdg_popup`, the input grab and the content-relative position.
- Windows (`src/platform_impl/windows/window.rs`): a pop-up is created hidden (shown only by
  `ViewportCommand::Visible(true)`), its DWM open/close transitions are disabled
  (`DWMWA_TRANSITIONS_FORCEDISABLED`) and it asks for `DWMWCP_ROUNDSMALL` corners
  (`m12-popup-paint`).

## Pop-up stacking, placement and reposition (`m12-wayland-popup-stack`)

Goal: GNOME accepts a grabbing pop-up only when its parent is the topmost pop-up; a submenu is
positioned relative to its menu and a pop-up that grows is repositioned so the compositor's
slide/flip use the new size. X11 and Windows are unchanged.

- `src/platform/wayland.rs`: `popup_positioner_under` (positioner relative to the parent
  pop-up or, under the root, exactly `popup_positioner`). Under a parent pop-up the request is
  already in that pop-up's coordinates, so the anchor is the one-pixel rect at the request; only
  root-parented pop-ups add the title-bar offset. New `PopupStack` (`push`, `remove`,
  `topmost`, `may_grab_under`) and `popup_should_grab`. New `POPUP_REPOSITION_VERSION`,
  `PopupResize` and `popup_resize_action`.
- `src/platform_impl/linux/wayland/window/mod.rs`: `Window::new` rejects a dismissed transient
  parent (falls back to the existing toplevel warning). New `Window::make_positioner`; it is
  used for creation and for repositioning. `Window::new_popup` accepts a pop-up parent: since a
  pop-up-parented position is in the parent pop-up's own points, it positions with
  `popup_positioner_under(Some(_), ..)` without walking any chain, records the placement on the
  window state, applies the grab rule (`popup_should_grab` with `PopupStack::may_grab_under`)
  and pushes the new pop-up on the stack. `Window::request_inner_size` repositions a resized
  pop-up with a positioner of the new size built from the same placement and a fresh token.
- `src/platform_impl/linux/wayland/window/state.rs`: `WindowState` stores the pop-up's creation
  `popup_placement` and the next reposition token; `configure_popup` applies the compositor's
  size. Getters `popup_placement` and setters `set_popup_placement`,
  `next_popup_reposition_token`.
- `src/platform_impl/linux/wayland/state.rs`: `WinitState` keeps `popups: PopupStack<WindowId>`
  and its `xdg_shell` is now shared in an `Arc` so a window can make a positioner. `PopupHandler`
  `configure` passes the configured position to `configure_popup`; `done` removes the pop-up
  from the stack.
- `src/platform_impl/linux/wayland/event_loop/mod.rs`: a closed or dropped window is removed
  from the pop-up stack when it is destroyed.
