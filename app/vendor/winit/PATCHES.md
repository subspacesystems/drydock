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
- `src/platform_impl/linux/wayland/window/mod.rs` (`Window::new`): a pop-up logs a warning once
  and is created as a normal undecorated toplevel (no `xdg_popup` yet).
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
  and `resolve_popup_position` (models the compositor's flip/slide rules for the tests).
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
  seat and latest button serial for the grab.
- `src/window.rs` (`WindowAttributes::with_popup` docs): the Wayland line describes the
  `xdg_popup`, the input grab and the content-relative position.
