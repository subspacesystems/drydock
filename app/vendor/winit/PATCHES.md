# Local changes to winit 0.30.13

This directory is the crates.io source of winit 0.30.13 (git e9809ef), wired in with
`[patch.crates-io]` in `app/.cargo/config.toml` and the repository's `.cargo/config.toml`.
The published `Cargo.lock` is left out (cargo ignores it in a dependency). Only the
changes below differ from the published crate; each is marked
`Local patch (Drydock, PATCHES.md)` in the source. Keep this list complete when you change
anything here, and carry it forward when winit is upgraded.

Two groups of changes: modal dialogs, then dialog parents.

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
- X11: not mapped yet.
