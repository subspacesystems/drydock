# m3-tape: tape measure and guides (kernel side)

Implement `app/crates/modeler-kernel/src/tape.rs` (skeleton on main; replace every `unimplemented!`). Only that file changes. Tests: `tests/tape.rs`, `tests/tape_props.rs`. Units are meters; use `crate::geom::LINEAR_TOL` (1e-5) as the length tolerance.

A guide is a construction aid, not geometry. This task is the pure data and math; storing guides in the model, undo, and the tool are later tasks.

## Types
- `Guide::Line { point, dir }`: infinite line through `point`, `dir` of length 1. `Guide::Point(p)`.
- `GuideId(u64)`.

## Functions
- `measure(a, b)`: straight-line distance.
- `parallel_guide(a, b, toward, typed)`: a line parallel to the edge `a`→`b`, on the side of `toward`.
  - Return `None` if any input is not finite, if `|b - a| < LINEAR_TOL`, or if `toward` is within `LINEAR_TOL` of the edge's line (no side).
  - Let `u` = unit `a`→`b`. `dir` is `u` (so reversing the edge reverses `dir`).
  - The offset direction is the part of `toward - a` perpendicular to `u`, normalised (it may point out of any plane; this is 3D).
  - The offset distance is `typed` if given, else the distance from `toward` to the edge's line. A typed distance must be finite and greater than `LINEAR_TOL`, else `None`; it never changes the side.
  - `point` = `a` + offset direction × distance.
- `point_guide(p)`: `Some(Guide::Point(p))`, `None` if `p` is not finite.
- `Guide::distance_to(p)`: distance from `p` to the line (ignoring position along it) or to the point.

## `Guides`
Ordered collection. `add` returns a fresh `GuideId`; ids are never reused, not after `remove` and not after `clear`. `remove` returns the guide; `clear` returns everything it removed as `(id, guide)` in creation order; `iter` is in creation order; `get`, `len`, `is_empty`. `Clone` makes an independent copy.
