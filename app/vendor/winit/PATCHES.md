# Local changes to winit 0.30.13

This directory is the crates.io source of winit 0.30.13 (git e9809ef), wired in with
`[patch.crates-io]` in `app/.cargo/config.toml` and the repository's `.cargo/config.toml`.
The published `Cargo.lock` is left out (cargo ignores it in a dependency). Only the
changes below differ from the published crate; each is marked
`Local patch (Drydock, PATCHES.md)` in the source. Keep this list complete when you change
anything here, and carry it forward when winit is upgraded.

Four groups of changes: modal dialogs, dialog parents, event loop recreation, then enabled
title-bar buttons.

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
