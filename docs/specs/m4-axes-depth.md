# m4-axes-depth: axes hidden behind nearer geometry

Milestone M4 (Tyrel's report: the axes show through the model).

## Problem

The axes are drawn by egui on top of the rendered view, so they ignore the model: a
wall in front of the origin does not hide them. They must be drawn in the GPU pass with
the scene's depth test.

## What to build

### Renderer (`modeler-render`)

`ExtraLine { a, b, color }` (already on main) is a world-space segment, `color` sRGB RGBA.

- `Renderer::render_with_lines(device, queue, target, scene, camera, style, extra)` draws
  the scene exactly like `render`, plus every `extra` line in the same pass with the
  same depth test as the scene's own lines, so faces and lines nearer the camera hide
  them and they hide whatever is behind them. Lines are unlit and use `color` as given
  (alpha included). An empty `extra` gives the same image as `render`.
- Non-finite ends: that line is skipped.
- `render_headless_with_lines` is the headless form. `render_headless` and `render` stay
  as they are.
- Extra lines never change the scene's bounding sphere (the near and far planes).

### `overlay::axis_lines(transform, half_length, dash) -> Vec<ExtraLine>` (`modeler`)

Built from the same six axes as `overlay::axes(transform, half_length)` (same origin,
directions, order, same omissions and same empty results for bad input).

- Colors (sRGB 0..1, alpha 1): red `200/255, 40/255, 40/255`, green `40/255, 150/255,
  40/255`, blue `40/255, 70/255, 210/255` (as the egui overlay uses today).
- Positive half: one line from the origin, `half_length` long.
- Negative half: dashes along the same line. Dash `k` (k = 0, 1, ...) runs from
  `2k * dash` to `(2k + 1) * dash` from the origin, so a gap of `dash` follows each dash.
  The last dash is cut at `half_length`. At most 300 dashes per axis. When `dash` is not
  finite or not greater than 0, the negative half is one solid line instead.
- Order in the result: for axis 0, 1, 2: the positive line, then its dashes in order.

### App wiring

In `app.rs`, `View > Axes` draws these lines through `render_with_lines` (origin of the
open instance as today, `half_length = camera.distance * 100`, `dash = camera.distance *
0.04`) and the egui `overlay::axes` painting is removed. The viewport must be redrawn
when the camera changes, the axes are toggled or an instance is opened or closed
(it already is for every action). `overlay::axes` and `clip_in_front` stay (tests use them).

## Tests

`modeler-render/tests/extra_lines.rs`, `modeler/tests/axes_gpu.rs`. Lavapipe renders them;
the render tests skip nothing.
