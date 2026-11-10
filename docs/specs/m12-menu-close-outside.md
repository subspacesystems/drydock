# m12-menu-close-outside: pop-up menus close on outside click, Escape and focus loss

Sign-off found that with the menus as OS pop-up windows (`PopupMode::Window`) a click in the
viewport or Escape in the main window leaves the menu open. The pop-up window never has
keyboard focus (under Xvfb, and on Windows, where pop-ups do not activate), so the pop-up's own
Escape / focus-loss rules never fire. The main window must close the menus itself, with the
same rules the inline mode already has. Inline mode is unchanged.

## What to build (`chrome.rs`, `popup.rs`)

1. `root_close(&RootInput) -> RootClose` (pure; skeleton on main). Rules, in this order:
   - no menu open: `Nothing`;
   - the main window unfocused for at least `FOCUS_GRACE` and no pop-up window focused: `All`;
   - Escape: `Submenu` if a submenu is open, else `All`;
   - a primary press not inside any title rectangle: `All`. A press on a title is `Nothing`:
     `top_menu` already handles it (switching titles, or toggling the open one closed once,
     with the `MENU_CLOSED` 0.3 s guard against reopening).
2. `close_menus_from_root(ctx, mode) -> bool`: does nothing unless `mode == PopupMode::Window`
   and a menu is open. Builds a `RootInput` from the ROOT viewport's input (primary press
   position this frame, Escape, the title rectangles saved by `top_menu` under `MENU_TITLE`,
   `viewport().focused`), applies the result (`Submenu` clears the submenu; `All` clears
   submenu and menu), and returns whether it closed anything.
   - Escape is consumed (`consume_key`) when it acts, so tools do not also see it. A press is
     not consumed: the click still reaches what is under it.
   - Unfocused time: remember when the root's `focused` went from `Some(true)` to
     `Some(false)` (ctx data, cleared when focus returns); `unfocused_for` is `None` if it
     never had focus (Xvfb without a window manager) or has it now. This avoids closing
     where focus is never reported.
   - Pop-up focus: `popup_window_window` records every frame whether its viewport is focused
     (`focused == Some(true)`) in ctx data, and `close_menus_from_root` reads it. On X11 with
     a window manager the pop-up may take focus when it maps: focus moving from the main
     window to a pop-up must not close it (the grace time and this flag together).
   - Call it from `menu_bar_inner` each frame, before the title buttons are laid out, so that
     a press on a title still reaches `top_menu`.
3. Submenu: `submenu_button` treats an Escape in the pop-up as it does now; the main-window
   Escape is handled by `close_menus_from_root` (submenu first, then the menu).
4. `request_menu_step(ctx, step)`: stores a pending step; `menu_bar_inner` applies it with the
   same wrap-around as the ArrowLeft/ArrowRight block (only when a menu is open and no
   submenu is). `popup_window_window` (or the menu code inside it) calls it when ArrowLeft
   / ArrowRight is pressed in a pop-up viewport that has focus, consuming the key. The root's
   own arrow handling stays.

Keep it small: no change to `PopupMode`, the winit patch, or the inline paths.

## Tests (already written; do not edit)

`app/crates/drydock/tests/menu_close_outside.rs`:
- pure rules for `root_close` (every row above, plus: nothing open closes nothing; a press on
  a title is left alone; focus on a pop-up keeps the menus; grace time);
- kittest: `close_menus_from_root` in `Window` mode closes on Escape and on a press (and the
  press still clicks the button under it); `Inline` mode does nothing; it reports whether it
  closed something; `request_menu_step` moves with wrap and does nothing with no menu open;
- a real-window X11 check (`DRYDOCK_GUI_TESTS=1`, Xvfb + xdotool, skipped otherwise): open File,
  xdotool click in the viewport area -> the pop-up window is gone; reopen, Escape with the main
  window focused -> gone.

Existing `menu_popups.rs` and `submenu_popups.rs` tests must keep passing.

## Where to look

`show` these: `top_menu`, `submenu_button`, `menu_bar_inner`, `open_menu`, `set_open_menu`,
`clear_submenu`, `request_menu`, `root_close`, `close_menus_from_root`, `request_menu_step`,
`RootInput`, `popup_window_window`, `popup_window`, `popup_mode`, `popup_viewport_id`,
`forget_state`. Constants near `MENU_TITLE`, `MENU_CLOSED`, `MENU_LABELS` in `chrome.rs`.
Test helpers: `menu_popups.rs` (`harness`, the `real` module's `find_window`, `geometry`).
