# m12-submenu-popups: submenus and context menus as pop-ups

After m12-menu-popups the top-level menus are pop-ups and `submenu_button` shows its rows
expanded inline under the row while the surrounding menu is a window pop-up (the interim).
This task replaces the interim: a submenu (today only View > Toolbars) opens as its own
pop-up to the side of its row, and a helper makes context menus that open at the click.
`submenu_button` is the only submenu in the app and nothing opens a context menu yet, so the
context helper is new code with no caller; do not add right-click menus anywhere.

## What to build

Skeletons on main: `chrome.rs` (`submenu_popup_title`, `submenu_popup_position`,
`open_submenu`) and the new `context_menu.rs` (`context_popup_title`, `context_popup_position`,
`context_menu_open`, `open_context_menu`, `context_popup`). Read their doc comments; they are
the contract. Both modules use `popup_window` (`popup.rs`).

### Submenus (`chrome.rs`)
- `submenu_button` keeps its row (label, drawn arrow, accessible label) and signature. Clicking
  the row opens the submenu, or closes it when it is the open one. The open submenu's label
  lives in `ctx.data` (temp), read by `open_submenu`; at most one is open. Remember the row's
  rect in `ctx.data` for the position.
- The rows show in `popup_window` (title `submenu_popup_title(label)`), at
  `submenu_popup_position(root_inner, row, size, monitor)` where `root_inner` is
  `ctx.input(|i| i.viewport().inner_rect)`, `size` is the pop-up's size from last frame (the
  content size; on the first frame use the row's size, the window is resized right after) and
  `monitor` is the root's `ViewportInfo::monitor_size`. `submenu_popup_position` is pure.
- It replaces the interim expanded-inline behaviour, and the egui `SubMenu` use. The rows run
  via the same `add_contents`; `ui.close()` inside them closes the submenu and its parent
  menu (the item code in `chrome.rs` and `toolbars.rs` stays as is). A checkbox row in the
  submenu (toggling a toolbar) must not close anything, as now.
- **Focus.** In window mode the submenu takes focus from its parent pop-up, and a pop-up
  closes on focus loss. The parent must not close because its own submenu took focus: while a
  submenu of it is open, ignore the parent's focus-loss close. When the submenu itself loses
  focus and the parent does not have it either (both lost it: the user clicked elsewhere or
  switched windows), close both. Clicking another row of the parent closes the submenu
  (hover on another row may too, after the pointer rests there; keep what is simplest and
  does not flicker). This is the part that can go wrong in the real window: try it with the
  real-window test or `harness/bin/gui-session`.
- Inline mode (tests, Wayland until m12-winit-popup-wayland): `popup_window` draws an area;
  the parent's "press outside closes" rule must not count a press inside the submenu's area as
  outside. The M10 fallback stays: the inline submenu is capped to the window (content bottom
  minus the bar bottom, minus `MENU_MARGIN` and the frame) and scrolls with `menu_scroll`;
  in window mode the cap is the monitor height minus the pop-up's top y minus `MENU_MARGIN`
  minus the frame, as for the top-level menus. `menus_scroll.rs` and `menus_fit.rs` pass
  unchanged. Where the new placement conflicts with them in an inline area (the submenu may
  not sit above the bar), clamp in inline mode; the existing tests are the guide.
- Closing the parent menu (Escape, title click, switching to another menu, an item chosen)
  closes the submenu. Escape closes the submenu first or both at once (the test accepts
  either, then a second Escape).

### Context menus (`context_menu.rs`)
- `context_popup(ui, id, response, add_contents)`: when `response.secondary_clicked()`, store
  the pointer position (`response.interact_pointer_pos()`) as the open position for `id`
  (`ctx.data`, temp) and open. While open, show `popup_window` titled `context_popup_title(id)`
  at `context_popup_position(root_inner, click)`. `context_menu_open` returns that screen
  position; `open_context_menu` sets it directly (position given in the calling viewport's
  points, converted the same way). Escape and focus loss close (the popup clears `open`); an
  item's `ui.close()` closes; a primary press outside the pop-up closes it in inline mode (the
  popup does that already); in window mode focus loss does it.
- Rows are laid out like a menu's (`menu_scroll` capped by the monitor height, as for
  submenus), so share a small helper with `submenu_button` if that is shorter.

Do not change any menu item, label, shortcut or action; do not touch `app/vendor` or `popup.rs`.
`menu_popups.rs`, `popup_window.rs` and the other existing tests must still pass.

## Tests (already written; do not edit)

`tests/submenu_popups.rs`: pure titles and placement (right of the row, flips left at the
monitor's right edge, moves up at its bottom, context position); kittest (inline): clicking
Toolbars opens its rows beside the row with View still open, "Reset Toolbars..." and
"Customize..." report their events and close everything, toggling a toolbar changes the layout
and keeps the submenu open, Escape, closing or switching the parent menu closes the submenu;
context menus: closed at first, a right click opens at the click position, a left click does
not, choosing a row acts and closes, Escape closes, `open_context_menu` without a click; and a
gated real-window test (`DRYDOCK_GUI_TESTS=1`, X11, xdotool, xprop) that opens View, clicks
down its rows until the Toolbars pop-up appears, checks it is a separate `POPUP_MENU` window
to the right of the menu, and that Escape closes both.

## Where to look

- `show popup_window`, `show PopupWindow`, `show PopupOutcome`, `show popup_mode`,
  `show inline_position` (`popup.rs`).
- `show submenu_button`, `show top_menu`, `show menu_scroll`, `show menu_popup_title`,
  `show menu_popup_position`, `show open_menu`, `show MENU_MARGIN`, `show MENU_BAR_BOTTOM`
  (`chrome.rs`; the top-level pop-up code from m12-menu-popups is the pattern to follow).
- `show toolbars_menu`, `show ToolbarsMenuEvent` (`toolbars.rs`).
- Tests to copy patterns from: `tests/menu_popups.rs` (`harness`, real-window probe),
  `tests/menus_scroll.rs`, `tests/menus_fit.rs`.
