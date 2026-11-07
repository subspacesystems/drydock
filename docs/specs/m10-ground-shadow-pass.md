# m10-ground-shadow-pass: shade the ground colour, intersect from the eye

Feedback on M10: with Show Shadows on and Shadows on Ground ticked, the ground turns
background-coloured, and when zoomed out the strip nearer than the model (bottom of the
viewport) loses its shadows and shows a different colour.

## What is wrong

`fs_ground` in `drydock-render/src/scene.wgsl` draws after the sky pass (`fs_sky`):

1. It returns `camera.background.rgb * shade`. The sky pass shows the model's ground colour
   below the horizon when the ground is on, so the shadow pass must shade that colour.
2. It starts the ray at the near-plane point and discards `t <= 0`. Zoomed out, the near
   plane is `d - radius` away (`Camera::projection_matrix`), so ground between the eye and
   the model's bounding sphere is behind the start point, gets discarded, and shows unshaded
   ground colour, even where a shadow falls.

## Required behaviour (shader only; no Rust API change)

- Base colour: `camera.ground_color` when its `w` flag is set (ground on), otherwise
  `camera.background` (ground off: as before). Output `base.rgb * shade`, alpha from `base`.
  Ground pixels in light keep exactly the ground colour.
- Perspective (`camera.parallel == 0`): intersect the line from `camera.eye` through the
  pixel's far-plane point with `z = 0`; keep hits with `t > 0` (in front of the eye), discard
  the rest and near-horizontal rays. An eye below the ground keeps the current behaviour
  (the plane is hit above it and shaded).
- Parallel: keep using the near point and ray direction, and accept a hit at any `t`,
  including negative ones.
- `sun_visibility` and the shade mix are unchanged. `lib.rs` needs no change (the eye,
  ground colour and flags are already in the camera uniform).

Existing tests (`sun_shadows.rs`, `sky_ground.rs`) must stay green; they use the background
as ground colour when `Style::ground` is `None`.

## Tests

`drydock-render/tests/ground_shadow_pass.rs` (6 tests, headless): lit ground equals the
ground colour and shadowed ground is darker by dark / lit; no background colour below the
horizon; ground off still shades the background; a camera 50 radii away looking down 35
degrees shows the ground colour on the bottom row and near the top; a shadow nearer than the
near plane is drawn (the test asserts that precondition); parallel views shade the ground colour.

A GUI check follows at sign-off (draw a box, Show Shadows, zoom out 10 notches).

## Where to look

- `show fs_ground`, `show fs_sky`, `show sun_visibility` in `scene.wgsl`; the `Camera` struct
  there (`eye`, `parallel`, `ground_color`, `background`).
- `CameraUniform` and the draw order around `ground_pipeline` in `drydock-render/src/lib.rs`.
- `Camera::projection_matrix` in drydock-view for the near/far planes.
- Test helpers: `render_headless`, `Style`, `SunLight` (`tests/sun_shadows.rs`, `tests/sky_ground.rs`).
