# m3-transform: transform a selection

Kernel operation under Move, Rotate, Scale and Flip. One function applies an affine matrix to a
selection of one body. The app builds the matrix (translation, rotation about a point, scale
about a point, mirror) and previews it itself.

Files: `app/crates/modeler-kernel/src/transform.rs` (skeleton on main; fill it in). Tests:
`tests/transform.rs`, `tests/transform_props.rs`, fixtures in `tests/transform_common/`.

## API

- `moved_vertices(ctx, selection)`: the vertices a transform moves. For each edge, both ends. For
  each face, every vertex of its outer and inner loops. Instances add none. Entities the context
  doesn't have are skipped.
- `transform(model, body, selection, matrix, undo, label)`: one undo step named `label`.

## Rules for `transform`

Checks run in this order; the first failure is returned and nothing changes (model and undo
stack equal their old values).

1. `MissingBody` if the body is unknown.
2. `Empty` if `selection` is empty.
3. `BadMatrix` if any entry is not finite, the bottom row is not exactly `(0, 0, 0, 1)`, or
   `|determinant| < 1e-9`.
4. `MissingEntity(e)` for the first selection entry (in order) the body doesn't have.
5. The identity matrix returns `Ok` and pushes no undo step.

Then:

- Every vertex in `moved_vertices` goes to `matrix.transform_point3(position)`, once, however
  often the selection names it. Unselected vertices stay. A vertex shared by a selected and an
  unselected edge or face moves; the unselected ones stretch.
- Each `Entity::Instance` gets `transform = matrix * old transform`.
- When the determinant is negative (a flip), every face whose loop vertices (outer and inner) are
  all moved is replaced by a face with the same edges and each loop reversed (use order
  reversed, each `reversed` flag toggled). Reversing keeps the front on the mirrored side of the
  surface, so a closed solid's normals still point outward. The new face gets the old face's
  materials unchanged (front stays front). Faces that are only partly moved keep their loops.
- After the edits, run `Context::validate()` on the body. If it reports a violation that the
  body did not already have (compared by value, so `Violation::NonPlanarFace(f)` before and
  after is the same), roll everything back and return `Invalid(first_new_violation)` in
  `validate`'s order. Violations that existed before never block a transform. Instances alone
  skip this check.
- Otherwise push the whole edit as one undo step. Undo restores the model exactly (equal by
  `==`); redo applies it again.
- `ModelError`s from the model come back as `Model(e)` with everything rolled back.

## Known limits (follow-up tasks, not this one)

- No sticky behaviour: a moved face that lands on other geometry is refused (`Invalid`), not
  merged or split. A later `m3-move-merge` makes moves heal and split against the rest.
- `validate` looks at the whole body, so this is for committing a move, not for every pointer
  event of a drag preview.
- Hole windings and loop contents are never changed except by the flip rule above.
