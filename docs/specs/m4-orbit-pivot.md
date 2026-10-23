# m4-orbit-pivot: orbit about the point under the cursor

Milestone M4, follow-up from Tyrel's M4 review ("Orbit mode doesn't orbit around
the cursor ... It should orbit around whatever point the cursor is over when I start
orbiting").

## Behaviour

When an orbit drag starts (left drag with the Orbit tool and no Shift, or a middle-button
drag without Shift, with any tool), the app finds the model point under the cursor at the
press position: the nearest drawn triangle the cursor's ray hits. For the whole drag the
view turns about that point, which stays fixed on screen. When the cursor is over empty
space, the drag orbits about the camera target as it does today. Pan, zoom and the other
tools are unchanged.

## What to build

### `Camera::orbit_about` (`crates/modeler-view/src/lib.rs`, skeleton on main)

```rust
pub fn orbit_about(&mut self, pivot: DVec3, dx: f64, dy: f64)
```

- Yaw and pitch change exactly as `orbit(dx, dy)` changes them (same rate, same pitch
  clamp to ±90°). `distance`, `fov_y` and `projection` stay.
- The camera turns rigidly about `pivot` by the same rotation: the yaw change about the
  world vertical (+Z) through the pivot, then the pitch change about the camera's new
  right axis through the pivot. Equivalently, the eye and the target both move by that
  rotation about the pivot. Use the pitch change actually applied after clamping, so a
  drag past straight up/down leaves the pivot where it was on screen.
- Consequences the tests check: the pivot's screen position and view depth don't change
  (both projections); the eye-to-pivot distance doesn't change; with `pivot == target`
  the result equals `orbit`; a 180° yaw mirrors the eye through the pivot's vertical axis.
- A non-finite pivot behaves exactly like `orbit(dx, dy)`.

### `FlatScene::raycast` (`crates/modeler-dae/src/flat.rs`, skeleton on main)

```rust
pub fn raycast(&self, origin: DVec3, dir: DVec3) -> Option<DVec3>
```

The nearest point where the ray `origin + t * dir`, `t > 0`, crosses a triangle of any
batch, either side; `None` when nothing is hit. `dir` need not be unit length (only its
direction matters). Lines are ignored. A zero or non-finite `dir` or `origin`, rays lying
in a triangle's plane, and triangles with out-of-range indices hit nothing. Compute in
f64. A linear scan over the triangles is fine (one cast per drag start); no acceleration
structure in this task.

### `ViewState::pivot` (`crates/modeler/src/viewstate.rs`, field on main)

`ViewState::apply(Action::Orbit { dx, dy })` calls `camera.orbit_about(pivot, dx, dy)`
while `pivot` is `Some`, else `camera.orbit(dx, dy)`. Nothing else in `ViewState` reads
or changes `pivot`.

### App wiring (`crates/modeler/src/app.rs`, `viewport_input`)

- When a drag starts that `input::drag` would turn into `Action::Orbit` (Primary with
  `view.tool == Tool::Orbit` and no Shift, or Middle without Shift), set
  `self.view.pivot` to `document.scene.raycast(origin, dir)` of the cursor ray at the press
  position (`EditorViewport::ray` gives the ray for both projections; build the viewport
  from the current camera and rect as the other handlers do). No document: `None`.
- When that drag ends (the button is released, or the drag is lost), set it back to
  `None`, so a later drag picks again and wheel zoom or panning isn't affected.
- The ray hits what is drawn: while editing inside an instance with the rest hidden, only
  the shown geometry can be the pivot.

## Tests

- `crates/modeler-view/tests/orbit_pivot.rs` (7 + a property test over random cameras,
  pivots anywhere on screen in front of the camera, and drags).
- `crates/modeler-dae/tests/flat_raycast.rs` (9).
- `crates/modeler/tests/viewstate_pivot.rs` (4).

The window wiring has no automated test; the orchestrator checks it in the real window.

## Out of scope

A visible pivot marker, orbiting about the selection, zoom/pan changes.
