# m9-sweep-curves: curves carried through a sweep (kernel)

Implement `app/crates/drydock-kernel/src/sweepcurve.rs` (skeleton on main: `CurveCarry`,
`CurveCarry::new`, `CurveCarry::add_curves`) and wire it into
`app/crates/drydock-kernel/src/sweepface.rs`. Change only those two files.
Acceptance tests: `app/crates/drydock-kernel/tests/sweep_curves.rs`,
`tests/sweep_curves_props.rs`, and one changed assertion in `tests/sweep_face.rs` (fixtures in
`tests/sweep_face_common/` and `tests/pushpull_common/`).

`sweep_face` (`docs/specs/m9-sweep-face.md`) builds the swept faces from plain edges. A curve
(`curve.rs`, `docs/specs/m7-curves.md`) is a chain of vertices that lies on a circle; the app
selects, measures and exports it as one arc. This task makes the sweep leave curves where the
new geometry really lies on circles:

- **Section curves**: a curve of the profile (a round profile, a round hole, an arc on the
  outline) reappears on each new section whose carried chain is still a circle: the end cap of
  a straight or mitred path square to the profile, every corner of a ring round a regular
  polygon when the profile stands in a corner's mitre plane, the mitres along an arc of the
  path.
- **Curves along the path**: where the path runs along a curve (an arc or circle of the
  path), each profile vertex traces an arc (or circle) round the same axis.

Mitre sections that are slanted to the profile are ellipses, not circles; they stay plain
edges. That is expected, not a failure.

## Wiring in `sweepface.rs`

1. Add a field `at: Vec<usize>` to `Plan`: `at[s]` is the index into `path.points` of
   section `s` (`plan.sections[s]`).
   - Open path: `at = [0, 1, .., k-1]`, reversed together with the points when `plan_open`
     reverses the path.
   - Closed, profile at vertex `j` (`CandidateKind::Vertex(j)`): `at[s] = (j + s) % k`.
   - Closed, profile part way along segment `i` (`CandidateKind::Segment(i)`):
     `at[s] = (far + s) % k` with `far = (i + 1) % k`.
2. In `sweep_face`, before the edit (where `old_materials` etc. are read), build
   `CurveCarry::new(ctx, profile, path, &plan.at)` from the body's geometry.
3. Pass it to `build_open` / `build_closed` and call `add_curves(ctx, &section_vertices, log)`:
   - open: after `build_sides` and `build_cap`, **before** `merge_sides`;
   - closed: right after `build_sides`, **before** the profile face is deleted and before
     `merge_sides`.

   Before the merge on purpose: when coplanar sides merge, `erase_edge` takes some section
   edges away, and `curve::reconcile` (run by `Model::edit_geometry` after the closure,
   because the log now holds `RemoveEdge` records) splits those curves into arcs or drops
   them exactly as after any other erase. The same holds for the profile's own curve when a
   ring deletes the profile. Don't call `reconcile` yourself.
4. Nothing else changes: same checks, errors, faces and ids as now; `sweep_sections` is
   untouched. A refused sweep rolls back the whole edit, curve records included.

## Reading the curves (`CurveCarry::new`)

All read from `ctx`, the body before the sweep.

- `profile`: for the profile face's outer loop, then each hole in the face's order,
  `Context::loop_vertices(loop)`. These line up with the sections: `sections[s][l][p]` is
  where the sweep put `profile[l][p]` at section `s`.
- `curves`: every curve of the context (ascending id) whose `curve::curve_edges` is not empty
  and are all edges of the profile's loops. Cloned.
- `steps`: one entry per sweep step, in sweep order. An open sweep with `S` sections has
  `S - 1` steps (step `s` from section `s` to `s + 1`); a ring has `S` steps (the last from
  section `S - 1` back to section 0). Step `s` runs along the path segment between path
  indices `at[s]` and `at[(s + 1) % S]`: on an open path segment `min` of the two, on a
  closed path segment `at[s]`. The entry is `Some(c)` when all of these hold, else `None`:
  - `path.edges[seg]` is an edge of `ctx`,
  - its two ends are `path.vertices[seg]` and `path.vertices[(seg + 1) % k]` (either way
    round),
  - `curve::curve_of_edge(ctx, that edge)` is `Some(c)`.

  So paths of bare points with made-up ids (most of the `sweep_face` tests) have no curve
  steps.
- `closed`: `path.closed`.

## Adding the curves (`add_curves`)

If `curves` is empty and every step is `None`, add nothing.

**Adding a chain.** Every candidate chain below goes through the same steps:

1. `curve::fit(ctx, chain, start)`. `None`: the chain is not a curve; add nothing.
2. Its step edges: `curve::step_edges` for each pair of consecutive chain vertices (now
   `pub(crate)`). If any of them already belongs to a curve (one in the context when
   `add_curves` started, or one added earlier in this call), add nothing. This keeps the
   profile's own curve as the first section's when the sweep reuses the profile's vertices
   and edges, and the path's own curve when a profile vertex sits on the path and its side
   edges are the path's edges. Collect the edges of all existing curves once at the start and
   add to that set as you go; `curve_of_edge` per edge is too slow.
3. Otherwise `ctx.insert_curve(curve, log)` and push the id onto the result.

