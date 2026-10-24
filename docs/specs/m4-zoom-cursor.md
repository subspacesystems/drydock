# m4-zoom-cursor: zoom about the model point under the cursor

Milestone M4, from Tyrel's report: close to a surface, one wheel notch jumps so
far that the view passes through the surface. Each step should be relative to what the
cursor is over, so a small part of a large model (a computer mouse in a house) can be
looked at closely.

## Why it happens today

`Camera::zoom` scales the eye-to-target distance by `ZOOM_STEP_FACTOR` (0.8) per step and
keeps the point on the target plane under the cursor fixed. The eye moves 20% of the
distance to the *target* per step. When the surface under the cursor is much nearer than
the target, that is more than the whole distance to the surface.

## Behaviour

A wheel zoom, and a Zoom-tool drag, scale the view about the model point under the cursor:
the nearest drawn triangle the cursor's ray hits (the same pick as the orbit pivot). One
step in moves the eye 20% of the way to that point, one step out moves it back, and the
point stays under the cursor. Zooming in never reaches or passes the point. Over empty
space (the ray hits nothing) zoom works as it does today. Orbit, pan and the other tools
are unchanged.

## What to build

### `Camera::zoom_about` (`crates/modeler-view/src/lib.rs`, skeleton on main)

```rust
pub fn zoom_about(&mut self, steps: f64, pivot: DVec3)
```

A uniform scaling of the camera about `pivot`:

- `distance` becomes `distance * ZOOM_STEP_FACTOR.powf(steps)`, clamped to
  `MIN_DISTANCE..=MAX_DISTANCE`. Let `ratio` be new distance / old distance (after the
  clamp, so a clamped zoom moves nothing more).
- `target` becomes `pivot + (target - pivot) * ratio`.
- `yaw`, `pitch`, `fov_y` and `projection` stay.

Consequences the tests check: the eye moves to `pivot + (eye - pivot) * ratio`; the pivot's
NDC position doesn't change (both projections); its view depth scales by `ratio`; with the
pivot on the target plane under `ndc` the result equals `zoom(steps, ndc, aspect)`; zooming
`s` then `-s` returns the original camera; any number of steps in leaves the pivot in
front of the eye.

A non-finite `pivot` (or non-finite `steps`) leaves the camera unchanged.

### `viewstate::zoom_pivot_at` (`crates/modeler/src/viewstate.rs`, skeleton on main)

```rust
pub fn zoom_pivot_at(
    camera: &Camera,
    scene: Option<&FlatScene>,
    viewport_px: (f64, f64),
    cursor_px: DVec2,
) -> Option<DVec3>
```

The ray through `cursor_px` (pixels from the viewport's top left, y down) is
`editor::Viewport { camera: *camera, size_px: viewport_px }.ray(cursor_px)` (right for
both projections). Return `scene.raycast(origin, dir)`; `None` when `scene` is `None` or
nothing is hit. No ground-plane fallback: over empty space zoom keeps today's behaviour.

### `ViewState::zoom_pivot` and `ViewState::scroll_zoom` (same file, skeleton on main)

- `zoom_pivot: Option<DVec3>` (field on main, `None` in `new`).
- `apply(Action::Zoom { steps, cursor_ndc })`: while `zoom_pivot` is `Some(p)`, call
  `camera.zoom_about(steps, p)` and ignore `cursor_ndc`; else `camera.zoom(steps,
  cursor_ndc, aspect)` as today. `apply` never changes `zoom_pivot`, and `Action::Orbit`
  ignores it (orbit uses `pivot`, as before; zoom still ignores `pivot`).
- `scroll_zoom(lines, cursor_px, scene, viewport_px)`: one wheel zoom of `lines` steps.
  With `p = zoom_pivot_at(&self.camera, scene, viewport_px, cursor_px)`: `Some(p)` →
  `camera.zoom_about(lines, p)`; `None` → `camera.zoom(lines, ndc, aspect)`, today's
  wheel zoom (what `apply(input::scroll(lines, ndc), viewport_px)` does with no
  `zoom_pivot`), where `ndc` is `cursor_px` in NDC (`x = 2 * cx / w - 1`,
  `y = 1 - 2 * cy / h`) and `aspect` comes from `viewport_px` as in `apply`. `lines == 0`
  changes nothing. `zoom_pivot` and `pivot` are the same afterwards as before.

### App wiring (`crates/modeler/src/app.rs`, `viewport_input`)

- Wheel: add up the lines of all `MouseWheel` events of the frame (the unit conversion
  stays as it is), then, if the sum isn't 0, call `self.view.scroll_zoom(sum,
  local(cursor), scene, size)` once, with `scene` the open document's `scene` (`None`
  without a document) and `size` the viewport rect's (width, height) as the orbit-pivot
  code uses. One raycast per frame, not per event. Mark the view dirty as `apply` does
  for `Effect::None`.
- Zoom-tool drag: when a Primary drag starts with `self.view.tool == Tool::Zoom`, set
  `self.view.zoom_pivot = zoom_pivot_at(...)` at the press position (beside where `pivot`
  is set for an orbit drag). When the drag stops, set it back to `None`, next to the
  existing `pivot = None`.
- The pick hits what is drawn: while editing inside an instance with the rest hidden, only
  the shown geometry can be the pivot (the document's `scene` already is what's drawn).

## Tests

- `crates/modeler-view/tests/zoom_about.rs` (11, including two property tests over random
  cameras, pivots in front of the camera and steps).
- `crates/modeler/tests/viewstate_zoom.rs` (9).

The window wiring has no automated test; the orchestrator checks it in the real window.

## Out of scope

Pan speed relative to the surface, a ground-plane pivot for zoom, a zoom pivot marker,
a raycast acceleration structure, changing `ZOOM_STEP_FACTOR`.
