# m12-wayland-startup-hang: Wayland windows no longer deadlock at startup

Sign-off 4: on Wayland a fresh build never shows its window. The main thread is stuck in
`Window::request_redraw` waiting for a mutex.

## Cause

`m12-wayland-popup-hang` made `Window::request_redraw` (and `pre_present_notify`) lock
`window_state` to read the pop-up's dismissed flag. `Window::request_inner_size` already holds
that lock (`let mut window_state = self.window_state.lock()...`) when it calls
`self.request_redraw()`, and `std::sync::Mutex` is not reentrant, so creating any window
(egui-winit calls `request_inner_size` while building it) deadlocks. `Window::set_cursor_position`
has the same shape: the guard lives through `.map(|_| self.request_redraw())`.

## What to build

1. **`PopupDismissed`** (`vendor/winit/src/platform/wayland.rs`, skeleton there, tests in
   `app/crates/drydock/tests/popup_dismissed_flag.rs`): a cheap clonable flag over
   `Arc<AtomicBool>`. `new()` is unset, `set(bool)` stores with `Release`, `is_set()` loads with
   `Acquire`; every clone shares the one flag.
2. **State side** (`wayland/window/state.rs`): `WindowState.popup_dismissed` becomes a
   `PopupDismissed`. `is_popup_dismissed` / `set_popup_dismissed` keep their signatures and use
   it. Add `WindowState::popup_dismissed_flag(&self) -> PopupDismissed` returning a clone.
3. **Window side** (`wayland/window/mod.rs`): `Window` gets a `popup_dismissed: PopupDismissed`
   field, set from the state's flag in both constructors (`Window::new` and `Window::new_popup`;
   take the clone *before* building `Self { .. }` so no guard is alive while `window_state` is
   moved). `request_redraw` and `pre_present_notify` read that field and never lock
   `window_state` just to check it (`pre_present_notify` still locks to call
   `request_frame_callback`, but only after the check, and does not call `request_redraw`).
4. **Audit**: go through every `self.request_redraw()` in that file and make sure no
   `window_state` guard is alive at the call. Fix `request_inner_size` (take the result in one
   statement so the guard drops, then redraw) and `set_cursor_position` (bind the result of the
   locked call first, then `.map(|_| self.request_redraw())`). Check the `set_min_inner_size`,
   `set_max_inner_size`, `set_resizable`, `set_enabled_buttons` calls too.
5. **`app/vendor/winit/PATCHES.md`**: update the `m12-wayland-popup-hang` entry (the
   `Window::request_redraw` / `pre_present_notify` / `state.rs` bullets) to describe the shared
   flag and add a line that `request_redraw` must never need the `window_state` lock.

The real check (headless compositor) is done at sign-off; there is no Wayland compositor in CI,
so the tests cover the shared flag and the existing `popup_wayland_grab` tests must stay green.

## Where to look

- `PopupDismissed`, `popup_configure_wait` in `app/vendor/winit/src/platform/wayland.rs`
- `Window::request_redraw`, `Window::pre_present_notify`, `Window::request_inner_size`,
  `Window::set_cursor_position`, `Window::new`, `Window::new_popup` in
  `app/vendor/winit/src/platform_impl/linux/wayland/window/mod.rs`
- `WindowState::is_popup_dismissed`, `WindowState::set_popup_dismissed`, `WindowState::new` in
  `.../wayland/window/state.rs`; the other user is `PopupHandler::done` in `.../wayland/state.rs`
- Tests: `app/crates/drydock/tests/popup_dismissed_flag.rs`, `popup_wayland_grab.rs`
