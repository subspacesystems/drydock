# m11-dialog-window: dialogs in their own OS windows

Tyrel's request: dialogs should be their own windows that belong to the main window,
not boxes stuck inside it. This task adds the helper and moves Model Info and About to it.
It builds on `m11-winit-parent`: every window after the root already names the root as its
transient parent, so the helper only has to create the viewport.

## What to build

`crates/drydock/src/dialog.rs` already has the skeleton (`DialogWindow`, `centered_position`,
`dialog_viewport_id`, `dialog_window`); fill in the bodies.

- `centered_position(root_outer, size)`: the top-left that centers a `size` window over
  `root_outer`; each coordinate is clamped to at least 0.
- `dialog_viewport_id(title)`: `ViewportId::from_hash_of` of a fixed prefix plus the title.
- `dialog_window(ctx, window, open, content)`: does nothing when `!*open`.
  - **Real window** (`!ctx.embed_viewports()`): `ctx.show_viewport_immediate` with
    `dialog_viewport_id`, a `ViewportBuilder` with the title, `with_inner_size(window.size)`,
    not resizable, no minimize/maximize, and `with_position(centered_position(..))` using the
    root's `outer_rect` (read it with `ctx.input(|i| i.viewport().outer_rect)` before
    opening; skip the position when it is `None`). Inside, draw `content` in a
    `CentralPanel` (with the usual dialog frame margin). When the viewport's
    `close_requested()` is true, or Escape is pressed in it, clear `open`.
  - **Embedded** (`ctx.embed_viewports()`, as in `egui_kittest`): an `egui::Window` titled
    `window.title`, not collapsible, not resizable, with `.open(..)` so its title-bar × clears
    `open`; Escape also clears `open`.
  - `content` may also clear `open` itself, through the caller's own flag (the Close button).
    Since `content` is `FnOnce` and the viewport callback is `FnMut`, keep it in an `Option`
    and `take()` it.
- Move **About** (`chrome::about_window`, title "About Drydock") and **Model Info**
  (`chrome::model_info_window_with`, title "Model Info") to the helper. Keep their signatures
  and their Close buttons; only the `egui::Window::new(..)` wrapper changes. Pick an initial
  size that fits each one's content (About is about 380 x 440 points, Model Info about
  360 x 300; check with a screenshot). The existing kittest tests for both (`chrome.rs`,
  `menus.rs`, `status.rs`, `sun_windows.rs`, `about_lockup.rs`, `drydock_text.rs`,
  `background_model_info.rs`) must pass unchanged.
- Fonts, theme and the lockup images come from the shared `Context`; the About lockup loader
  setup (`install_lockups`) stays where it is. Check that the About lockup shows in the real
  window.
- Test hook: when the environment variable `DRYDOCK_OPEN_DIALOG` is `about` at startup, the
  app opens About right away (set `about_open` when building the app in `app.rs`). Undocumented
  in `--help`; it exists for the real-window test.
- Only egui APIs: no new dependency.

## Acceptance tests

`tests/dialog_window.rs` (do not edit):
- `centered_*`, `viewport_id_follows_the_title`: the pure functions.
- Embedded: titled window with content; nothing drawn while closed; Escape clears `open`.
- Real window (needs Xvfb, xdotool, xprop and `DRYDOCK_GUI_TESTS=1`; skips itself otherwise):
  starts the app with `DRYDOCK_OPEN_DIALOG=about`, finds a second X window titled
  "About Drydock", checks its `WM_TRANSIENT_FOR` is the main window, presses Escape with the
  dialog focused and checks it disappears while the main window stays.

Run `harness/bin/run-tests -p drydock` at the end, with Xvfb and `DRYDOCK_GUI_TESTS=1`.

## Where to look

Use `show <name>`:
- `dialog_window`, `DialogWindow`, `centered_position`, `button_row`, `DialogButton` in
  `dialog.rs`.
- `about_window`, `model_info_window`, `model_info_window_with`, `install_lockups` in
  `chrome.rs`; their callers in `app.rs` (`about_open`, `model_info_open`, `Effect::About`).
- egui: `Context::show_viewport_immediate`, `Context::embed_viewports`, `ViewportBuilder`,
  `ViewportInfo::outer_rect`, `ViewportCommand` (egui 0.36).
- Vendored winit/egui-winit patch: `app/vendor/egui-winit/PATCHES.md` (transient parent).
