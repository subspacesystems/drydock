# m10-viewport-gamma: the viewport reaches egui without a second sRGB conversion

## Problem
The viewport is rendered into an `Rgba8UnormSrgb` texture (`COLOR_FORMAT`) and the app
hands a view of that texture to egui. egui's shader expects plain (gamma) values, so an
sRGB view is linearized when sampled and the image is shown too dark: the Dark background
`#050A19` appears as `#010103`.

## Change
- `drydock-render`: create the color texture with `view_formats: &[DISPLAY_FORMAT]` and
  make `Viewport::display_view` a view of it in `DISPLAY_FORMAT` (`Rgba8Unorm`, same bytes).
  Rendering keeps using the sRGB `color_view`; `color`, readback and the headless render
  functions stay as they are. Do not change `linear_color` or any palette color.
- `drydock` (`app.rs`): already passes `viewport.display_view` to egui (skeleton, on main); nothing to do there.
- Remove the skeleton TODO on `display_view`.

## Tests (already written, in `tests/display_view.rs`)
- An empty scene with the Dark background, sampled through `display_view` without any
  conversion, gives `#050A19` within 2 per channel (skips when there is no adapter).
- Every viewport size (including 0, clamped to 1) has a display view; `color` stays sRGB.
- All existing tests keep passing (`viewport_colors`, `render`, ...).

## Where to look
- `Viewport` (struct), `Renderer::viewport`, `COLOR_FORMAT`, `DISPLAY_FORMAT`, `linear_color`
  in `app/crates/drydock-render/src/lib.rs`.
- `register_native_texture` / `update_egui_texture_from_wgpu_texture` calls in
  `app/crates/drydock/src/app.rs` (search `color_view`).
- `tests/display_view.rs` (`sample` helper) for how the view is checked.
