# m4-zoom-through: Ctrl+wheel steps through the surface under the cursor

Milestone M4, from Tyrel's follow-up. Wheel zoom now slows down near the
surface under the cursor and never passes it (`m4-zoom-cursor`). To get inside a closed
model (a house) he needs a way through: holding Ctrl while turning the wheel steps the eye
through the surface under the cursor, one notch per surface; Ctrl with the wheel turned
back steps out again.

## Behaviour

- **Ctrl + one notch forward:** the eye moves along the cursor's ray, past the first
  surface the ray meets, and stops halfway between that surface and the next one along the
  ray. With no further surface it stops as far beyond the surface as it was in front of
  it. The view direction, the camera distance and the field of view don't change (the
  whole camera, eye and target, moves by the same vector), and what was under the cursor
  stays on the cursor's ray.
- **Ctrl + one notch back:** the same along the ray behind the eye (the cursor's ray
  reversed): out through the nearest surface behind, to halfway to the next one behind it,
  or mirrored.
- Several notches in one call step that many surfaces. Fractions of a notch (smooth wheels,
  touchpads) add up in `ViewState::through_carry` and step once the sum reaches a whole
  notch either way.
- Nothing to step through (no surface ahead, or behind for a notch back): that notch is a
  plain wheel zoom step, as without Ctrl.
- In parallel projection moving the eye along the view changes nothing on screen, so
  Ctrl+wheel is the plain wheel zoom there. Without a scene too.
- Without Ctrl the wheel works as today.

## What to build

### `FlatScene::ray_hits` (`crates/modeler-dae/src/flat.rs`, skeleton on main)

```rust
pub fn ray_hits(&self, origin: DVec3, dir: DVec3) -> Vec<f64>
```

The ray parameters `t > 0` of every triangle of every batch the ray crosses (either side),
sorted ascending; the point is `origin + dir * t`, so for a non-unit `dir` the values are
not lengths. Use the same triangle test as `raycast` (same acceptance of edges, same
handling of out-of-range indices, which are skipped), so the first value is always the
point `raycast` returns, and `ray_hits` is empty exactly when `raycast` is `None`. Each
triangle crossed adds its own value, so a ray through an edge two triangles share may list
that surface twice. Lines don't count. A non-finite or zero `dir` or a non-finite `origin`
gives an empty list. Sharing a private helper between `raycast` and `ray_hits` is fine as
long as `raycast`'s results don't change.

### `viewstate::through_distance` (`crates/modeler/src/viewstate.rs`, skeleton on main)

```rust
pub fn through_distance(hits: &[f64]) -> Option<f64>
```

`hits` are distances along a unit ray, any order, repeats allowed. Ignore values that are
not finite or not `> 0`. With none left, `None`. Else let `t1` be the smallest. Values `h`
with `h <= t1 + 1e-6 * t1 + 1e-9` are the same surface as `t1`. If a value beyond that
exists, let `t2` be the smallest such and return `t1 + (t2 - t1) / 2`; else return
`2 * t1`.

### `ViewState::through_carry` and `ViewState::scroll_through` (same file)

`through_carry: f64` is on main (0 in `new`).

```rust
pub fn scroll_through(
    &mut self,
    lines: f64,
    cursor_px: DVec2,
    scene: Option<&FlatScene>,
    viewport_px: (f64, f64),
)
```

1. `lines == 0`: nothing changes (carry included).
2. `scene` is `None`, or the camera's projection is `Parallel`: exactly
   `self.scroll_zoom(lines, cursor_px, scene, viewport_px)`, and set `through_carry` to 0.
3. Else add `lines` to `through_carry`. While `through_carry >= 1`: one step forward,
   then subtract 1. While `through_carry <= -1`: one step back, then add 1.

A step: take `(origin, dir) = editor::Viewport { camera: self.camera, size_px:
viewport_px }.ray(cursor_px)` and make `dir` unit length. Forward: `d =
through_distance(&scene.ray_hits(origin, dir))`; `Some(d)` moves `self.camera.target` by
`dir * d`. Back: the same with `-dir` (moves the target by `-dir * d`). `None`: call
`self.scroll_zoom(1.0, ...)` for a forward step or `self.scroll_zoom(-1.0, ...)` for a
back step, with the same cursor, scene and viewport. The camera's yaw, pitch, distance,
field of view and projection are never changed by a step that moves through a surface.
`pivot` and `zoom_pivot` are never changed.

### App wiring (`crates/modeler/src/app.rs`, `viewport_input`)

Where the frame's `MouseWheel` lines are summed today, keep two sums: events whose
`modifiers.command` is set (Ctrl on Windows and Linux, Cmd on macOS) go to a Ctrl sum, the
rest to the plain sum (same unit conversion for both). After the loop, a non-zero plain sum
calls `scroll_zoom` as today and a non-zero Ctrl sum calls `self.view.scroll_through(sum,
local(cursor), scene, size)` with the same arguments; either marks the view dirty.

## Tests

- `crates/modeler-dae/tests/flat_ray_hits.rs` (7, one property test against `raycast`).
- `crates/modeler/tests/viewstate_through.rs` (15).

The window wiring has no automated test; the orchestrator checks Ctrl+wheel in the real
window.

## Out of scope

A key to step through without the wheel, an animation of the step, clipping planes or
section cuts, remembering where a step started so a notch back returns there, stepping
through in parallel projection.
