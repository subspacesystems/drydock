# m7-solid-repair: make a group printable in one step (kernel)

Implement `app/crates/modeler-kernel/src/repair.rs` (skeleton on main: `Unfixed`,
`RepairReport` with `changed()` already written, `RepairError`, `repair_geometry`,
`repair_body`). Change only that file. Acceptance tests:
`app/crates/modeler-kernel/tests/solid_repair.rs` and `tests/solid_repair_props.rs` (they use
the shared helpers in `tests/transform_common/`).

`m7-solid-check` (`solid.rs`, `docs/specs/m7-solid-check.md`) reports what keeps a context
from being a printable solid. This task fixes the simple cases: it removes walls inside the
solid, stray faces and loose edges, closes holes whose border is one flat loop, and turns
reversed faces round. What it can't fix it lists, so the UI (`m7-solid-ui`) can show it. The
Repair button calls `repair_body`, which is one undo step.

Terms are those of the solid-check spec: the **uses** of an edge are the faces whose loops
contain it (`Context::faces_of_edge`); an **open** edge has one use, a **shared** edge three
or more, a **loose** edge none.

## `repair_geometry(ctx, log)`

Runs these steps in order. Each step looks at the context as the previous steps left it.
Every change goes through the `Context` primitives (`delete_face`, `delete_edge`,
`delete_vertex`, `insert_face`) or through `Context::apply` with the record pushed onto
`log.records`, so `log` replays the repair on a copy of the input and `log.inverse()` undoes
it. Where an id list is walked, walk it in ascending order.

1. **Internal faces.** Delete every face in `check_geometry(ctx).internal_faces`. They go in
   `removed_internal`. (Their edges keep two or more uses: a wall's edges were shared.)
2. **Stray faces.** A face is *stray* when at least one of its edges (outer and inner loops)
   is open and none of them has exactly two uses: it isn't joined to any other face (a lone
   flat face, or a fin hanging off a box edge). If at least one face of the context is not
   stray, delete every stray face; they go in `removed_stray`. If every face is stray, delete
   none: there is nothing else to keep, and deleting them all would empty the part.
3. **Loose edges.** Delete every edge no face uses (loose before the repair, or left loose by
   steps 1 and 2); they go in `removed_edges`. Then delete every vertex no edge uses
   (`Context::validate` forbids stray vertices). Vertices aren't reported.
