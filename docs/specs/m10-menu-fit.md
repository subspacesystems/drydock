# m10-menu-fit: long menus open below their title and scroll

With the window 1100x420, Tools pushes itself up over the menu bar (covering the
File..Tools titles) and is cut off at the bottom. egui keeps popups on screen by shifting them up.

## Rule
- A top-level menu's top edge sits at the menu bar's bottom, never above it.
- Its height is capped at the space from there to the window's bottom edge, minus a small margin
  (about 8 px). Content that doesn't fit scrolls with the mouse wheel (a vertical `ScrollArea`
  inside the menu), and the last item can be scrolled to and clicked.
- A submenu opens beside its row and may shift up to fit, but never above the menu bar's bottom.
  Same height cap (window bottom minus the menu bar's bottom, minus the margin) and same scroll.
- A menu that fits looks exactly as now: no scroll bar, same width, same row positions. The
  existing `menus_fit.rs` and `menus_items.rs` tests stay green.
- Scrolling moves the rows inside the menu, never the menu.

## How
- One helper in `chrome.rs` wraps a menu's content: `ScrollArea::vertical()` with
  `auto_shrink([true, true])` and `max_height` set to the cap, so a menu that fits keeps its width
  and height. Use it in every `ui.menu_button` body in `menu_bar_inner` and in `submenu_button`.
  Check that the scroll bar does not widen a menu that fits (it must appear only on overflow) and
  that a menu that overflows keeps its rows full width.
- Cap for a top-level menu: window bottom (`ctx.content_rect().bottom()`) minus the top of the
  menu's content (`ui.cursor().top()` inside the closure) minus the margin. For a submenu: window
  bottom minus the menu bar's bottom minus the margin (egui shifts it up to fit; with the cap it
  can't reach above the bar). Get the bar's bottom from the `Ui` that holds the `MenuBar`
  (`menu_bar_inner`'s `ui.max_rect()`/`min_rect()` bottom) and pass it to `submenu_button`
  through the egui context (`ctx.data`) or an argument; `submenu_button` is public, keep its
  signature working for existing callers (default: use the top of the row's menu).
- If the popup is still shifted above the bar by egui, set the popup's position/constraint
  explicitly (`MenuButton::config`, `Popup::constrain_to`/`.anchor`) rather than hacking offsets.

## Tests (already written, do not edit)
`crates/drydock/tests/menus_scroll.rs` (egui_kittest, pointer wheel and clicks): Tools at 1100x420
stays under the bar; wheel to the end, then Print Setup... clicks to `Action::PrintSetup`; File,
Edit and Help at 1100x420 equal their layout in a tall window; all five menus at 1280x800 equal
their layout in a tall window; View at 1100x300 scrolls and its Toolbars submenu stays under the bar
and its last row scrolls into the window.

## Where to look
- `chrome::menu_bar_inner` (`show menu_bar_inner`), `chrome::submenu_button`, `chrome::menu_item`,
  `chrome::menu_check_item`, `toolbars::toolbars_menu` (the Toolbars submenu body).
- egui 0.36 `containers::menu` (`MenuBar`, `MenuButton`, `SubMenu`, `MenuConfig`) in the cargo
  registry source; `egui::ScrollArea`.
- Test helpers: `tests/menus_fit.rs` (`open`, `row`, `menu_rows`), `tests/panel_scroll.rs`
  (`wheel_over`), `tests/menus_scroll.rs`.
