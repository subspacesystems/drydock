# m12-menu-popups: top-level menus as pop-up windows

Today File, Edit, View, Tools and Help are egui `ui.menu_button` menus: areas inside the main
window, so they are clipped at its edge (M10 made them scroll to fit instead). They become
pop-ups made with `popup_window` (`popup.rs`, from m12-popup-window): their own OS windows
below the title where the platform allows, inline areas otherwise (UI tests, Wayland until
m12-winit-popup-wayland). The task list in the planning notes names Camera, Draw and Window
menus; this app has none (their rows live in View and Tools), so the five above are all.
Submenus other than the interim below, and context menus, are m12-submenu-popups.

## What to build (all in `chrome.rs` unless noted)

Skeletons on main: `menu_popup_title`, `menu_popup_position`, `open_menu`, `request_menu`.

- **State**: the open menu's label lives in `ctx.data` (temp), so `open_menu(ctx)` reads it and
  `request_menu(ctx, label)` sets it (only for the five known labels; others change nothing).
- **`menu_popup_title(label)`**: a distinct string per label, e.g. `format!("drydock-menu-{label}")`.
- **`menu_popup_position(root_inner, title)`**: `title.left_bottom()` plus `root_inner.min`
  when known (the pop-up's position is in root screen points, see `PopupWindow::position`).
- **`top_menu`** is rewritten: it draws the title as a button in the bar (same look as the
  `menu_button` title now: highlighted while its menu is open or hovered; accessible label =
  the title text, so tests find it with `get_by_label`). Remember the title's rect in `ctx.data`
  (by label) so `request_menu` and the arrow keys can place a menu whose title was not clicked.
  - Click on a title: open that menu, or close it when it is the open one. A title click that
    follows the focus-loss close of the same menu (the window pop-up closes when the main
    window takes focus, before the click is seen) must not reopen it: remember when and which
    menu closed and ignore a click on that title for ~0.3 s.
  - Hover on another title while a menu is open: switch to it. With nothing open, hover opens
    nothing. In window mode the main window is not focused while the pop-up is, but it still
    receives pointer motion; check this works in the real window and use `ctx.pointer_hover_pos`
    / the title response's `hovered()` accordingly.
  - Show the open menu with `popup_window` (title `menu_popup_title`, position
    `menu_popup_position(ctx.input(|i| i.viewport().inner_rect), title_rect)`). Its closure
    runs the existing `add_contents` of that menu unchanged (item code stays as is). Key the
    open flag so that when the pop-up reports itself closed (Escape, focus loss) and the open
    menu is still this one, the state clears; when another menu has been switched to meanwhile,
    leave the new state alone.
  - `ui.close()` inside the rows (all the items call it) must close the menu: since the rows
    no longer run inside egui's menu machinery, provide the close another way, e.g. wrap the
    rows with `ui.scope_builder(UiBuilder::new().sense(...))`/a `UiStack` tag that `ui.close()`
    understands, or replace the `ui.close()` calls in the menu bodies by a local closure; pick
    whichever keeps the item code smallest. An item click closes the menu and is reported by
    `menu_bar` as today.
  - **Keyboard**: Escape closes (popup_window does it). ArrowRight/ArrowLeft switch to the next
    or previous title, wrapping (File → Edit → View → Tools → Help → File), when the key is
    pressed while a menu is open and no interim submenu (below) is expanded; consume the key.
    Up/Down/Tab/Enter move and activate rows exactly as egui focus navigation did before; check
    it still does in the pop-up (inline).
  - **Fit**: keep `menu_scroll`. Inline the cap stays as now (window content bottom minus the
    bar bottom). In window mode the cap is the monitor's height (`ViewportInfo::monitor_size`
    of the root, if known) minus the pop-up's top y minus `MENU_MARGIN` minus the frame; when
    the monitor is unknown, fall back to the inline rule. `ScrollArea` must still scroll rows
    that exceed the cap.
- **`submenu_button` inside a window pop-up (interim)**: egui's `SubMenu` would be clipped by
  the small OS window, so when the surrounding menu is a window pop-up (set a flag in `ctx.data`
  while its content runs) `submenu_button` shows its rows expanded inline under the row,
  indented by 16 points, toggled by clicking the row (the arrow turns down). Inline mode keeps
  the real `SubMenu` as today, so `menus_scroll`/`menus_fit` tests stay valid. m12-submenu-popups
  replaces this.
- `app.rs` needs no change: `menu_bar_with_toolbars` is still called from the `menu` panel.
  The pop-up reads the root's inner rectangle itself.

Do not change any menu item, label, shortcut or action; do not touch `app/vendor`, `popup.rs`
or the existing tests. The hover-switch timing and the focus-loss rule are where it can go
wrong in the real window: use `harness/bin/gui-session` or the real-window test below to check.

## Tests (already written; do not edit)

`tests/menu_popups.rs`: pure placement and titles; kittest (inline): nothing open at first, click
opens and a second click closes each menu, the menu sits under its title, hover-switching,
hover with nothing open, Escape, Left/Right with wrap, choosing "New" reports the action and
closes, `request_menu`; and a gated real-window test (needs `DRYDOCK_GUI_TESTS=1`, X11, xdotool,
xprop) where a 320x160 probe opens Tools (taller than the window) as a separate
`_NET_WM_WINDOW_TYPE_POPUP_MENU` window just below the bar reaching past the window, File
replaces it, Escape closes it. The existing `menus*.rs`, `chrome*.rs` and M10 scroll tests must
still pass unchanged.

## Where to look

- `show popup_window`, `show PopupWindow`, `show PopupOutcome`, `show popup_mode`,
  `show popup_supported` (`popup.rs`).
- `show top_menu`, `show menu_scroll`, `show submenu_button`, `show menu_bar_inner`,
  `show MENU_BAR_BOTTOM`, `show MENU_MARGIN` (`chrome.rs`).
- `show floating_panel_window` (`floating.rs`) for reading the root's `inner_rect`.
- `tests/menus_scroll.rs` (`menus`, `open`) and `tests/menus_fit.rs` for the harness pattern.
