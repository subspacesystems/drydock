# m11-dialog-fit-content: dialog windows fit their content

## Problem
`dialog_window_impl` (crates/drydock/src/dialog.rs) opens each real dialog window at a fixed
inner size, not resizable, and draws the content in a `CentralPanel` with no scrolling. Content
past that size is cut off and unreachable: Model Info hides the Background rows, the
"Geolocation..." button and Close; Customize Toolbars cuts its tool list; the Export STL/3MF
options hide OK and Cancel. About and Purge are much taller than their content.

## Wanted
Every dialog window, modeless and modal, in the real-window path only, takes its height from
its content. The embedded path (`ctx.embed_viewports()`: `egui::Window` / `egui::Modal`) already
sizes to content and must not change.

1. **Pure functions in dialog.rs** (skeletons exist, `unimplemented!`):
   - `fit_height(content_height, monitor_height) -> DialogFit { height, scroll }`: `height` is
     `content_height` (never below 0). If `monitor_height` is `Some(m)` with `m > SCREEN_MARGIN`
     and `content_height > m - SCREEN_MARGIN`, then `height = m - SCREEN_MARGIN` and
     `scroll = true`. Otherwise (unknown, NaN, zero, negative, or too small a monitor) no clamp.
   - `needs_resize(current, target)`: true only if they differ by more than 1.0.
2. **Real-window sizing** in `dialog_window_impl`:
   - `window.size.x` stays the width; `window.size` stays the first-frame size.
   - Measure the content's height each frame inside the viewport callback: the `CentralPanel`
     inner response's `min_rect` height plus the panel's top and bottom inner margin (or the
     panel frame's `total_margin()`). Content must not be given a ScrollArea when it fits: a
     ScrollArea reports a size of its own and would hide the real height. Measure with
     `ScrollArea::vertical().auto_shrink([false, true])` only after a first frame that said
     `scroll = true`; keep the last `scroll` decision in `ctx.data_mut` / memory keyed by the
     dialog's viewport id, so a dialog that scrolls stays scrolling (no flip-flop).
   - Monitor height: `ui.input(|i| i.viewport().monitor_size)` of the dialog viewport (points);
     `None` when unknown.
   - Compute `fit_height`; if `needs_resize(current_inner_height, fit.height)` send
     `ViewportCommand::InnerSize(vec2(window.size.x, fit.height))` for the dialog viewport
     (`ctx.send_viewport_cmd_to(dialog_viewport_id(title), ..)`). Current inner height comes
     from `viewport().inner_rect` (fall back to `window.size.y` before it is known). Never send
     it every frame: after a send, ask for a repaint so the new size is read back, and do not
     send again until `needs_resize` says so. Content that is shorter than the window (a button
     row at the bottom of a window that has just shrunk) must not cause a grow/shrink loop:
     measure the content's own height, not the available height (`ui.min_rect()` after
     laying out, with the panel's `Frame` margins).
   - When `fit.scroll` the content goes in a vertical `ScrollArea` filling the panel.
3. **Customize Toolbars**: its tool lists scroll inside the window in any case (a vertical
   `ScrollArea` with a max height around the list's visible rows) so the window stays a
   reasonable height; the last tool must be reachable by wheel or scrollbar. Edit
   `customize_panel` in toolbars.rs accordingly; the buttons below the lists stay visible.
4. **Fixed sizes**: leave each caller's `size` as the first-frame size; adjust a caller only
   if its content needs a different width (Model Info, Customize Toolbars, Export options
   must not clip horizontally either). Purge unused (materials_panel.rs) and About shrink to
   content automatically.
5. **Test hooks** (already on main): `DRYDOCK_OPEN_DIALOG=model-info` opens Model Info,
   `=customize` opens Customize Toolbars.

## Acceptance
`crates/drydock/tests/dialog_fit.rs` (unit tests for the two functions; real-window tests
under `DRYDOCK_GUI_TESTS=1`: Model Info taller than 300 px and its Geolocation... button
reachable by click, About under 400 px, Customize Toolbars no taller than the screen). Existing
kittest suites (dialog_window, modal_prompts, sun_windows, dialog_print_toolbars, ...) stay green;
the embedded path is untouched.

## Where to look
- `dialog_window_impl`, `DialogWindow`, `dialog_viewport_id`, `centered_position`, `fit_height`,
  `needs_resize`, `SCREEN_MARGIN` in dialog.rs
- `model_info_window_with` (chrome.rs), `customize_window`, `customize_panel` (toolbars.rs),
  `export_prompt` (prompts.rs), the Purge dialog (materials_panel.rs)
- `App::new` hook block (app.rs, `dialog_hook`), "Geolocation..." button near app.rs:3241
- Test patterns: tests/dialog_window.rs, tests/dialog_sun.rs (`own_window_test`)
- egui: `ViewportInfo::{inner_rect, monitor_size}`, `ViewportCommand::InnerSize`,
  `Context::send_viewport_cmd_to`
