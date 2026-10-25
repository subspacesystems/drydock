# m4-axes-screen: extra lines sized in screen pixels

Milestone M4 (Tyrel, round 4: since the axes moved into the GPU pass they are
thick, and the positive halves show as large tilted rectangles. He wants them to look as
they did before, thin lines with short dashes, only hidden behind nearer geometry).

## Problem

`Renderer::render_with_lines` turns each `ExtraLine` into one quad whose width is worked
out at the segment's middle. An axis is 100 camera distances long, so its middle is far
away and the quad is many times too wide near the origin, and it faces the camera only at
the middle. The dashes of the negative halves were world-space pieces, so they grow and
shrink with distance instead of keeping a fixed pixel pattern.

## What is already on main

- `ExtraLine` has `width_px: f32` (target pixels) and `dash_px: Option<[f32; 2]>`
  (`[on, off]` in target pixels, measured on screen from `a`). The renderer reads
  `width_px` but still sizes the quad at the middle and ignores `dash_px`.
- `overlay::axis_lines(transform, half_length, pixels_per_point)` gives per axis one solid
  positive line and one dashed negative line, width `1.5 * ppp`, dashes `[6, 4] * ppp`;
  the app passes egui's `pixels_per_point`. Nothing to change there.

## What to build (`modeler-render`)

For every extra line drawn by `render_with_lines` / `render_headless_with_lines`:

1. **Width.** The line covers a band `width_px` target pixels wide around its projected
   centre line, everywhere along it: near the camera, far away, and for a line that runs
   towards the horizon. Both projections.
2. **Dashes.** With `dash_px = Some([on, off])`, the line is drawn `on` pixels, then left
   out `off` pixels, and so on, measured along the line on screen starting at the screen
   point of `a` (or, when `a` is behind the camera, where the line enters the view).
   A dash keeps its pixel length whether the line runs across the view or away from it.
   When `on` or `off` is not finite or not greater than 0, the line is solid.
3. **Front part only.** For a perspective camera, only the part of the line in front of
   the eye is drawn; a line that passes beside the camera draws as a ray from `a` to the
   edge of the image, with no stray triangles.
4. **Not cut by the scene's near and far distances.** The scene's projection keeps its
   near and far planes (extra lines still never change them). A part of a line farther
   than the far distance is still drawn, behind everything the scene draws; a part
   nearer than the near distance (but in front of the eye) is drawn in front of
   everything. The part between keeps the depth test against faces and lines as today
   (`tests/extra_lines.rs` must stay green).
5. **Bad values.** A line whose `width_px` is not finite or not greater than 0 draws
   nothing. Non-finite ends: skipped, as today.
6. Colours, the batching by colour and the rest of the pass stay as they are.

### Hints (not binding)

- The quad for a segment can keep its width constant on screen if its half-offset at each
  end is proportional to that end's view depth, along `direction × (mid - eye)` (that
  vector is perpendicular to every view ray to the segment). Long segments need no
  subdivision for this.
- Dashes can be cut on the CPU: map a screen fraction `f` of the projected segment back
  to the segment parameter with perspective `t = (f / w_b) / ((1 - f) / w_a + f / w_b)`
  (`w` = clip w of the ends).
- Near/far: split each piece where clip `z = 0` and `z = w` (both linear along the
  segment in clip space) and, in a separate vertex entry point for the extra-line
  pipeline, clamp `z` to `[0, w]`. Clamping without splitting would pull the in-range
  part of a long axis in front of the model. Clip to a small positive depth before that.
- Vertex count is no longer 6 per line; keep the colour runs (one draw per run).

## Tests

`modeler-render/tests/extra_lines_screen.rs` (lavapipe, 256x256): width across and along
receding lines, no gaps, no far cut, hidden under a face past the far distance, a line
passing beside the camera, dashes across and receding, bad width and dash values. The
tests compare against a render without the lines and allow 1.5 px for antialiasing.
