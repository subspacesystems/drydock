# m12-winit-popup: a pop-up window kind in the windowing patch (X11, Windows)

Milestone M12. Menus are to become real OS windows that can reach past the main
window's edge. This task adds the window kind to the local winit / egui-winit patch; the app
helper (`m12-popup-window`) and Wayland (`m12-winit-popup-wayland`) come later.

## What is already on main

- `WindowAttributes::with_popup(bool)` and `WindowAttributes::popup()` in
  `app/vendor/winit/src/window.rs` (field `popup`, default false). Nothing reads it yet.
- `egui_winit::mark_popup_title(ctx, title)` and `egui_winit::is_popup_title(ctx, title)` in
  `app/vendor/egui-winit/src/lib.rs`, bodies `unimplemented!()`. They work like
  `mark_modal_title` / `is_modal_title` (same storage idea, **own key**, so the two kinds of
  marks stay separate).
- From M11: `transient_parent` (egui-winit sets it to the root window for every later window)
  and `modal`.

## What to do

1. **egui-winit** (`src/lib.rs`): implement the two functions with a new temp-data key
   (`popup_titles_key`). In `create_window`, a window for a viewport whose title is marked gets
   `.with_popup(true)`, only when `root` is `Some` (the root viewport is never a pop-up, even
   when its title is marked). Update the comment above that code.
2. **X11** (`src/platform_impl/linux/x11/window.rs`, `UnownedWindow::new`, next to the
   transient-parent and modal code). For `window_attrs.popup`:
   - window types: `[WindowType::PopupMenu]` (add the variant if winit lacks it; it maps to
     `_NET_WM_WINDOW_TYPE_POPUP_MENU`, the atom may need defining in `atoms.rs`), unless the
     caller asked for a list other than the default `[Normal]`, which is kept. The M11 rule
     "default Normal becomes Dialog when there is a transient parent" must not win over popup:
     popup is decided first.
   - `_NET_WM_STATE` gets `_NET_WM_STATE_SKIP_TASKBAR` and `_NET_WM_STATE_SKIP_PAGER` before
     the window is mapped, in the same property write as the modal / other states (each atom
     listed once, also when `with_modal(true)` is set as well).
   - no decorations: treat `decorations` as false for the window (`set_decorations_inner(false)`
     at creation), whatever the attributes said.
   - `WM_TRANSIENT_FOR` still comes from the transient parent (M11 code, unchanged).
   - position: the requested position is the window's top-left corner on the screen;
     check that `with_position` already gives that for an undecorated window and do nothing if so.
   Windows without `popup` must be exactly as before.
3. **Windows** (`src/platform_impl/windows/window.rs`, `init`, and the style helper that builds
   the style flags): a popup window gets `WS_POPUP` (no caption, border or thick frame) and
   `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`; the owner comes from the transient parent as in M11.
   Not testable here: keep it small, make it compile on the Windows target as far as the
   existing code allows (`cargo check --target x86_64-pc-windows-gnu` if the toolchain has it,
   otherwise read carefully), and name the exact functions in PATCHES.md.
4. **Wayland** (`src/platform_impl/linux/wayland/window/mod.rs`, `Window::new`): for a popup,
   log a `warn!` once ("pop-up windows are not supported on Wayland yet; using an undecorated
   window") and create a normal toplevel with decorations off (it keeps the transient parent
   from M11). No `xdg_popup`; that is `m12-winit-popup-wayland`.
5. **Focus loss**: nothing to add. winit already sends `WindowEvent::Focused(false)` and egui
   reports it as `viewport().focused == false`; the app closes the pop-up on that and on Escape
   (`m12-popup-window`). Mention this in the doc comment of `with_popup` if not there.
6. **PATCHES.md** (winit and egui-winit): add a "Pop-up windows" group to winit's (one bullet per
   file and function, as for the modal group; update the "Four groups" sentence) and a bullet to
   egui-winit's.

Mark every change `// Local patch (Drydock, PATCHES.md): ...`. Keep the diff small; `cargo fmt`
does not touch `app/vendor`, so format by hand in the file's style. No other changes in
`app/vendor`.

## Acceptance tests

`app/crates/drydock/tests/popup_window_x11.rs` (do not edit):

- `popup_attribute_round_trips` (already passes).
- `marked_popup_titles_are_remembered_per_context`: the two egui-winit functions.
- `a_marked_viewport_is_a_popup_and_the_others_are_not`: a real eframe app on X11 under Xvfb
  (root, one marked immediate viewport asked to open at screen (320, 210) with decorations left
  on, one plain deferred viewport). With `xprop` / `xdotool` it expects, for the pop-up:
  type `_NET_WM_WINDOW_TYPE_POPUP_MENU`, `SKIP_TASKBAR` and `SKIP_PAGER` once each,
  `WM_TRANSIENT_FOR` the root window, `_MOTIF_WM_HINTS` with no decorations, top-left within 2 px
  of (320, 210); and for the plain viewport and the root: no pop-up type, no skip-taskbar, the
  plain one keeps its decorations, the root has no transient parent. It skips itself unless
  `DRYDOCK_GUI_TESTS=1` and `DISPLAY` are set (both are in the worker and the oracle).

Run: `cargo test -p drydock --test popup_window_x11`. Also the existing
`dialog_parent_x11` and `dialog_modal_x11` tests, and the rest of the workspace, must pass.

## Where to look

- `UnownedWindow::new`, `UnownedWindow::set_window_types`, `set_decorations_inner`,
  `XConnection::change_property` in `app/vendor/winit/src/platform_impl/linux/x11/` (`window.rs`,
  `util/window_property.rs`); the M11 modal state code in the same function.
- `WindowType` in `app/vendor/winit/src/platform/x11.rs`; atoms in `x11/atoms.rs`.
- `WindowAttributes::with_popup`, `WindowAttributes::with_modal` in `app/vendor/winit/src/window.rs`.
- `create_window`, `mark_modal_title`, `is_modal_title`, `modal_titles_key`, `mark_popup_title`
  in `app/vendor/egui-winit/src/lib.rs`.
- Windows: `init` and the window-style code in `app/vendor/winit/src/platform_impl/windows/window.rs`.
- Wayland: `Window::new` in `app/vendor/winit/src/platform_impl/linux/wayland/window/mod.rs`.
- The test helpers `find_window`, `xprop`, `position`, `motif_undecorated` in the test file.
- `app/vendor/winit/PATCHES.md`, `app/vendor/egui-winit/PATCHES.md`.
