# m0-camera: viewport camera

Milestone M0. Crate `app/crates/modeler-view`. Implement every `unimplemented!()` in `src/lib.rs`. Don't change public signatures or constants; add private helpers freely. `glam` only. In glam 0.34 the view/projection helpers are functions, not `DMat4` methods: `glam::dcamera::rh::view::look_to_mat4` and `glam::dcamera::rh::proj::directx::{perspective, orthographic}` (depth 0..1, y up) fit these conventions.

Conventions: world is meters, Z up, right-handed. NDC x right, y up, -1..1. Screen pixels from the top-left, y down. `aspect` = width / height.

## Orientation
- Offset from target to eye: `o = (cos(pitch) cos(yaw), cos(pitch) sin(yaw), sin(pitch))`; `eye = target + distance * o`.
- `basis()` returns (forward, right, up): `forward = -o`, `right = (-sin(yaw), cos(yaw), 0)`, `up = right x forward`. All unit length, and well defined at pitch = +-pi/2.
- `view_matrix()`: world to view, the camera at `eye` looking along `forward` with `up` as +Y (right-handed, looking down -Z).

## Projection
- `projection_matrix(aspect, scene)`: depth range 0..1.
  - Perspective: vertical field of view `fov_y`.
  - Parallel: orthographic with visible height `h = 2 * distance * tan(fov_y / 2)` and width `h * aspect`.
  - Near/far, with `scene = Some((center, radius))`: let `d` = distance from the eye to `center` along `forward`. `far = d + radius`, and `near = max(d - radius, far / 10_000)`; both are at least 1e-4 (perspective). For parallel, near may be negative: use `near = d - radius - 1e-3` and `far = d + radius + 1e-3`. With `None`, use near = distance / 1000 and far = distance * 1000 (parallel: near = -distance * 1000).
- `project(p, aspect)`: NDC of `p` under `projection_matrix(aspect, None)` composed with `view_matrix()`, and its depth along `forward` from the eye. `None` when the depth is <= 0 for a perspective camera.
- `target_plane_point(ndc, aspect)`: the point on the plane through `target` with normal `forward` that projects to `ndc`.
- `units_per_pixel(height_px)` = `2 * distance * tan(fov_y / 2) / height_px` (the same for both projections).

## Navigation
- `orbit(dx, dy)`: `yaw -= dx * ORBIT_DEGREES_PER_PIXEL` (in radians), `pitch += dy * ORBIT_DEGREES_PER_PIXEL`, then clamp pitch to [-pi/2, pi/2]. Target and distance unchanged. Dragging right swings the eye to the left (the model turns with the mouse); dragging down raises the eye.
- `pan(dx, dy, height_px)`: move the target (and so the eye) so the scene follows the mouse on the target plane: `target += (-dx * right + dy * up) * units_per_pixel(height_px)`. Orientation and distance unchanged.
- `zoom(steps, cursor_ndc, aspect)`: `f = ZOOM_STEP_FACTOR ^ steps`; new distance = clamp(distance * f, MIN_DISTANCE, MAX_DISTANCE); let f' = new / old distance. The point `p = target_plane_point(cursor_ndc)` must stay under the cursor: `target = p + (target - p) * f'`. Positive steps zoom in. Same for both projections.
- `zoom_extents(bounds, aspect)`: keep yaw, pitch, fov and projection. `target` = box center, `r` = half the box diagonal (if r < 1e-6 use 1). Perspective: `distance = r / sin(min(fov_y, fov_x) / 2)` where `fov_x = 2 atan(aspect tan(fov_y / 2))`. Parallel: the visible height must be `2r` if aspect >= 1, else `2r / aspect`; set distance to match. Result: every box corner projects inside NDC [-1, 1] on both axes, and the target projects to (0, 0).
- `set_standard_view(view)`: yaw/pitch in degrees:

| View | yaw | pitch |
| --- | --- | --- |
| Top | -90 | 90 |
| Bottom | -90 | -90 |
| Front | -90 | 0 |
| Back | 90 | 0 |
| Left | 180 | 0 |
| Right | 0 | 0 |
| Iso | -45 | asin(1/sqrt(3)) = 35.264 |

Front looks along +Y with +X to the right; Top looks down -Z with +X right and +Y up the screen; Iso shows +X going down-right and +Y going up-right at 30 degrees in a parallel projection.

## Tests (fixed; don't edit)
`tests/camera.rs` (examples) and `tests/properties.rs` (proptest invariants: orbit keeps target and distance, pan keeps orientation and tracks the cursor, zoom keeps the cursor point fixed, zoom extents frames any box at any orientation/aspect, basis is orthonormal everywhere).
