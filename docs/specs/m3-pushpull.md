# m3-pushpull: push/pull a face

Kernel side of the Push/Pull tool. Implement `app/crates/modeler-kernel/src/pushpull.rs`
(skeleton on main; replace the `unimplemented!`). Only that file changes. Tests:
`tests/pushpull.rs`, `tests/pushpull_props.rs`, fixtures in `tests/pushpull_common/`.
Lengths are meters; `LINEAR_TOL` is `crate::geom::LINEAR_TOL`.

## Checks

In this order; the first failure is returned and nothing changes (model and undo stack equal
their old values):

1. `MissingBody` if the body is unknown.
2. `MissingFace` if the body has no such face.
3. `BadDistance` if `distance` is not finite or `|distance| <= LINEAR_TOL`.

## Terms

- `n`: the face's front normal, `Context::face_plane(face).normal`. `t = distance * n`.
- The face's uses: every use of its outer loop, then of each inner loop, in order.
- **Isolated**: no edge of the face is used by any other face.
- **Stretch edge**: an edge of the face used by exactly one other face `G`, where `G`'s plane
  normal `m` has `|m . n| <= 1e-6`, and every outer-loop vertex `p` of `G` has
  `n . (p - q) <= LINEAR_TOL` for a point `q` of the face's plane (`G` is on the back side).

## Slide (every edge of the face is a stretch edge)

Move every loop vertex of the face by `t` (`set_vertex_position`). Nothing is added or
removed; the neighbours stretch or shrink. Result: `cap` = the face (same id), `sides` empty,
`base` `None`. Example: the top of a box goes up or down and the walls follow.

## Extrude (otherwise)

Working loops: the face's loops, or, when the face is isolated and `distance < 0`, the face's
loops reversed (use order reversed, each `reversed` flag toggled). With these loops the result
is a closed solid whose fronts face out.

1. For each loop vertex `x`, a new vertex `x'` at `x + t`, and an edge `x`-`x'`.
2. For each working use from `a` to `b`, an edge `a'`-`b'`.
3. The **cap**: a new face with the working loops, each use of edge `a`-`b` replaced by a use of
   `a'`-`b'` in the same direction. It gets the face's materials; front and back swap when the
   working loops are reversed.
4. **Sides**: for each working use from `a` to `b`, in order, a new face with the outer loop
   `a -> b -> b' -> a'` (the old edge in the working direction, then the edges just made). No
   materials. These front away from the face's interior when pushing out (`distance > 0` or the
   face is isolated) and toward it when pushing in, so a pocket's walls face into the pocket.
5. The pushed face is deleted. If it was isolated, a **base** face is added at the old position
   with the working loops reversed (it fronts away from the cap), no materials. Otherwise the old
   edges stay as the bottom of the new walls and there is no base.

Stretch edges are not treated specially when the face also has other edges (no merging of a new
side with a coplanar neighbour yet).

## Refusals after the edit

- Run `validate()` on the body before and after. If it reports a violation that wasn't there
  before (by value), roll back and return `Invalid(first new violation)`.
- Else, for a slide only: if a face using a moved vertex now has a front normal `m2` with
  `m2 . m1 < 0` against its normal `m1` before, roll back and return `Inverted(lowest such id)`
  (pushing a box top below its bottom; `validate` alone doesn't see this).
- Else run `face_overlaps()` before and after. If a pair is new, roll back and return
  `Overlap(first new pair)`.
- `ModelError`s come back as `Model(e)`, rolled back.

Otherwise push the whole change as one undo step named `label`. Undo restores the model exactly
(`==`); redo applies it again and gives the same geometry.

## Known limits (later tasks)

- No merging of new sides with coplanar neighbours, no hole punching (pushing a face onto the
  opposite face of a solid), no "new start face" modifier: those results are refused or left
  with an extra edge.
