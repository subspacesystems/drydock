# m10-ground-pass-horizon: the shadowed ground pass leaves the sky alone

## Problem
With Show Shadows on (Shadows on Ground on), `fs_ground` in
`app/crates/drydock-render/src/scene.wgsl` paints the ground colour on rays that point up. In
perspective it takes the eye as origin and `t = -origin.z / ray.z`, and discards only `t <= 0`.
When the eye is slightly below z = 0 (float error grows with distance: Front view zoomed out
about 10 notches) or really under the ground (orbiting below), upward rays get `t > 0` and the
whole viewport turns ground-coloured. With shadows off the sky is right.

## Change (shader only)
In `fs_ground`:
- Discard unless `ray.z < 0`, in both projections (this also covers `abs(ray.z)` tiny).
- In perspective also discard unless `camera.eye.z > 0`.
- Parallel projection: keep origin = near-plane point; a top view still draws the shadowed ground.
- Everything discarded falls back to `fs_sky`, whose rule is per direction (sky above the
  horizon, ground below). Do not change `fs_sky`.
- Keep the behaviour of `m10-ground-shadow-pass`: ground colour shaded by `sun_visibility`,
  rays from the eye, same at any zoom.

## Tests
`app/crates/drydock-render/tests/ground_pass_horizon.rs` (given, do not edit):
- front view at distance 5000 and 50000, shadows on: top quarter sky, bottom quarter ground;
- eye just below ground (target z = -1e-3 * distance): top is sky;
- eye under the ground looking up: top rows are sky and equal the shadows-off render;
- top parallel view still shows the roof's shadow and lit ground.
Existing `ground_shadow_pass.rs` and `sky_ground.rs` tests must still pass.

## Where to look
- `fs_ground`, `fs_sky` in `scene.wgsl` (`show fs_ground`)
- `camera.eye`, `camera.parallel`, `camera.inv_view_proj` in the camera uniform struct
- test helpers in `tests/ground_shadow_pass.rs`: `style`, `slab`, `scene`, `render`, `camera`, `pixel_of`
