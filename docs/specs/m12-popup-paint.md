# m12-popup-paint: pop-up menus appear fully painted and square

On Windows a menu is seen drawing itself: a black rectangle grows, then the light menu fills in; the
rounded corners are black and a black line sometimes crosses the top. Two causes in
`popup_window_window` (app/crates/drydock/src/popup.rs): the window is created at a 120 x 40 placeholder,
made visible while an `InnerSize` command is still in flight (so it is shown at the wrong size and grows),
and the content is `egui::Frame::menu` (rounded corners and a shadow) inside a rectangular opaque window,
so the corners and the shadow margin show the clear color (black).

## Behavior

1. **Size before show.** A window pop-up is created hidden. The size it is created at is `initial_size`
   of the content size remembered from the last opening under the same title (keep it in egui temp data
   under a key that `forget_state` does NOT clear; store the `fit_size` result each frame), or the
   placeholder when none. Every frame, measure the content, compute `fit_size`, and ask `show_step`.
2. **Show after a paint.** Drive visibility with `show_step(visible, current, painted, target, frames_hidden)`:
   - `Resize(t)`: send `ViewportCommand::InnerSize(t)`, clear `painted`, request a repaint.
   - `Wait`: remember `painted = current` (this frame is the painted one) and request a repaint.
   - `Reveal`: send `ViewportCommand::Visible(true)` and set the visible flag. Never before a paint.
   - `Keep`: nothing.
   Keep `visible`, `painted` and `frames_hidden` in egui temp data keyed by the viewport id and clear
   them in `forget_state`. Build the viewport with `with_visible(false)` always (not the old `shown`
   flag), at `initial_size(..)` and `popup.position`. The existing focus, Escape, close and arrow-key
   handling stays as it is. `current` is the viewport's `inner_rect` size, or `initial_size(..)` when unknown.
3. **Square and filled.** Window pop-ups use `popup_frame(ui.style())` (square, no shadow, 1 px stroke,
   menu fill) instead of `Frame::menu`. Before the panel, paint the whole viewport rectangle (`ui.max_rect()` of the viewport's root `Ui`, i.e. `popup_fill_rect(..)` of it) with the frame's fill via `ui.painter().rect_filled(rect, 0.0, fill)`, so no clear color shows at fractional scales; the 1 px stroke is drawn inside that rectangle. The inline pop-up keeps `Frame::menu` (tests, unsupported platforms).
4. **Windows** (code under `#[cfg(windows)]`, must pass the oracle's `winclippy` gate): in the patched
   winit (`app/vendor/winit/src/platform_impl/windows/window.rs`, where `with_popup` / `WS_POPUP`
   windows are created):
   - turn off the DWM open/close animation for pop-ups: `DwmSetWindowAttribute` with
     `DWMWA_TRANSITIONS_FORCEDISABLED` set to 1 (add the constant import if the windows-sys feature set
     lacks it; use the numeric value 3 as a `const` if needed);
   - ask for small rounded corners: `DWMWA_WINDOW_CORNER_PREFERENCE` = `DWMWCP_ROUNDSMALL` (3), which also
     gives the native shadow on Windows 11; ignore the error on Windows 10 (the corners stay square);
   - to avoid a black first flash, make sure the window is created hidden and shown only through `Visible(true)`: check the pop-up path calls neither `ShowWindow(SW_SHOW...)` at creation nor shows when `visible` is false.
   Record the changes in `app/vendor/winit/PATCHES.md`. `app/vendor/egui-winit` only if the visibility
   command is dropped for pop-ups (check `Visible(true)` on a window created hidden shows it without
   activating it; keep no-activate behavior).
5. **Submenus and context menus** use `popup_window`, so they get all of this. Do not change their
   positions. Keep every existing popup / menu test green: `popup_window`, `menu_popups`, `submenu_popups`,
   `menus_*`, `menu_close_outside`, `popup_wayland_on`, and the real-window ones.

## Tests

`app/crates/drydock/tests/popup_paint.rs` (already written, failing on the `unimplemented!` skeletons):
`popup_frame`, `popup_fill_rect`, `initial_size`, `show_step`. Implement those four functions as their
doc comments say and use them in `popup_window_window`. The real-window check (X11: the first visible
pop-up window already has its final size, no growth between shots) is made by the sign-off in the real
window, and Tyrel checks the Windows look.

## Where to look

- `popup_window_window`, `popup_window`, `forget_state`, `fit_size`, `popup_viewport_id` in
  app/crates/drydock/src/popup.rs; the new skeletons `popup_frame`, `popup_fill_rect`, `initial_size`,
  `ShowStep`, `show_step` there.
- `crate::dialog::needs_resize` (1 px tolerance).
- `egui::Frame::menu`, `egui::CentralPanel`, `ViewportCommand::{InnerSize, Visible}`,
  `ViewportBuilder::with_visible`.
- Callers: `top_menu` and the menu contents in app/crates/drydock/src/chrome.rs;
  app/crates/drydock/src/context_menu.rs.
- winit: `show app/vendor/winit` Windows `window.rs` around `DwmSetWindowAttribute` (line ~1100,
  `DWMWA_WINDOW_CORNER_PREFERENCE` is used for the window-corner attribute), `with_popup`, `WS_POPUP`.
- Tests: app/crates/drydock/tests/popup_paint.rs, popup_window.rs, menu_popups.rs, submenu_popups.rs.
