# m12-menu-focus-loss

Two fixes to the pop-up menu close rules (`chrome::close_menus_from_root`, from m12-menu-close-outside). Windows pop-ups never take focus, so neither path changes behavior there.

## 1. Focus loss closes the menu on time

Today the menus close once the main window has been unfocused for `FOCUS_GRACE` (0.25 s), but nothing schedules a frame: egui repaints only on input, so a menu left open when another app takes focus stays until the next pointer motion.

- Implement `chrome::root_close_repaint_after(&RootInput) -> Option<f64>` (skeleton on main): `Some(FOCUS_GRACE - unfocused_for)` when `menu_open`, `unfocused_for` is `Some(t)` with `t < FOCUS_GRACE`, and `popup_has_focus` is false; `None` otherwise (also at exactly `FOCUS_GRACE` or later).
- In `close_menus_from_root` (Window mode, a menu open, not the embedded path), after building the `RootInput`, if the helper returns `Some(secs)`, call `ctx.request_repaint_after_for(Duration::from_secs_f64(secs), ViewportId::ROOT)`. Do it before the early return on `RootClose::Nothing`.

## 2. An arrow key in a focused pop-up keeps the new menu open

ArrowRight/Left in a focused pop-up (popup.rs calls `chrome::request_menu_step`) switches menus, but the old pop-up is destroyed, nothing has focus, and after the grace the new menu closes.

- In `menu_bar_inner`, when a step comes from the `MENU_STEP` request (not from keys in the main window) and the new menu is set, record a "step pending focus" marker in the context data with the current time, and send `ViewportCommand::Focus` to the new menu's pop-up viewport (`popup::popup_viewport_id(&menu_popup_title(label))`) via `ctx.send_viewport_cmd_to`. Send it again on later frames while the marker is pending, since the pop-up may not exist on the first.
- Constant `pub const STEP_FOCUS_WINDOW: f64 = 0.5` (already on main). In `close_menus_from_root`, treat the menus as `popup_has_focus` while the marker is younger than `STEP_FOCUS_WINDOW` and the new menu's pop-up has not yet reported focus. Clear the marker when that pop-up reports focus (its `popup-has-focus` data is true) or the window passes, or the menu closes.
- Keep the marker out of `RootInput`; fold it into the `popup_has_focus` value passed in, so `root_close` and its tests stay as they are.
- Arrow keys pressed in the main window (no pop-up step) set no marker.

## Tests

- `tests/menu_close_outside.rs`: two pure tests of the helper (written), the existing tests, and the extended real-window probe (skips unless DRYDOCK_GUI_TESTS=1 and X11): loses X focus entirely, the menu closes within 1 s with no input; Right in a focused File pop-up leaves a drydock-menu-Edit window that is still there 1 s later. Do not run it on this host; the oracle runs it.
- Keep menu_close_outside, menu_popups, submenu_popups, popup_window and popup_wayland_on passing.

## Where to look

`show close_menus_from_root`, `show root_close`, `show RootInput`, `show root_close_repaint_after`, `show menu_bar_inner`, `show request_menu_step`, `show menu_popup_title`, `show popup_viewport_id`, `show popup_window` (the focus block that writes `popup-has-focus`), `show FOCUS_GRACE`; test helpers `input`, `bar_harness`, and `mod real` in `tests/menu_close_outside.rs`.
