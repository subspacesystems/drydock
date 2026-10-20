# m2-shapes: points of rectangles, circles and arcs

Milestone M2. Crate `app/crates/modeler-kernel`, file `src/shapes.rs` only. Replace the `unimplemented!("m2-shapes")` bodies; don't change public signatures. Uses `glam` and `geom::LINEAR_TOL` (1e-5 m). Tests: `tests/shapes.rs`, `tests/shapes_props.rs` (fixed; don't edit).

The drawing tools call these to get the points of a shape from the user's clicks; they then add the edges. Nothing here touches a model. Functions never panic; for input that makes no shape they return `None` or an empty `Vec`.

## `rectangle(a, b, u, v)`
`u` and `v` are unit, perpendicular directions of the drawing plane (the caller guarantees it; don't check). `b` need not lie in the plane. Let `du = (b - a)·u` and `dv = (b - a)·v`. The result is `[a, a + du·u, a + du·u + dv·v, a + dv·v]`, in that order (counter-clockwise about `u × v` when `du` and `dv` have the same sign, clockwise otherwise; don't reorder). `None` when `|du|` or `|dv|` is at most `LINEAR_TOL`, or either is not finite.

## Common frame for `circle` and `arc`
`n` is `normal` made unit length. `x` is `x_dir` with its component along `n` removed, made unit length. `y = n × x`. Angle `t` names the point `center + radius·(cos t · x + sin t · y)`. Both functions return an empty `Vec` when `normal` is zero or not finite, when `x_dir` is zero, not finite or parallel to `normal` (nothing remains after removing the `n` part: its remaining length is at most 1e-9), or when `radius` is not finite or at most `LINEAR_TOL`.

## `circle(center, normal, x_dir, radius, segments)`
Fewer than 3 `segments` gives an empty `Vec`. Otherwise exactly `segments` points, point `k` at angle `2π·k/segments` (so the first is `center + radius·x`, and the points run counter-clockwise about `normal`). Every point is `radius` from `center`, in the plane through `center` perpendicular to `normal`. The last point is not a repeat of the first.

## `arc(center, normal, x_dir, radius, start, sweep, segments)`
Empty `Vec` when `segments` is 0, when `start` or `sweep` is not finite, when `|sweep|` is below 1e-9 or above `2π + 1e-9`. Otherwise exactly `segments + 1` points, point `k` at angle `start + sweep·k/segments`. A negative `sweep` runs clockwise. The first point is at angle `start`, the last at `start + sweep` (for `sweep = 2π` the last coincides with the first).

## `sweep_angle(center, normal, from, to)`
Used when the user clicks the arc's start and end: the angle, counter-clockwise about `normal`, from the direction of `from` to the direction of `to`, both as seen from `center` after dropping their component along `n`. Result in `[0, 2π)`. `None` when `normal` is zero or not finite, or when either direction has a remaining length of at most `LINEAR_TOL`. Two directions equal within rounding give `Some(0.0)` (never a value just below 2π: anything above `2π - 1e-9` is returned as 0.0). A quarter turn counter-clockwise gives `π/2`; a quarter turn clockwise gives `3π/2`.
