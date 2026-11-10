# m12-popup-window: a pop-up as its own window

Menus (m12-menu-popups) need a window with no title bar that opens at a screen position,
may reach past the main window's edge, fits its content and goes away when the user clicks
elsewhere or presses Escape. This task writes that helper in `crates/drydock/src/popup.rs`
(skeleton on main) and nothing else: no menus. It follows `dialog_window` (M11) closely.

## What to build

Fill in every `unimplemented!` in `popup.rs`; the doc comments there are the contract.

- **`popup_viewport_id(title)`**: `ViewportId::from_hash_of(("drydock-popup", title))`.
- **`popup_supported_on(os, wayland)`** / **`popup_supported()`**: Windows and X11 yes, Wayland
  and other systems no. `popup_supported()` reads `std::env::consts::OS` and the environment
  (Wayland when `WAYLAND_DISPLAY` is non-empty and `WINIT_UNIX_BACKEND` is not `x11`). The
  Wayland task (m12-winit-popup-wayland) will flip the Wayland answer later; keep the rule in
  this one place.
- **`popup_mode(embed, supported)`**: `Window` only if `!embed && supported`.
- **`fit_size(content, monitor)`**: ceil each side, at least 1.0, at most the monitor size.
- **`inline_position(root_inner, position, size, content_rect)`**: see the doc comment. Subtract
  `root_inner.min` when known; then clamp so the rectangle fits `content_rect` (right/bottom
  first, then the top-left corner wins, so a pop-up larger than the area starts at its corner).
- **`popup_window`**:
  - `!*open`: draw nothing and forget any state kept for the pop-up (as `dialog_window` does with
    `forget_opening_position`), so the next opening starts fresh.
  - **Inline**: a foreground `egui::Area` (id from the title), at `inline_position` using the
    area's size from the previous frame (zero on the first), content in
    `egui::Frame::menu(ui.style())`. `Escape` closes (consume the key). A primary press whose
    position is outside the area's rect closes too; a press inside does not. The area rect is
    `PopupOutcome::rect` (inline coordinates are fine there).
  - **Window**: call `egui_winit::mark_popup_title(ctx, title)` every frame before the
    viewport is shown. Build the viewport with `with_title`, `with_decorations(false)`,
    `with_resizable(false)`, `with_minimize_button(false)`, `with_maximize_button(false)`,
    `with_taskbar(false)`, `with_position(position)`, a small first `with_inner_size` (e.g.
    `vec2(120.0, 40.0)`), and `crate::window_icon::with_app_identity` like the dialogs. Show it
    with `show_viewport_immediate`, content in a `CentralPanel` with a menu-style frame. Measure
    the content (as `dialog_window_impl` does) and send `ViewportCommand::InnerSize(fit_size(
    content + frame margin, monitor_size))` when it differs from the window's inner size
    (use `dialog::needs_resize`). Keep it hidden until the first measurement if that avoids a
    flash (as `dialog_window_impl` does with its "shown" flag), but it must be visible within
    a few frames. Close (`*open = false`) on Escape, on `close_requested`, and on focus loss:
    remember in `ctx.data` that the viewport has been focused (`viewport().focused == Some(true)`)
    and close when it later reports `Some(false)`. A pop-up that never got focus is not closed
    by this rule (a window manager may not focus it).
  - `PopupOutcome::rect`: the pop-up's outer/inner rectangle in screen points when
    `ViewportInfo` has it, else `None`.

Do not touch `app/vendor`, `app.rs` or any menu code.

## Tests (already written; do not edit)

`tests/popup_window.rs`:
- pure: viewport id, platform support, mode choice, `fit_size`, `inline_position`.
- kittest (viewports embedded, so inline): content shown at the position without a title bar;
  moved inside when the position is off-screen; nothing while closed; Escape closes; a press
  outside closes; a press inside does not.
- real window (Linux, needs `DRYDOCK_GUI_TESTS=1`, Xvfb, xdotool, xprop; skips itself on Wayland
  or without them): an eframe probe app opens a pop-up at screen (250, 200) over a 320x240 root
  at (0, 0); the test finds it as a separate X window, checks its position, that it reaches
  past the root, that it is sized to its small content, `_NET_WM_WINDOW_TYPE_POPUP_MENU`,
  `WM_TRANSIENT_FOR` set, Escape closes it, and after reopening, moving focus to the root closes
  it. The window-type and transient checks depend on the winit patch from `m12-winit-popup`
  (merged before this task starts).

## Where to look

- `show dialog_window_impl`, `show dialog_window`, `show needs_resize`, `show fit_height`,
  `show opening_position`, `show forget_opening_position` (`dialog.rs`): the pattern.
- `show floating_panel_window` (`floating.rs`): real window vs embedded, `root_inner`.
- `show mark_popup_title`, `show is_popup_title` (`app/vendor/egui-winit/src/lib.rs`), and
  `show with_popup` (`app/vendor/winit/src/window.rs`): from m12-winit-popup.
- `show with_app_identity` (`window_icon.rs`).
- `tests/dialog_window.rs` and `tests/dialog_modal_x11.rs`: the harness and probe-app patterns
  the new test copies.
