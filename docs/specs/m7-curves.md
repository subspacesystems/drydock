# m7-curves: arcs and circles stay curves (kernel, native format v6)

A circle or arc is drawn as a chain of straight edges. Until now the kernel forgot the circle
right after drawing it. For 3D printing the exporter must re-segment round parts at a chosen
chord tolerance (`m7-print-mesh`), and later Entity Info can show a radius. So the kernel now
keeps each drawn arc or circle as a **curve**: center, plane normal, radius, start and end
angles, plus the chain of vertices it runs through. Curves follow every geometry edit (move,
rotate, scale, mirror, sticky splits, erase, undo/redo) and are saved in native format
version 6.

## Files

Already on main (don't redesign):
- `topology.rs`: `CurveId`; `Context::curve`, `curves`, `curve_count`, `insert_curve`,
  `delete_curve`; `Record::AddCurve` / `RemoveCurve` (applied, inverted, compared in
  `Context` equality); `TopoError::MissingCurve`; `Violation::BadCurve`, reported by
  `Context::validate` (step 7) for every curve that fails `curve::fits`.
- `model.rs`: `Model::edit_geometry` calls `curve::reconcile` after the closure whenever the
  body has curves; `clone_body_into` copies curves.
- `curve.rs`: the `Curve` struct, `is_closed`, `sweep`, `y_axis`, `point_at`, the constants,
  and `unimplemented!()` bodies for the rest.
- `tools/mod.rs`: `draw_curve` signature.
- `docs/format.md`: the version 6 `curve` record.

You implement, changing only these files:
- `app/crates/modeler-kernel/src/curve.rs`: `segments_for`, `points`, `fits`, `fit`,
  `add_curve`, `curve_edges`, `curve_of_edge`, `reconcile` (private helpers welcome).
- `app/crates/modeler-kernel/src/tools/mod.rs`: `draw_curve` (share the body of
  `draw_segments`; don't duplicate it).
- `app/crates/modeler-kernel/src/tools/circle.rs`, `arc.rs`, `arc3.rs`: place their shape
  with `draw_curve` instead of `draw_segments` (circle: `closed = true`; both arc tools:
  `closed = false`). Previews and everything else stay as they are.
- `app/crates/modeler-kernel/src/format.rs`: version 6.

Acceptance tests: `app/crates/modeler-kernel/tests/curves.rs`, `curves_format.rs`,
`curves_props.rs`; `tests/units_model.rs` no longer pins version 5. Every existing test must
still pass: the circle and arc tools now make curves, so their tests and the tool property
tests run through your code.

## Terms

- **Curve**: `center`, unit `normal`, unit `x_axis` perpendicular to it, `radius > 0`,
  angles `start < end` in radians, and `vertices`, the chain. `y_axis = normal × x_axis`;
  `point_at(a) = center + radius · (cos a · x_axis + sin a · y_axis)`. The chain runs
  counter-clockwise about `normal` from angle `start` (first vertex) to `end` (last vertex).
- **Closed** (a circle): the chain repeats its first vertex at the end, and `end - start` is
  `2π`. **Open** (an arc): no vertex repeats and `0 < end - start < 2π`.
- **Distinct vertices**: the chain without the closing repeat. A curve has at least 3, so at
  least two steps. (A one-segment arc is just an edge.)
- **Angle of a point** `p` on a curve: `atan2((p - center) · y_axis, (p - center) · x_axis)`.
- **Turn** from `p` to `q`: `(angle(q) - angle(p)).rem_euclid(2π)`, the counter-clockwise
  angle between them.
- **Step**: two consecutive chain vertices `a`, `b`. The step is **intact** when both exist
  and an edge joins them, or a **straight run** does: a path of edges from `a` to `b` whose
  inner vertices all lie on segment `ab` (within `LINEAR_TOL`) and get strictly further from
  `a` along it. That is how a chord looks after another line was drawn across it: the
  sticky kernel split it at the crossing, and the curve keeps both pieces. To find the run,
  walk from `a`: from the current vertex take the edge whose other end is `b`, otherwise the
  one whose other end lies on the segment beyond the current vertex (nearest first). Use
  `Context::edges_of_vertex`; `edge_between` scans every edge and is too slow here.
- **Step edges** of a curve: the edges of all its steps, in chain order.

## When a curve fits (`fits(ctx, curve)`)

True when all of these hold, else false:
1. Every number is finite; `radius > LINEAR_TOL`; `normal` and `x_axis` have length 1 and
   `|normal · x_axis|` is at most `1e-9`.
2. At least 3 distinct vertices, none repeated (apart from a closed chain's last).
3. Closed: `|end - start - 2π| <= 1e-9`. Open: `0 < end - start < 2π`.
4. Every chain vertex exists and lies on the circle: with `d = p - center`, `h = d · normal`
   and `ρ = |d - h · normal|`, `hypot(h, ρ - radius) <= LINEAR_TOL`.
5. The first vertex is within `LINEAR_TOL` of `point_at(start)` and the last of
   `point_at(end)`.
6. Each step's turn is in `(0, π)` (strictly), and the turns add up to within `π` of
   `end - start` (so the chain goes around once, the right way).
7. Every step is intact.

Pure: nothing changes.

## Fitting a curve (`fit(ctx, vertices, start)`)

The circle through the chain `vertices`, with angle `start` at the first vertex:
1. `None` for a non-finite `start`, fewer than 3 distinct vertices, or a vertex that doesn't
   exist. The chain is closed when its first and last ids are the same.
2. With `m` distinct points `p`, take three spread ones: indices `0, m/3, 2m/3` (integer
   division) for a closed chain, `0, (m-1)/2, m-1` for an open one. Their circumcenter is
   the `center` (`u = b - a`, `v = c - a`, `w = u × v`: `a + (|u|² (v × w) + |v|² (w × u)) /
   (2|w|²)`); `None` when `|w|²` is zero or not finite (points on a line).
3. `normal` is `w` normalized, negated if the chain runs clockwise about it: if the sum over
   the chain's steps of `(p_i - center) × (p_i+1 - center)` points against it.
4. `radius = |p_0 - center|`. With `d` the unit direction from `center` to `p_0` (made
   perpendicular to `normal`), `x_axis = d · cos(start) - (normal × d) · sin(start)`,
   normalized, so `point_at(start)` is `p_0`.
5. `end = start + 2π` when closed, else `start` plus the sum of the step turns.
6. Return the curve only if it `fits`; otherwise `None`.

Three distinct points always lie on some circle, so a 3-vertex curve survives any affine map
of its vertices (re-fitted). That's fine.

## Adding a curve (`add_curve(ctx, points, closed, log)`)

1. Each point must be a vertex: `ctx.find_vertex(p, LINEAR_TOL)`; any miss → `None`.
2. The chain is those vertices; if `closed` and the last isn't the first, append the first.
   (If `closed` is false but the first and last points found the same vertex, the chain is
   closed anyway: a full-turn arc is a circle.)
3. `fit(ctx, chain, 0.0)`; `None` → `None`.
4. An edge belongs to at most one curve: if any step edge already belongs to a curve
   (`curve_of_edge`), `None`.
5. `ctx.insert_curve` and return its id. On every `None`, `ctx` and `log` are unchanged.

## `curve_edges(ctx, id)` and `curve_of_edge(ctx, edge)`

`curve_edges`: the curve's step edges in chain order; empty for an unknown id or a curve with
a broken step. `curve_of_edge`: the lowest curve id whose step edges contain `edge`, else
`None`.

## Keeping curves through edits (`reconcile(ctx, log)`)

`Model::edit_geometry` runs it after every geometry edit on a body with curves, with `log`
holding that edit's records. It brings each curve in line with the geometry and appends its
own records to `log`, so undo and redo restore everything exactly through the normal records
(and never call `reconcile`).

1. If `log` has no `MoveVertex`, `RemoveVertex` or `RemoveEdge` record, return: nothing a
   curve depends on changed. (Adding edges never breaks a step: a split chord becomes a
   straight run.)
2. For each curve in ascending id order:
   - If it `fits`, leave it alone: no records. This covers moves that leave its vertices
     where they were, edits elsewhere in the body, and lines drawn across its chords.
   - Else, if every step is intact, `fit(ctx, vertices, start)` with the old `start`. Found:
     replace the curve under the **same id** (`RemoveCurve` of the old, then `AddCurve` of
     the new). Not found: remove it; its edges stay as plain edges. This is how a move,
     rotation, uniform scale or mirror carries a curve (the center and frame follow the
     vertices, the radius scales, `start` and `end` keep their values), and how dragging
     part of it, or stretching it in its plane, turns it into plain edges.
   - Else (some step is broken: an edge erased, a vertex gone or merged away) remove the
     curve and split its chain into **runs**: maximal sequences of consecutive intact steps.
     On a closed curve a run may wrap past the closing vertex. Runs of fewer than 2 steps
     are dropped (plain edges). For each remaining run, in the order of its first step's
     index in the old chain: the run's vertices on the **old circle** (same center, normal,
     x axis, radius) with `start = old.start + (angle of the run's first vertex - old.start)
     .rem_euclid(2π)` and `end = start +` the sum of its step turns, if that `fits`;
     otherwise `fit(ctx, run, that start)`; otherwise the run is dropped. The first run
     added takes the old id; the others get fresh ids (`insert_curve`).

So erasing one edge of a 24-segment circle leaves a 23-step arc starting at the next vertex;
erasing two opposite edges leaves two arcs; an arc whose middle edge is erased becomes two
arcs, and the first keeps its id.

## Segments for export (`segments_for`, `points`)

The exporter will replace a curve's chain with finer or coarser chords. The **sagitta** of
`n` equal segments is `2 · radius · sin²(sweep / (4n))` (the largest gap between a chord and
the arc).
- `segments_for(tolerance)`: read a tolerance below `MIN_CHORD_TOL` or not finite as
  `MIN_CHORD_TOL`. Return the smallest `n` with sagitta `<= tolerance`, but at least 3 for a
  closed curve and 1 for an open one, and at most `MAX_CURVE_SEGMENTS`. Compute a first guess
  from `acos` and then step to the exact smallest `n` with the sagitta formula itself (the
  tests check `n` and `n - 1` with it). A curve with a bad radius or sweep gives the minimum.
- `points(tolerance)`: with `n = segments_for(tolerance)`, the `n + 1` points
  `point_at(start + sweep · k / n)`; the last is `point_at(end)` for an open curve and an
  exact copy of the first for a closed one.

## Drawing tools (`draw_curve`)

`draw_curve(target, points, closed, label)` draws the segments between consecutive points
(and from the last back to the first when `closed` and there are more than 2 points) exactly
as `draw_segments` does, including face materials and failure handling, then, inside the
same `edit_geometry` call, `add_curve(ctx, points, closed, log)`. One undo step. Returns the
edge chains and the curve id (`None` when the points don't make a curve; the edges are drawn
anyway). No points or segments: `Ok((vec![], None))` and no undo step, like
`draw_segments`.

The Circle tool passes its points with `closed = true`. The Arc and 3-point Arc tools pass
their `segments + 1` points with `closed = false`; a 360° arc then ends on its first vertex
and becomes a closed curve; a 1-segment arc has only 2 points, so no curve. With
`start = 0`, the curve's `x_axis` points from the center to the first point, so a circle's
x axis is the direction to the clicked rim point. An arc drawn clockwise about the drawing
plane's normal (possible with the 3-point arc) gets the opposite normal.

## Native format, version 6

`docs/format.md` (already updated on main) is the reference. In short:
- `curve <id> <cx> <cy> <cz> <nx> <ny> <nz> <xx> <xy> <xz> <radius> <start> <end> <v0> <v1>
  ...`: the fields of `Curve` in that order, then the chain's vertex ids (a closed chain
  ends with its first id again). Numbers are written with `Display` like the others, ids
  as `u64`.
- Writers put a body's `curve` records right after its `face` records, ascending id. The
  header is `6` when any body (root or definition) has a curve; otherwise the rules for 1 to 5
  are unchanged, so a model without curves writes the same text as before. Bump
  `FORMAT_VERSION` to 6.
- Reading: a `curve` record in a version 1 to 5 file is `UnknownRecord`. Outside a body:
  `NoBody`. Fewer than 17 tokens, a number that isn't finite, or an id that isn't a `u64`:
  `BadField`. A vertex the current body doesn't have: `Rejected(ModelError::Topo(
  TopoError::MissingVertex(id)))` (the first such vertex in the chain). A curve that doesn't
  `fits` the body's geometry as read so far: `BadField`. A repeated curve id:
  `Rejected(ModelError::Topo(TopoError::IdInUse))` (from applying the record). Version 5
  and older files read as before.

## Notes

- Tolerances: positions use `geom::LINEAR_TOL` (10 µm); unit vectors and the closed sweep
  `1e-9`. The tests compare a moved curve with the matrix applied to the old one at `1e-9`,
  so derive the new curve from the vertices with the formulas above (circumcenter of three
  spread points), not from a least-squares fit.
- Speed: `reconcile` runs after every geometry edit on a body with curves. Return early as in
  step 1 and when a curve still fits; find steps through `edges_of_vertex`. `curve_of_edge`
  may scan all curves.
- `Context::apply` doesn't look at curve vertices (undo may add a curve back before its
  vertices); don't add checks there.
- Use `crate::geom` helpers where they fit (`point_segment_distance`, `LINEAR_TOL`).

## Not in scope

Carrying curves into and out of groups and components (make group/component, explode,
copy/paste, import) and giving push/pull's new cap a curve are the next task,
`m7-curves-carry`; here those operations leave plain edges where they rebuild geometry, which
`reconcile` already handles. Also not here: picking a whole curve by clicking one of its edges,
showing curves in Entity Info, and the exporter that uses `points`.
