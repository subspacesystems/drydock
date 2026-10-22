# m3-move-merge: a move that sticks

Kernel operation under Move (and the other transforms when the app wants them sticky).
`transform_sticky` does what `transform` does, then makes the moved geometry join the rest of
the body: coinciding points weld, touching and crossing edges split, and faces re-form.

Files: `app/crates/modeler-kernel/src/movemerge.rs` (skeleton on main; fill it in; you may add
private helpers and may make small additive changes elsewhere only if unavoidable). Tests:
`tests/move_merge.rs` (fixtures from `tests/transform_common/`).

Read first: `docs/specs/m3-transform.md`, `docs/specs/m1-sticky-edges.md`,
`docs/specs/m1-sticky-faces.md`, `docs/adr/004-kernel-design.md`.

## API

`transform_sticky(model, body, selection, matrix, undo, label) -> Result<(), TransformError>`,
same arguments and error type as `transform::transform`.

## Rules

1. The checks and their order are those of `transform` (`MissingBody`, `Empty`, `BadMatrix`,
   `MissingEntity`, identity is `Ok` with no undo step). Moved vertices, stretching of unselected
   neighbours, instance transforms and the flip rule are also exactly `transform`'s.
2. After the move, the moved geometry is stuck to the rest, with tolerance `geom::LINEAR_TOL`:
   - **Weld.** A moved vertex that lands on an unmoved vertex becomes that vertex. Edges left with
     the same two ends merge into one edge, and faces keep correct loops over it.
   - **T-split.** A moved vertex that lands on an unmoved edge splits that edge there; an
     unmoved vertex that lands on a moved edge splits the moved edge there.
   - **Crossing.** A moved edge that crosses an unmoved edge (same plane) splits both at the
     crossing, sharing one new vertex.
   - **Overlap.** A moved edge laid along an unmoved one is not duplicated: the pieces they share
     are one edge, and each is split at the other's vertices.
   - **Faces.** Wherever the stuck edges close a loop in a plane, the face rules of
     `m1-sticky-faces` hold: a loop dropped inside a face gives that face a hole and gets its own
     face; a face moved across another face splits both along the shared lines, each region
     having exactly one face; faces sharing an edge share the one edge.
   - Moved faces keep their materials on the face that covers their region; regions without a
     moved or old face get none.
3. Geometry that touches nothing new ends up exactly as `transform` leaves it (the models are
   equal by `==`, ids included).
4. Validation is `transform`'s: when the result has a violation (`Context::validate()`) the body
   did not already have, roll everything back and return `Invalid(first_new_violation)`.
5. One undo step named `label`; undo restores the model exactly, redo reapplies. Any other
   failure rolls back and returns `Model(..)`.

## Suggested approach (not binding)

Do the plain move without validating, then re-stick: delete the moved edges (and the faces over
them) from the context and re-add each at its new place with `Context::add_edge`, which already
splits, welds and builds faces, then restore materials by matching face edge sets. Moved
vertices that coincide with unmoved ones can be welded by re-adding the edges of both.
Whatever you do, every change must go through the `topology` primitives so `ModelEdit` can
invert it.

## Known limits (follow-ups)

- Hole punching and merging by Push/Pull is `m3-pushpull-merge`.
- Instances (groups, components) never stick.
- Cost: this may scan the whole body per move; it runs on commit, not on every pointer event.
