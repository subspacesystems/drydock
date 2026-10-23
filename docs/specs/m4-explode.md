# m4-explode: explode an instance (kernel)

Implement `explode` in `app/crates/modeler-kernel/src/explode.rs` (skeleton on main). You
may add private helpers there, and make small additive changes in `src/movemerge.rs`
(`restick` and `interior_point` are already `pub(crate)` for you). Tests:
`app/crates/modeler-kernel/tests/explode.rs` and `tests/explode_props.rs`.

Read first: `docs/specs/m4-make-component.md` (what definitions, instances and groups are),
`docs/specs/m3-move-merge.md` (how moved geometry sticks to the rest), `docs/specs/m4-paint.md`
(face materials).

Explode is the opposite of make component: an instance's contents go into the body that
holds the instance, placed where the instance showed them, and stick to the geometry there
like geometry drawn in that body. The model looks the same afterwards.

## `explode(model, body, instance, undo, label) -> Result<Vec<Entity>, ExplodeError>`

### Checks, in this order (nothing changes on error)

1. `body` doesn't exist: `ExplodeError::Model(ModelError::MissingBody(body))`.
2. `body` has no instance `instance`: `ExplodeError::MissingInstance(instance)`.
3. The instance's transform `M` has a non-finite entry or `|det M| < 1e-12`:
   `ExplodeError::BadTransform`.

### What happens (one `ModelEdit`, pushed once onto `undo` with `label`)

Let `D` be the instance's definition body and `mat` the instance's material.

1. **Copy.** Every vertex, edge and face of `D` is copied into `body` with positions
   `M · p`. When `det M < 0` (a mirrored instance) each copied face's loops are reversed
   (as `transform` does for a flip), so faces keep pointing the way the instance showed them.
2. **Copy materials.** A copied face gets the face materials of its source in `D`, with
   each side that has the default material (`None`) taking `mat` instead (so a painted
   instance's unpainted faces keep its color).
3. **Nested instances.** Every instance inside `D` gets a copy in `body` with transform
   `M * its transform`, the same definition and name, and material: its own, or `mat` when
   its own is `None`.
4. **Remove** the exploded instance from `body`.
5. **Stick.** The copied geometry sticks to the geometry of `body` with the move-merge rules
   (`geom::LINEAR_TOL`): coinciding vertices weld, an end on an edge splits it, crossing edges
   split each other, overlapping edges become shared pieces, and loops in a face become
   holes with their own faces. `movemerge::restick` with the copied vertices as the moved set
   does this.
6. **No new faces.** Afterwards every face of `body` lies inside (its interior point is
   inside) a face of `body` from before the call or a copied face. Any other face that
   sticking made (a loop closed by old and new edges, the open top of a box) is deleted;
   those edges stay. Only faces go; never delete edges or vertices in this step.
7. **Orientation and materials.** Each face of `body` that lies inside a copied face points
   the way that copied face pointed (after step 1); otherwise, inside an old face of `body`,
   the way that one pointed. Reverse the face (delete it and insert it with reversed loops)
   where it doesn't. Its materials are those of the copied face it lies in, unless that
   copy's materials are both default and an old face covers it too, in which case the old
   face's. Materials stay on the side of space they were on: when the face they come from
   pointed the opposite way, front and back swap.
8. **Groups.** If the definition is a group (`DefinitionInfo::group`), it now has no
   instance: empty its body (its instances, faces, edges, vertices, face material entries
   set back to default, guides) and remove the definition. A component definition stays,
   even with no instances left (purging unused definitions is separate).
9. **Validate.** If `model.validate()` now has a violation it didn't have before the call,
   roll everything back (apply the inverse of what was logged) and return
   `ExplodeError::Invalid(new violations)`. Any `ModelError` or `TopoError` on the way: roll
   back and return `ExplodeError::Model(..)` (a `TopoError` wrapped as `ModelError::Topo`).
10. Push the edit; undo restores the model exactly (`==`), redo brings back the result.

### Return value

The entities of `body` that came from the explode, sorted ascending, no duplicates:

- every edge of `body` whose two ends both lie on (within `LINEAR_TOL` of) one copied edge's
  segment (so both halves of a copied edge that got split count);
- every face of `body` whose interior point lies inside a copied face;
- every instance added in step 3.

The app will select these.

### Examples

- Unit cube definition, instance translated by `(5, 0, 0)` in an empty root: the root gets
  8 vertices, 12 edges, 6 faces at `x` 5..6, normals out; the result lists 12 edges and
  6 faces; the definition is still there and unchanged.
- The same cube touching a root cube face to face: 12 vertices, 20 edges, 11 faces (the two
  faces at the shared side are one face).
- A definition holding one edge `(0,0,0)-(2,0,0)` exploded over a root edge
  `(1,-1,0)-(1,1,0)`: 5 vertices, 4 edges; the result is the two halves of the copied edge.
- An open box (no top) whose corner lands on a loose root edge stays open: 5 faces.
- A cube standing on a painted ground face: the ground gets a square hole, the footprint
  gets its own face, 7 faces in all; the ground keeps its paint.

## Not in scope

- Editor and menu wiring (`m4-app-components`), selecting the result.
- Purging unused component definitions.
- Guides inside the definition (not copied).
