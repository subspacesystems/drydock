# m11-winit-parent: dialog windows name the main window as their parent on X11

Milestone M11. Part of the local windowing patch: every egui viewport other than
the root (a dialog or tool window) tells the desktop that it belongs to the main window, so the
desktop keeps it above the main window and opens it over it.

## What is already on main

- `app/vendor/winit` is winit 0.30.13 and `app/vendor/egui-winit` is egui-winit 0.36.2, copied
  from crates.io and wired in with `[patch.crates-io]` (`app/.cargo/config.toml`). Their
  changes are listed in each directory's `PATCHES.md` and marked
  `Local patch (Drydock, PATCHES.md)` in the source.
- winit: `WindowAttributes` has a new field `transient_parent: Option<WindowId>` with
  `WindowAttributes::with_transient_parent` and the getter `WindowAttributes::transient_parent`
  (`app/vendor/winit/src/window.rs`). Wayland and Windows already use it.
- egui-winit: `create_window` remembers the first window it makes for an `egui::Context` (the
  root viewport's) and passes its id to every later window through `with_transient_parent`.
- **X11 ignores the field.** That is this task.

## What to do

Only one source file changes: `app/vendor/winit/src/platform_impl/linux/x11/window.rs`, in
`UnownedWindow::new`, where it now calls
`window.set_window_types(window_attrs.platform_specific.x11.x11_window_types)`.

When `window_attrs.transient_parent` is `Some(parent)`:

1. Set the `WM_TRANSIENT_FOR` property of the new window (`window.xwindow`) to the parent's X
   window id. Type `WINDOW`, format 32, one value, mode replace. On X11 a winit `WindowId` is the
   X window id: `u64::from(parent)` converted to `xproto::Window`. Use the connection's
   `change_property` helper (as the surrounding code does for `WM_CLASS`) and ignore the error
   the same way (`leap!(...).ignore_error()`).
2. If the window types are still the default, exactly `[WindowType::Normal]`, set
   `[WindowType::Dialog]` instead. Any other list the caller asked for (for example
   `[WindowType::Utility]` through `with_x11_window_type`) is kept as it is.

Then set the window types as before (one `set_window_types` call, with the possibly changed
list). When `transient_parent` is `None`, nothing changes: no `WM_TRANSIENT_FOR`, the types the
caller gave.

Mark the new code with a `// Local patch (Drydock, PATCHES.md): ...` comment, like the Wayland
change in `wayland/window/mod.rs`, and replace the line `- X11: not mapped yet.` in
`app/vendor/winit/PATCHES.md` with one bullet saying what X11 now does (file and function, the
property, the Dialog default).

Keep the diff small: no reformatting of the vendored file, no other changes in `app/vendor`.
`cargo fmt` does not touch `app/vendor`, so format your lines by hand in the file's style.

## Acceptance tests

`app/crates/drydock/tests/dialog_parent_x11.rs`:

- `transient_parent_attribute_round_trips` checks the attribute API (already passes).
- `dialog_viewports_name_the_main_window_as_transient_parent` runs a small eframe app on X11 in
  a test thread: a root window, one immediate viewport and one deferred viewport that asks for
  the Utility type. It finds the windows by title with `xdotool` and reads `WM_TRANSIENT_FOR`
  and `_NET_WM_WINDOW_TYPE` with `xprop`. Expected: the root has no transient parent and type
  NORMAL; both viewports are transient for the root; the immediate one has type DIALOG, the
  deferred one keeps UTILITY. It skips itself unless `DRYDOCK_GUI_TESTS=1` and `DISPLAY` are
  set (both are, in the worker and the oracle, under Xvfb).

Run them with `cargo test -p drydock --test dialog_parent_x11`. The rest of the workspace tests
must still pass.

## Where to look

- `UnownedWindow::new` in `app/vendor/winit/src/platform_impl/linux/x11/window.rs` (the
  `set_window_types` call; the `WM_CLASS` `change_property` call just above it).
- `UnownedWindow::set_window_types`, same file.
- `XConnection::change_property` in
  `app/vendor/winit/src/platform_impl/linux/x11/util/window_property.rs`.
- `WindowType` in `app/vendor/winit/src/platform/x11.rs` (derives `PartialEq`).
- `WindowAttributes::with_transient_parent` and `WindowAttributes::transient_parent` in
  `app/vendor/winit/src/window.rs`; `impl From<WindowId> for u64` in the same file.
- The Wayland version of the same change, for style: `Window::new` in
  `app/vendor/winit/src/platform_impl/linux/wayland/window/mod.rs`.
- `create_window` and `root_window_id` in `app/vendor/egui-winit/src/lib.rs` (read only).
- `app/vendor/winit/PATCHES.md`.
