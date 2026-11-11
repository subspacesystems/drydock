# m12-wayland-popup-hang: Wayland pop-ups grab with a press serial and never hang

Tyrel, GNOME on Ubuntu: clicking any menu title freezes Drydock completely.

## Cause

A Wayland pop-up (`xdg_popup`) is grabbed with `xdg_popup.grab(seat, serial)`. Drydock passes the
serial of the latest button event, and sctk records that on press *and* release. The menu titles
open on the click (release), so the serial is the release's. mutter accepts a grab only with the
serial of a button *press* (or key press); it answers `xdg_popup.popup_done` at once and never
configures the pop-up. `Window::new_popup` then loops in `blocking_dispatch` until the first
configure, which never comes: the main thread hangs.

## What to build

The two decisions are pure and already declared as skeletons in
`vendor/winit/src/platform/wayland.rs` (bodies `unimplemented!`); the tests in
`app/crates/drydock/tests/popup_wayland_grab.rs` pin them. Implement them, then wire them in.

1. **`PopupGrabSerial`**: `record` keeps the serial of the latest *press* (`ButtonPress`,
   `KeyPress`), whichever kind came last; releases are ignored; a serial of 0 is ignored.
   `serial()` is `None` until a press has been recorded.
2. **`popup_configure_wait`**: returns `Dismissed` if dismissed (even if also configured), else
   `Configured` if configured, else `TimedOut` if `elapsed >= timeout`, else `Wait`.
   `POPUP_CONFIGURE_TIMEOUT` stays 1 s.

## Wiring (not unit-testable here; there is no compositor in CI)

- **Press serials**: keep a `PopupGrabSerial` per seat in `WinitSeatState`. Feed it from winit's
  pointer handler (`PointerEventKind::Press { serial }` / `Release { serial }`, where
  `latest_button_serial` is set today) and from the keyboard handler's `WlKeyboardEvent::Key`
  (pressed and released, with its `serial`). `WinitSeatState::popup_grab` returns the seat and
  `serial()` instead of `latest_button_serial()`; with no press seen it returns `None`, as now.
  Leave the other users of `latest_button_serial` (move, resize, window menu) alone.
- **Dismissal**: add a `popup_dismissed` flag to the window state with a setter and getter.
  `PopupHandler::done` (wayland/state.rs) sets it, in addition to what it does now (focus-lost,
  `CloseRequested`, `requests.closed`).
- **The wait** in `Window::new_popup`: loop on `popup_configure_wait(...)` with the elapsed time
  since the loop started. `Wait`: dispatch events without blocking forever (use a bounded
  dispatch, e.g. `dispatch_pending` plus a short `prepare_read`/poll or `blocking_dispatch` only
  while the remaining time is positive; the timeout must really fire even if no event arrives).
  `Configured`: continue as today. `Dismissed` or `TimedOut`: stop waiting; on `TimedOut` set the
  dismissed flag and push the same `Focused(false)` and `CloseRequested` that `done` does, and
  set `requests.closed`.
- **Never attach a buffer to a pop-up that was dismissed before its first configure** (a protocol
  error that kills the connection). The window object is still returned, stays unmapped, and
  draws nothing: make sure redraw / present on a dismissed pop-up is skipped. The app then gets
  `CloseRequested` and `popup.rs` closes the menu.
- The wait stays: attaching a buffer before the first configure is also a protocol error.
- **`vendor/winit/PATCHES.md`**: describe the change in the Wayland pop-up section, and fix the
  older line that still says "no `xdg_popup` yet".

## Done when

- `run-tests -p drydock --test popup_wayland_grab` and the existing
  `popup_positioner_wayland` tests pass; clippy and the oracle's gates are green.
- The orchestrator's sign-off checks the real behavior in a headless mutter (the grab uses the
  press serial, the pop-up gets `xdg_surface.configure` and a buffer, the app stays responsive);
  workers cannot run that.

## Where to look

- `popup_configure_wait`, `PopupGrabSerial`, `PopupGrabInput`, `POPUP_CONFIGURE_TIMEOUT`,
  `PopupConfigureWait` (`vendor/winit/src/platform/wayland.rs`)
- `Window::new_popup` (`wayland/window/mod.rs`): the grab and the first-configure loop
- `WindowState::is_configured`, `WindowState::configure_popup` (`wayland/window/state.rs`)
- `PopupHandler::done` (`wayland/state.rs`)
- `WinitSeatState::popup_grab` (`wayland/seat/mod.rs`)
- the `PointerEventKind::Press` / `Release` arm in `wayland/seat/pointer/mod.rs`; the
  `WlKeyboardEvent::Key` arms in `wayland/seat/keyboard/mod.rs`
- test helpers: `serial_after` in `crates/drydock/tests/popup_wayland_grab.rs`