**Section curves**, first. For each section `s = 0, 1, .., S-1`, for each profile curve in
`curves` order: the chain is the curve's `vertices` with each vertex `profile[l][p]` replaced
by `sections[s][l][p]` (a closed curve stays closed: its repeated first vertex maps to the
same section vertex). `start` is that profile curve's `start`. Section 0 of an open sweep is
the profile itself, so step 2 always skips it; nothing special-cases it.

**Curves along the path**, after all section curves. A **run** is a maximal sequence of
consecutive steps that all have the same `Some(c)`. On a ring a run may wrap from the last step
to step 0; if every step of a ring has the same `Some(c)`, that is one **whole-ring run**.
Runs of fewer than 2 steps add nothing. Take the runs in the order of their first step's
index (a wrapping run's first step is its high-index one). For each run, for each loop `l`
and point `p` in `profile` order:

- Whole-ring run: the chain `sections[0][l][p], sections[1][l][p], .., sections[S-1][l][p],
  sections[0][l][p]` (closed), `start = 0.0`. Only this chain.
- Other runs, steps `s0 .. s0 + L - 1`: the sections `s0, s0 + 1, .., s0 + L` (mod `S` on a
  ring), `L + 1` vertices, `start = 0.0`. Try, in this order, the whole chain, the chain
  without its last vertex, without its first, without both; skip any with fewer than 3
  vertices. The first that `fit` accepts is the only one tried: go on to step 2 with it, and
  if step 2 rejects it, add nothing for this point.

  Why trimming: the sections at a run's two ends are the profile's own plane, a mitre with a
  straight segment, or an end cap square to the last chord; those generally lie off the
  circle the inner mitres share (the inner mitres of a regular arc are the planes through its
  axis). So an arc of 6 steps with the profile in the plane through the axis at its first
  vertex gives arcs of 5 steps (the end cap is off); a profile square to the first chord
  gives 4 (both ends off); a straight lead-in into the arc gives 4.

Return the ids in the order added.

## Results the tests rely on

- Ids of new curves are fresh (above every existing id), in the order above: section curves
  by section then profile curve, then curves along the path by run, loop, point. A chain
  along the path starts at the first section it keeps, so a whole-ring run's circles start
  at section 0: the profile's own vertices when the profile stood at a corner.
- Existing curves that still fit are untouched (same id, equal `Curve`): the path's curve, the
  profile's curve on an open sweep's start cap.
- Every curve after the sweep `fits`, and no edge belongs to two curves.
- Undo of the sweep gives back the model as it was, curves included; redo gives the same
  model as after the sweep.
- Saved and loaded with the native format (v6 curve records), the curves come back. No
  format change.

## Tests

- `sweep_curves.rs`: a circle on a straight path reappears on the end cap (same `start`); an
  L path's slanted corner is no circle, its end cap is; an arc on the profile outline is
  carried; a round hole is carried through a tube; a ring round a regular 12-gon carries the
  circle to all 11 other corners (profile curve kept); a circle part way along a side leaves
  no curves; a refused sweep leaves curves alone; level sides that merge split mitre circles
  into two arcs each; a square along a quarter arc leaves one 5-step arc per corner (path
  curve unchanged); square to the first chord, 4-step arcs; a straight lead-in, 4-step arcs;
  a ring round a circle path, one circle per corner; a circle round a circle, 25 curves in
  order, undo and redo; a profile corner on the path doesn't double the path's curve; save
  and load.
- `sweep_curves_props.rs`: a square or even-sided round profile swept round a random arc or
  circle (any orientation) gives one curve per profile vertex round the axis, `n - 1`
  section circles when round, path curve unchanged, all curves sound, undo and redo exact; a
  circle with any `start` swept straight reappears on the end cap moved by the path.
- `sweep_face.rs`: `a_curved_profile_keeps_its_curve_on_the_start_cap` now expects 2 curves
  (the end cap's is new).

## Not in scope

- Smooth or soft edge flags: the model has none yet (no flag on edges, nothing in the
  format), so there is nothing to carry. Shading the swept surface smooth is a later task.
- Ellipses or any non-circular curve type; slanted mitre sections stay plain edges.
- Changing the path's edges or curves, or merging collinear edges.

## Where to look

- `sweepface::sweep_face`, `plan_open`, `plan_closed`, `Plan`, `build_open`, `build_closed`,
  `build_sides`, `assign_vertices`, `merge_sides` (in `sweepface.rs`)
- `CurveCarry` (in `sweepcurve.rs`), `sweep::SweepPath`
- `curve::fit`, `curve::fits`, `curve::step_edges`, `curve::curve_edges`,
  `curve::curve_of_edge`, `curve::reconcile`, `Curve`
- `Context::curves`, `Context::curve`, `Context::insert_curve`, `Context::loop_vertices`,
  `Context::edge`, `Context::face`, `Model::edit_geometry`
- Test helpers: `curved_face`, `body_path`, `curve_path`, `new_curves`,
  `assert_curves_sound`, `ring12`, `quarter_arc`, `radial_square`, `radial_circle`
  (`sweep_curves.rs`); `path`, `sweep`, `rect_points` (`sweep_face_common`); `mesh`, `ctx`, `v`
  (`pushpull_common`)
