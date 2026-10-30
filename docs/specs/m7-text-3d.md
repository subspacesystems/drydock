# m7-text-3d: emboss or engrave text loops on a face

3D text for printing: flat letter outlines lying on a face of a part are drawn onto that face
and pushed out of it (emboss, raised letters) or into it (engrave, sunk letters), so the
part stays one solid. This task is the kernel operation only; the font outlines come from
`modeler-dae`'s `text_outline` and the tool that places them is `m7-text-3d-ui`. Here the
input is plain 3D loops already lying on the face's plane.

Skeleton on main: `app/crates/modeler-kernel/src/text3d.rs` (`TextMode`, `Text3dError` with
its `Display` messages done, `TextFaces`, `check_text`, `text_3d`; `pub mod text3d` in
`lib.rs`). Two helpers were made reusable for this task:
`pushpull::push_pull_into` (push/pull without its own undo step: appends its records to a
`ModelEdit` you pass, leaves the model and that edit untouched on an error) and
`offset::edge_pierces(ctx, edge, face)` (does one edge cross or touch one face it shares no
vertex with).

Files to change: `app/crates/modeler-kernel/src/text3d.rs` only.
Acceptance tests: `app/crates/modeler-kernel/tests/text_3d.rs`, `tests/text_3d_props.rs`
(fixtures from `tests/offset_common/mod.rs`).

Lengths are model units (meters); `LINEAR_TOL` is `geom::LINEAR_TOL` (1e-5).

## Terms

- **n**: the target face's front normal, `ctx.face_plane(face).normal`.
- **Out**: the direction away from the material: `n`, or `-n` when
  `check_geometry(ctx).reversed_faces` contains the face (its front points into the part).
- **Depth of loop i**: how many other loops contain loop i's first point, using
  `point_in_polygon_2d(.., LINEAR_TOL) == Inside` on the loops projected with
  `to_plane_2d(.., n)`. Even depth (0, 2, ...) is a **letter** loop (an outer outline or an
  island in a counter); odd depth is a **counter** (the hole in an `O`). The winding of the
  loops is ignored.
- **Touch**: two closed loops touch when some segment of one and some segment of the other
  give `geom::segment_intersection(a0, a1, b0, b1, LINEAR_TOL) != SegmentHit::None`. A loop's
  segments run from each point to the next and from the last back to the first. You may skip
  pairs whose bounding boxes, grown by `LINEAR_TOL`, don't overlap.
- **Diameter** of a point set: the largest distance between two of its points.

## `check_text(ctx, face, loops, mode, height)`

Checks, in this order, and returns the first refusal; `Ok(())` when all pass. It changes
nothing.

1. `face` missing from `ctx`, or `ctx.face_plane(face)` is `None`: `MissingFace(face)`.
2. `height` not finite, or `height <= LINEAR_TOL`: `BadHeight`.
3. `loops` empty: `NoText`.
4. Loop by loop, in index order (all three checks on loop 0, then loop 1, ...):
   fewer than 3 points: `BadLoop(i)`; a point whose distance from the face's plane is
   more than `LINEAR_TOL`: `OffPlane(i)`; `!polygon_is_simple_2d(to_plane_2d(loop, n),
   LINEAR_TOL)`: `BadLoop(i)` (covers repeated points, crossings, zero area).
