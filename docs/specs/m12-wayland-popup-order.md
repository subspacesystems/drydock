# m12-wayland-popup-order: pop-ups go in order on Wayland

Sign-off 5 on GNOME found two ordering bugs in the Wayland pop-ups. The vendored winit is
right (a grab only when the parent is the topmost pop-up); both bugs are in the app
(`chrome.rs`, `popup.rs`). Everything below applies only when `popup_mode_in(ctx) == Window`
and `wayland_in(ctx)`; X11, Windows and inline mode must not change.

How eframe sees pop-ups: it creates a window when a viewport id first appears in a frame's
`FullOutput::viewport_output` and destroys it on the first frame it is missing. egui drops a
child viewport (the submenu) from the output at the end of its *parent's* pass (the menu's
pop-up) when the parent's pass did not use it; if the parent's pass does not run at all, the
child stays one frame longer. So a menu pop-up missing from a frame while its submenu is still
listed is the crash case.

## 1. Closing a menu with its submenu shown: the submenu goes first

`wl_display.error(xdg_wm_base, 2, "destroyed popup not top most popup")` today, for a click on
a Toolbars item, Escape in the submenu pop-up, a click outside, focus loss, Exit, and switching
menus by hover. (Left/Right do nothing while a submenu is open, and Escape in the main window
only closes the submenu first; both are already fine and stay as they are.)

Rule: on the frame that would first drop the menu's pop-up while the submenu's pop-up was shown
on the previous frame, `top_menu` keeps showing the menu's pop-up (its rows in a disabled ui, so
input is ignored; its close/focus/Escape results are ignored) and shows no submenu. The submenu's
pop-up is thereby missing from that frame's output with the menu's still present. The next frame
drops the menu as today. The submenu being dropped means `submenu_button` returns before
`popup_window` (it already does when `open_submenu` is `None`). Do it in `top_menu`, where the
`open_menu(&ctx) != Some(label)` early return is: track per frame (use `ctx.cumulative_pass_nr()`
or the existing frame counters) when each menu's pop-up and when a submenu's pop-up were last
shown, in new `Id` keys beside `SHOWN_MENU` (do not reuse `SHOWN_MENU`: `set_open_menu(None)`
clears it). One extra frame, never more: the frame after a keep-showing frame has no submenu
shown on its previous frame, so it drops.

## 2. Switching menus: the old pop-up is gone before the new one is created

The hover path creates the new pop-up in the same frame the old is destroyed (File's
`xdg_popup.destroy` 34 ms after Edit's `get_popup`), so Edit gets no grab and its submenu then
grabs with a non-grabbing parent. Cause (found by reading, confirm it): on a hover switch the
new title's `top_menu` runs *inside the frame in which the old menu's pop-up was already shown*
(File's `top_menu` ran earlier with `open_menu` still File). The `HideThenShow` branch then
clears `shown_menu` in that very frame, so the next frame goes straight to `Show` and
`viewport_output` goes from "File" to "Edit" with no frame between. Left/Right set the menu at
the end of the frame, so the next frame already has the old one missing and works.

Required: some frame has neither menu's pop-up between the last with the old and the first with
the new. Suggested: in the `HideThenShow` branch, when the old menu's pop-up was shown in this
same pass, or its submenu was shown on the last pass (so its `top_menu` will keep showing it
under rule 1 in this frame or the next), do nothing yet (no `Close`, no `forget_state`, keep
`shown_menu`, `request_repaint`, return); a later frame then finds the old one dropped and runs
the hide step as today, and the frame after shows the new one. `menu_switch_step` keeps its
signature and its tests (`popup_switch.rs`, `menu_popups.rs`, `submenu_popups.rs`,
`menu_close_outside.rs`, `popup_wayland_on.rs`, `menu_focus_loss.rs` stay green; you may not
edit them).

## Tests

`app/crates/drydock/tests/popup_order.rs` (failing now; do not edit). It runs the menu bar on a
context with `embed_viewports(false)` and `force_wayland_windows`, a renderer that runs each
pop-up's callback as a pass of its own viewport (as eframe does), and records the pop-up ids in
each frame's `viewport_output`. It covers the submenu-first order for Escape in the submenu
pop-up, a click in main, focus loss and hover to another title; the gap for hover and Right
switches; and that X11 mode still closes the menu on the click's own frame. Main now has the
test hooks `chrome::request_submenu` and `chrome::menu_title_rect`.

Not covered by tests (check by reading): an item clicked in the submenu (`submenu_button`'s
`closed` branch, then `set_open_menu(None)`), Exit, and Escape in the View pop-up itself; they
all end with the menu closed while the submenu was shown, so rule 1 covers them. The
orchestrator reruns the GNOME harness at sign-off.

## Where to look

- chrome.rs: `top_menu`, `submenu_button`, `clear_submenu`, `set_open_menu`, `shown_menu`,
  `menu_switch_step`, `MenuSwitch`, `popup_rows`, `close_menus_from_root`, the `MENU_STEP`
  handling at the end of `menu_bar_inner` (`outline` it), test hooks `request_submenu`,
  `menu_title_rect`.
- popup.rs: `popup_window`, `popup_window_window`, `forget_state`, `popup_viewport_id`,
  `popup_mode_in`, `wayland_in`, `force_wayland_windows`.
- tests/popup_order.rs: `Rig`, `assert_child_first`, `assert_gap`; tests/popup_switch.rs for the
  older switch tests.
