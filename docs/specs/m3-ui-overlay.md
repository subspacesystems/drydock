# m3-ui-overlay: overlay primitives for the M3 tools

`app/crates/modeler/src/overlay.rs`: four pure functions that turn tool state into world-space
drawing primitives (lines, outlines, markers) with a `Style` tag. The window task
(`m3-app-ui`) projects them and picks colours; nothing here knows about pixels or egui. Tests:
`app/crates/modeler/tests/overlay.rs`.

Only `overlay.rs` changes. Types and signatures are on main; fill in the bodies (private helpers
are fine). Output order is part of the contract, as given below. All angles are degrees.

## `face_highlight(ctx, face)`

One `Primitive::Polygon` with `Style::FaceHighlight` per loop of the face: the outer loop first,
then the holes in their stored order. Points are the loop's vertex positions in loop order
(`Context::loop_points`). An unknown face gives an empty list.

## `scale_handles(bounds, hover, grabbed)`

1. The box: one `Primitive::Line` with `Style::BoundsBox` for each distinct edge of the box
   `bounds.min`..`bounds.max`. Edges shorter than 1e-9 are left out and so are repeats of the same
   segment (in either direction), so a box flat in one direction has 4 lines, a box flat in two
   has 1, and a point has none. Order: the lines along x first, then y, then z; within a
   direction, by increasing (other-axis, other-axis) corner as you like — tests compare sets.
2. One `Primitive::Marker` per handle in `scale::handles(bounds)` order, at `bounds.point(handle)`.
   Style: `HandleGrabbed` for the grabbed handle, else `HandleHover` for the hovered one, else
   `Handle`. A hover or grab that is not one of the box's handles changes nothing.

## `protractor(center, plane, radius, start, angle)`

`radius` not finite or 0 or less: empty list. Otherwise, in this order:

1. The ring: 24 `Line`s, `Style::ProtractorRing`, line `k` from the point at `15*k` degrees to the
   point at `15*(k+1)` degrees. The point at angle `t` is
   `center + radius * (cos t * plane.u + sin t * plane.v)`.
2. The ticks: 24 `Line`s, `Style::ProtractorTick`, tick `k` from `center + 0.9 * (point k - center)`
   to point `k` (so they sit on the ring).
3. Arms, `Style::ProtractorArm`, only when `start` is given and not zero-length: `start` is a
   direction in the plane (normalise it; ignore any component along the plane normal). First arm
   from `center` to `center + radius * start_dir`. When `angle` is also given, a second arm from
   `center` to `center + radius * d`, where `d` is `start_dir` turned by `angle` counter-clockwise
   seen from the front of the plane (the side `plane.u x plane.v` points to), the same turn the
   Rotate tool makes. A non-finite `angle` or `start` counts as not given.

## `guides(guides, around, half_length)`

One primitive per guide, in creation order (`Guides::iter`):

- `Guide::Point(p)`: `Marker { at: p, style: GuidePoint }`.
- `Guide::Line { point, dir }`: a `Line` with `Style::Guide` from `c - dir * half_length` to
  `c + dir * half_length`, where `c` is the point of the guide line closest to `around`. When
  `half_length` is not finite or 0 or less the line is left out (points are still listed).
