# m11-winit-modal: a modal dialog window says so on X11

Milestone M11. Part of the local windowing patch: a prompt window (one the user has
to answer before going back to the main window) tells the desktop that it is modal.

## What is already on main

- `app/vendor/winit` (winit 0.30.13) and `app/vendor/egui-winit` (egui-winit 0.36.2) are local
  copies wired in with `[patch.crates-io]`. Their changes are listed in each directory's
  `PATCHES.md` and marked `Local patch (Drydock, PATCHES.md)` in the source.
- winit: `WindowAttributes` has a field `modal: bool` with `WindowAttributes::with_modal` and the
  getter `WindowAttributes::modal` (`app/vendor/winit/src/window.rs`). Wayland (xdg-dialog-v1)
  and Windows (the owner window is disabled) already use it.
- egui-winit: `mark_modal_title(ctx, title)` marks a viewport title; `create_window` gives every
  later window made for a viewport with that title `with_modal(true)` (never the root window).
- Task `m11-winit-parent` (merged before this one starts) makes X11 set `WM_TRANSIENT_FOR` and
  the Dialog window type in the same function you change.
- **X11 ignores `modal`.** That is this task.

## What to do

1. `app/vendor/winit/src/platform_impl/linux/x11/atoms.rs`: add the atom `_NET_WM_STATE_MODAL`
   to the atom list, next to the other `_NET_WM_STATE_*` atoms.
2. `app/vendor/winit/src/platform_impl/linux/x11/window.rs`, `UnownedWindow::new`: when
   `window_attrs.modal` is true, put `_NET_WM_STATE_MODAL` into the new window's
   `_NET_WM_STATE` property before the window is mapped: the connection's `change_property`
   on `window.xwindow`, property `atoms[_NET_WM_STATE]`, type `ATOM`
   (`xproto::Atom::from(xproto::AtomEnum::ATOM)`), mode `APPEND`, one value
   `atoms[_NET_WM_STATE_MODAL]`. Ignore the error the way the surrounding code does
   (`leap!(...).ignore_error()`). Put it right after the window types are set
   (`set_window_types`). A window that is not modal gets no change.

   Why a property and not a message: the window manager reads `_NET_WM_STATE` from the
   property when the window is first mapped; the client message winit uses elsewhere
   (`set_netwm`) only works for windows that are already mapped. Do not send a message.
3. Mark the new code with a `// Local patch (Drydock, PATCHES.md): ...` comment, like the other
   local changes.
4. `app/vendor/winit/PATCHES.md`, section "Modal dialogs": replace the line
   `- X11: modal not mapped yet.` with one bullet saying what X11 now does (file and function,
   the property and atom, set before mapping). Also name `atoms.rs`.

Keep the diff small: no reformatting of the vendored files, no other changes in `app/vendor`.
`cargo fmt` does not touch `app/vendor`, so format your lines by hand in the file's style
(4 spaces, lines under 100 characters).

## Acceptance tests

`app/crates/drydock/tests/dialog_modal_x11.rs`:

- `modal_attribute_round_trips` and `marked_titles_are_remembered_per_context` check the API
  that is already on main (they pass now).
- `a_marked_viewport_is_modal_and_the_others_are_not` runs a small eframe app on X11 in a test
  thread: a root window, an immediate viewport whose title is marked modal, and an unmarked
  deferred viewport. The root's title is marked too, after the root window exists. The test
  finds the windows by title with `xdotool` and reads `_NET_WM_STATE` with `xprop`. Expected:
  the marked viewport lists `_NET_WM_STATE_MODAL` exactly once; the other viewport and the root
  do not list it. It skips itself unless `DRYDOCK_GUI_TESTS=1` and `DISPLAY` are set (both are,
  in the worker and the oracle, under Xvfb).

Run them with `cargo test -p drydock --test dialog_modal_x11`. The rest of the workspace tests,
including `dialog_parent_x11`, must still pass.

## Where to look

- `UnownedWindow::new` in `app/vendor/winit/src/platform_impl/linux/x11/window.rs` (the
  `set_window_types` call, and the `WM_CLASS` `change_property` call above it for the style).
- `UnownedWindow::set_netwm`, same file (read only: shows `atoms[_NET_WM_STATE]`).
- The atom list in `app/vendor/winit/src/platform_impl/linux/x11/atoms.rs`
  (`_NET_WM_STATE_MAXIMIZED_VERT`).
- `XConnection::change_property` in
  `app/vendor/winit/src/platform_impl/linux/x11/util/window_property.rs`.
- `WindowAttributes::with_modal` and `WindowAttributes::modal` in
  `app/vendor/winit/src/window.rs`.
- `create_window` and `mark_modal_title` in `app/vendor/egui-winit/src/lib.rs` (read only).
- `app/vendor/winit/PATCHES.md`.
