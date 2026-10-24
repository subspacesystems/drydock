# m4-orbit-ground: orbit about the ground when the cursor is over empty space

Milestone M4, follow-up from Tyrel's M4 review: orbiting about the point under
the cursor (`m4-orbit-pivot`) should, when the cursor is over empty space, orbit about the
ground at that spot instead of the camera target.

## Behaviour

The ground is the plane z = 0 (world space is meters, Z up). When an orbit drag starts and
the cursor ray hits no drawn triangle, the pivot is the point where that ray meets the
ground. If the ray never meets the ground in front of it (looking at the sky, or level with
the ground), or meets it very far away (near the horizon), the drag orbits about the camera
target as before.

## What to build

### `Camera::ground_pivot` (`crates/modeler-view/src/lib.rs`, skeleton on main)

```rust
pub const GROUND_PIVOT_RANGE: f64 = 50.0;

pub fn ground_pivot(&self, origin: DVec3, dir: DVec3) -> Option<DVec3>
```

Returns the point `origin + t * dir` with `t > 0` whose z is 0, or `None`:

- `None` when `origin` or `dir` has a non-finite component, `dir` is zero, or the camera's
  `target` or `distance` is not finite.
- `None` when the ray is level with the ground: `|dir.z| <= 1e-12 * |dir|`.
- `None` when the crossing is not in front of the origin (`t <= 0`; an origin lying on the
  ground gives `None`). Rays from below the ground going up do hit it.
- `None` when the point is farther than `GROUND_PIVOT_RANGE * self.distance` from
  `self.target` (exactly that far is still a hit). The range is measured from the target,
  not from `origin`: in parallel projection the ray origin sits far behind the camera.
- `dir` need not be unit length; only its direction matters.
- The returned point's z is exactly `0.0` (set it after computing x and y). Compute in f64.

The function doesn't use `yaw`, `pitch`, `fov_y` or `projection`.

## App wiring: not in this task

`m4-orbit-pivot` (in progress separately) sets `ViewState::pivot` from
`FlatScene::raycast` at the start of an orbit drag. After both tasks are on main, the
orchestrator changes that pick to fall back to `camera.ground_pivot(origin, dir)` when the
raycast misses. Don't edit `crates/modeler/src/app.rs` here.

## Tests

`crates/modeler-view/tests/orbit_ground.rs`: 12 worked examples and a property test
(random rays from above the ground pointing down whose crossing is in range: the result is
on the ray, in front of the origin, z = 0).

## Out of scope

Raycasting the model, a visible pivot marker, changes to orbit, pan or zoom.