4. **Holes.** Take the open edges and group them into **borders**: two open edges are in the
   same border when they share a vertex (connected groups). Handle the borders in the order
   of their lowest edge id; each border's edges are listed in ascending order below.
   1. If some vertex of the border has other than two of the border's edges, it is
      `Unfixed::NotOneLoop(edges)`.
   2. Otherwise the border is one closed loop. Walk it into a loop of edge uses for the new
      face: start with the border's lowest edge `e0`, used in the direction opposite to its
      one existing use (so the new face runs against its neighbour there, and faces the same
      way as that neighbour), then follow the border's other edge at each vertex until back
      at the start. The loop's points are the start vertex of each use, in order.
   3. If the points are not planar (`geom::is_planar(points, geom::LINEAR_TOL)`, the test
      faces use), it is `Unfixed::NotFlat(edges)`.
   4. **Nested loops.** Compare every pair of loops that passed 1-3 (before any face is
      added, so the order doesn't matter): loops `A` and `B` are nested when every point of
      `B` lies within `LINEAR_TOL` of `geom::fit_plane(A's points)`, and some point of `B`,
      projected with `geom::to_plane_2d` along that plane's normal, is not `Outside` `A`'s
      projected polygon (`geom::point_in_polygon_2d` with `LINEAR_TOL`); check both orders.
      Both loops of a nested pair are `Unfixed::Nested(edges)`. (A ring-shaped hole has an
      outer and an inner loop in one plane; filling either alone would cover the inner
      opening. Two separate holes side by side in one plane are not nested and are filled.)
   5. Otherwise insert the face `Face { outer: that loop, inner: vec![] }`. If
      `ctx.face_violation(new)` is `Some`, take the face back out and report
      `Unfixed::BadFill(edges)`. If the new face overlaps any other face
      (`crate::faces::faces_overlap(ctx, new, other)`, the pairwise test of
      `Context::face_overlaps`; don't call `face_overlaps` itself, it compares every pair
      of faces and is too slow for imported meshes), take it back out and report
      `Unfixed::Covered(edges)`: the border was the edge of a flat sheet, not of a hole.
      Otherwise the face goes in `filled`.
      "Take it back out" means undo the records added since the insertion and truncate
      `log.records` to its length before it, so a repair that ends up changing nothing
      leaves `log` empty.
   Each unfixed border is pushed onto `unfixed` when its turn comes, so `unfixed` lists
   borders in order of their lowest edge id.
5. **Reversed faces.** For every face in `check_geometry(ctx).reversed_faces`, replace it by
   the same face turned round, under the **same id**: push `Record::RemoveFace(f, old)` and
   then `Record::AddFace(f, turned)` (apply each with `Context::apply`). The turned face has
   every loop reversed (uses in reverse order, each use's `reversed` flag negated), inner
   loops kept as inner loops. These faces go in `reversed`. A filled face can be among them
   (when its neighbour at `e0` was itself reversed).
6. **What's left.** `after = check_geometry(ctx)`. Push `Unfixed::SharedEdge(e)` for each
   edge in `after.shared_edges` (ascending), after the border entries.

Every id list in the report is in ascending order (steps that walk ascending lists give
that).

Non-orientable surfaces (a Moebius strip) can't be turned consistently; step 5 does what the
check says and `after` still shows reversed faces. No test covers it.

## `repair_body(model, body, undo, label)`

1. If the body doesn't exist: `Err(RepairError::Model(ModelError::MissingBody(body)))`,
   nothing changed.
2. Run `repair_geometry` on a **clone** of the body's geometry with a scratch `Edit`. If that
   log is empty, nothing would change: don't touch the model (not even
   `Model::edit_geometry`, which bumps the revision) and push no undo step.
3. Otherwise run the repair for real in one `ModelEdit`:
   `model.edit_geometry(body, &mut log, repair_geometry)` (the repair is deterministic, so
   it gives the same report). Then, for each face in `report.reversed` whose materials
   (`Body::face_materials`) are not `FaceMaterials::default()`, swap the two sides with
   `Model::set_face_materials`: `front` <-> `back` and `front_map` <-> `back_map`. The paint
   stays on the same physical side of the face, so the part looks the same after the
   repair. If any record fails, apply `log.inverse()` and return the error as
   `RepairError::Model`.
   Push `log` as one undo step named `label`.
4. If the body holds `n > 0` instances (`Body::instances()`), push
   `Unfixed::NestedInstances(n)` last and set `after.nested_instances = n` and
   `after.volume = None`, so `after` equals `check_body(model, body)` of the result. The
   instances themselves are left alone.

Useful signatures in `model.rs`:
`Model::body(&self, BodyId) -> Option<&Body>`, `Body::geometry(&self) -> &Context`,
`Body::instances()`, `Body::face_materials(&self, FaceId) -> FaceMaterials`,
`Model::edit_geometry<R>(&mut self, BodyId, &mut ModelEdit, impl FnOnce(&mut Context, &mut Edit) -> R) -> Result<R, ModelError>`,
`Model::set_face_materials(&mut self, BodyId, FaceId, FaceMaterials, &mut ModelEdit) -> Result<(), ModelError>`,
`Model::apply_edit(&mut self, &ModelEdit)`, `UndoStack::push(&mut self, &str, ModelEdit)`.
`erase.rs` is a short example of a command with rollback and one undo step.

## What the tests check

- A clean solid, an empty context, a lone sheet, a hole around a corner, a ring-shaped hole,
  boxes touching along an edge, and a body whose only problem is a nested instance: nothing
  changes (model equal, same revision, no undo step) and `unfixed` says why.
- Flipped faces, inside-out boxes and cavities facing the wrong way are turned round, keep
  their ids, and keep their paint on the same side; undo restores the paint.
- Walls between boxes, stray faces, fins, loose edges and their free vertices are removed.
- Missing faces, a window in a face (its border is the face's inner loop), and half of a
  split top are filled; the fill faces out; volumes are right.
- One undo step that restores the model exactly; redo gives the repaired model again; a
  second repair does nothing; `after` always equals a fresh `check_body`; the model always
  validates.
- `repair_geometry`'s log replays on a copy and inverts back to the input; it is empty when
  nothing changes.
- Properties: random boxes (size, rotation, position) with any faces flipped and at most one
  face missing repair to a clean solid of the right volume; clean boxes are left alone;
  repair is idempotent; the wall between two boxes is removed.

## Not in scope

The Repair button and highlighting (`m7-solid-ui`); holes whose border isn't flat or has
several loops (reported, not filled); merging coplanar faces after a wall is removed or a
hole is filled (they stay separate faces sharing an edge); faces that pierce each other
without an edge between them; repairing nested groups and components (reported).