5. For each pair `i < j`, ordered by `i` then `j`: loops i and j touch: `LoopsCross(i, j)`.
6. The diameter of all loop points together `>=` the diameter of the face's outer loop
   points `- LINEAR_TOL`: `TooSmall` (the text can't fit however it is turned).
7. For each loop i in order: a point that is not strictly inside the face (inside the outer
   loop, `Inside` and not `Boundary`, and `Outside` every hole of the face), or loop i
   touches any loop of the face (outer or hole): `CrossesBorder(i)`.
8. Engrave only: the **thickness** is the smallest `t > LINEAR_TOL` over every point `p` of
   every loop and every other face `g` of `ctx` (not `face`) where the ray `p - out * t`
   meets `g`'s plane (skip `g` when `|g.normal . out| < 1e-9`) at a point inside `g` or on
   its border (not `Outside` its outer loop, not strictly `Inside` a hole). No hit: infinite.
   `height >= thickness - LINEAR_TOL`: `TooDeep { thickness }`.

Emboss has no height limit in `check_text`; what it would run into is found while building.

## `text_3d(model, body, face, loops, mode, height, undo, label)`

On any error the model and `undo` are as they were: roll back everything recorded so far
with `model.apply_edit(&edit.inverse())`. On success there is exactly one undo step,
labelled `label`.

1. `body` missing: `MissingBody(body)`. Then `check_text` on the body's geometry; return its
   error.
2. Before changing anything, keep: `before = check_geometry(ctx)`, the ids of every edge and
   face of the body, and each loop's depth. The push distance is `height` for emboss and
   `-height` for engrave, negated when the face is in `before.reversed_faces` (so emboss
   always goes out of the material and engrave into it).
3. **Draw.** In one `model.edit_geometry(body, &mut edit, ..)`, for each loop in order and
   each of its segments, `ctx.add_edge(a, b, log)`; the face splits on its own when a loop
   closes. Keep the union of the returned `EdgeChain::edges` per loop. A `StickyError`:
   `Draw(e)`; a `ModelError` from `edit_geometry`: `Model(e)`.
4. **Find the letter faces.** For each letter loop (even depth), in loop order, the face of
   the body whose outer loop uses exactly that loop's edge set. None found: `BadLoop(i)`.
   Find all of them before pushing any. Counter faces are left where they are: on emboss the
   counter stays at the face (the hole of a raised `O`), on engrave it stays as a post.
5. **Push.** For each letter face in that order, `push_pull_into(model, body, f, distance,
   &mut edit)`; an error: `PushPull(e)`. Collect each result's `cap` into `caps` and append
   its `sides` to `sides`.
6. **Pierce check.** With the edges and faces that weren't in the body before step 3 as
   "new": if any new edge pierces any face, or any edge pierces any new face
   (`offset::edge_pierces`): `Intersects`. (A letter pushed exactly onto a facing parallel
   face is joined to it by push/pull and is not an error.)
7. **Solid check.** `after = check_geometry`. What is wanted: if `before.is_clean()`, the
   result must be clean; else if `before.is_solid()`, it must be solid; else nothing (text on
   a lone face or an open surface). If `after` doesn't meet it, run `repair::repair_geometry`
   in another `edit_geometry` call on the same edit and set `repaired = Some(report)`; if the
   report's `after` still doesn't meet it: `NotSolid(Box::new(report.after))`. Otherwise
   `repaired` is `None`.
8. `undo.push(label, edit)` and return `TextFaces { caps, sides, repaired }`.

The volume of a solid part changes by the letters' net area (letter loops minus counters)
times the height: up for emboss, down for engrave. The property tests check this on random
boxes, faces and letter grids, so the faces really must be pushed by push/pull; don't build
the faces by hand.

## Where to look

- Skeleton and types: `show text_3d`, `show check_text`, `show Text3dError`, `show TextFaces`,
  `show TextMode`.
- Reused kernel code: `show push_pull_into`, `show PushPulled`, `show edge_pierces`,
  `show check_geometry`, `show SolidReport` (`is_solid`, `is_clean`), `show repair_geometry`,
  `show RepairReport`, `show Context::add_edge`, `show EdgeChain`, `show Model::edit_geometry`,
  `show Model::apply_edit`, `show ModelEdit`, `show UndoStack`.
- Geometry: `show segment_intersection`, `show SegmentHit`, `show point_in_polygon_2d`,
  `show Containment`, `show polygon_is_simple_2d`, `show to_plane_2d`, `show Plane`,
  `show LINEAR_TOL`; `Context::face`, `faces`, `edges`, `face_plane`, `loop_points`.
- Test helpers: `run` in `tests/text_3d.rs` (what every call must satisfy: valid model and one
  undo step on success; unchanged model, empty stack and the same answer from `check_text`
  on a refusal); `plate`, `rect`, `rev`, `profile_part`; `letters` and `frame` in
  `tests/text_3d_props.rs`; `offset_common` for `build`, `add_group`, `box_points`,
  `box_faces`, `prism`, `bounds`, `close`.
- Module style: `modeler-kernel/src/repair.rs`.
