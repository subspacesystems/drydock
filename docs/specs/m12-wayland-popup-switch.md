# m12-wayland-popup-switch: submenu parent mark and menu switching order

Sign-off 4, app side of `m12-wayland-popup-stack`. On Wayland a grabbing pop-up must be a
child of the topmost pop-up, and the compositor refuses a new grab while the previous menu's
pop-up still exists. Two changes in the app (nothing in winit):

## 1. A submenu names its menu as parent

- `PopupWindow.parent` (popup.rs) is now used: in `popup_window_window`, when `popup.parent`
  is `Some(parent)`, call `egui_winit::mark_popup_parent(ctx, popup.title, parent)` right
  after `mark_popup_title`.
- `submenu_button` (chrome.rs) sets `parent: Some(&menu_popup_title(<open menu label>))`.
  The top-level menus and `context_menu.rs` keep `parent: None`.

## 2. Switching menus destroys the old pop-up first

Applies only when pop-ups are windows and the session is Wayland (`popup_mode_in(ctx) ==
Window && wayland_in(ctx)`); X11, Windows and inline behaviour must not change.

- `menu_switch_step(shown, open, wayland_window)`: `HideThenShow` when `wayland_window` and
  `shown` is `Some` other menu than `open`; otherwise `Show`.
- `shown_menu(ctx)` holds the menu whose pop-up window was shown on the last frame. Set in
  `top_menu` after `popup_window` ran for its label; cleared when the open menu becomes `None`
  (`set_open_menu(None)`), and when the switch below runs.
- In `top_menu`, for the open label, before showing anything: if `menu_switch_step` says
  `HideThenShow`, then this frame show no pop-up for this label: clear the submenu
  (`clear_submenu`, which closes a submenu's pop-up first), send `ViewportCommand::Close` to the
  old menu's viewport (`popup_viewport_id(menu_popup_title(old))`), clear the old pop-up's state
  (`forget_state` in popup.rs, make it `pub(crate)` if needed), clear `shown_menu`, call
  `ctx.request_repaint()` and return. The next frame finds `shown_menu` empty and opens the new
  menu normally.
- This covers hover over another title and Left/Right (`request_menu_step` and the arrow
  keys in `menu_bar_with_toolbars`), from the main window or a pop-up, because they all end in
  `set_open_menu(Some(new))`.
- Replace the direct `popup_mode(ctx.embed_viewports(), popup_supported())` and
  `wayland_session()` calls in chrome.rs and popup.rs with `popup_mode_in(ctx)` and
  `wayland_in(ctx)` (already on main), so the test hook reaches them. They behave as before
  when the hook is off.

## Real check (worker may skip, orchestrator does it)

The mutter harness: View > Toolbars > Drawing, then hover File to Edit and ArrowRight. Not
needed for the tests.

## Tests

`app/crates/drydock/tests/popup_switch.rs` (failing now; do not edit). It forces the Wayland
path with `force_wayland_windows`. Keep `menu_popups`, `submenu_popups`, `menu_close_outside`,
`menu_focus_loss` and the `popup_window*` tests green. The parent-mark test needs the winit
side from `m12-wayland-popup-stack` (already merged when this task starts).

## Where to look

- popup.rs: `PopupWindow`, `popup_window`, `popup_window_window`, `forget_state`,
  `popup_viewport_id`, `popup_mode_in`, `wayland_in`, `force_wayland_windows`.
- chrome.rs (give yourself only these ranges via `show`): `top_menu`, `submenu_button`,
  `clear_submenu`, `set_open_menu`, `open_menu`, `menu_popup_title`, `submenu_popup_title`,
  `request_menu_step`, `menu_switch_step`, `shown_menu`, `MenuSwitch`; the arrow-key block in
  `menu_bar_with_toolbars` (`outline` it).
- egui-winit: `mark_popup_parent`, `popup_parent_title`.
- tests/popup_switch.rs, tests/menu_popups.rs (`harness`, `shows`).
