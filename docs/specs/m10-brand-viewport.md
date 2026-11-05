# m10-brand-viewport: viewport colours per theme, rose back faces

Milestone M10. The viewport takes its colours from the brand palette for the current theme. Skeletons are on main: `Style.default_back` in `drydock-render`, and `look::viewport_style`, `look::apply_viewport_theme`, `look::axis_color`. Fill in the `unimplemented!` bodies. Needs `m10-brand-theme` (`look::palette` returns real values).

## Colours
`viewport_style(theme)` returns a `drydock_render::Style` with, from `palette(theme)` (RGB bytes / 255, alpha 1):
- `background` = `viewport_bg`
- `default_line` = `edge`
- `default_face` = `face_front`
- `default_back` = `Some(face_back)`
- `sun` = `None`

Dark: `#050A19`, `#EEF7FF`, `#3D475E`, `#B04A80`. Light: `#F4F8FC`, `#091126`, `#E2E8F4`, `#C25A8C`.

`apply_viewport_theme(style, theme)` overwrites those four colour fields and leaves `sun` alone; returns true when any value changed. There is no viewport grid: do not add one.

## Back faces (render)
Faces without a material are drawn twice today: front-facing triangles in the first opaque pass, back-facing ones in the `opaque_back_pipeline` pass, both with `style.default_face` (`Renderer::render_with_lines`). In the back pass, a batch without a material uses `style.default_back` when it is `Some`, else `default_face`. Nothing else changes: batches with a material (either side), transparent batches, lines, shading, the sun and the overhang tint keep their colours. `Style::default().default_back` is `None`, so every existing render test stays as it is. The shader needs no change unless you find you need it.

## Axis colours
Axis colours depend on the theme: `look::axis_color(theme, axis)` (`axis_x/y/z` of the palette, grey `[160,160,160]` for other indices). Add a `theme: Theme` parameter, last, to:
- `hints::axis_color(theme, axis)` (delegates to `look::axis_color`) and `hints::hint_label(&inference, theme)`;
- `overlay::axis_lines(transform, half_length, pixels_per_point, theme)`: its private `COLORS` goes;
- `overlay_style` in `app.rs`, for `overlay::Style::Axis` (use `look::axis_color`).

The tests `tests/hints.rs` and `tests/axes_gpu.rs` are already updated to the new signatures (Dark colours); snap colours (`snap_color`) are unchanged.

## App wiring
`App` builds `self.style` from `viewport_style(theme)` at start (with the sun as before) and, each frame, takes the theme from `look::theme_of(&ctx.style().visuals)`; when `apply_viewport_theme` returns true, set `self.dirty = true`, so the viewport follows the OS theme at once. Pass the theme to `axis_lines`, `hint_label` and `overlay_style`. The `--screenshot` path in `main.rs` keeps `Style::default()` (existing CLI tests pin its pixels).

## Tests (fixed)
- `drydock-render/tests/viewport_colors.rs`: clear colour, default face from the front and from behind (rose), no back colour = same on both sides, a material keeps its colour on both sides.
- `drydock/tests/viewport_look.rs`: style values per theme, `apply_viewport_theme` keeps the sun and reports changes, axis lines use the theme's colours.
- `drydock/tests/hints.rs`, `tests/axes_gpu.rs`: updated for the new signatures.

All other tests keep passing (workspace). No new dependencies.

## Where to look
`show Style` (drydock-render), `show Renderer::render_with_lines`, `show linear_color`, `show Palette`, `show palette`, `show theme_of`, `show hint_label`, `show axis_lines`, `show overlay_style`, `show App::paint_viewport`, `show shading::update_style`; test helpers `scene`, `top_camera` in `drydock-render/tests/viewport_colors.rs`.
