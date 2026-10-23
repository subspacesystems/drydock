# m4-make-component: make component and make group (kernel)

Implement `unique_definition_name` and `make_component` in
`app/crates/modeler-kernel/src/component.rs` (skeleton on main). Change only that file.
Acceptance tests: `app/crates/modeler-kernel/tests/component.rs` and
`tests/component_props.rs` (shared helpers in `tests/transform_common/`).

A component is a definition (its own body: geometry, instances, face materials) placed by
instances. Geometry inside a definition doesn't stick to the geometry around its instances.
A group is a definition flagged `group: true` that has exactly one instance
(`Model::validate` checks that). "Make component" and "make group" both take a selection
out of a body, put it in a new definition, and leave one instance in its place, so the
model looks the same afterwards.

## `unique_definition_name(model, base, group) -> String`

Trim `base`. If that is empty, use `Group` when `group` is true, else `Component`. If no
definition has exactly that name, return it; otherwise `"<name> <n>"` for the smallest
`n >= 2` that no definition has.

## `make_component(model, body, selection, info, undo, label) -> Result<InstanceId, ComponentError>`

### Checks, in this order (nothing changes on error)

1. `selection` is empty: `ComponentError::Empty`.
2. `body` doesn't exist: `ComponentError::Model(ModelError::MissingBody(body))`.
3. An edge, face or instance in `selection` is not in the body:
   `ComponentError::Missing(entity)`, first such entity in selection order.

Duplicates in `selection` count once.

### What goes into the definition

- **Faces** `F`: the selected faces.
- **Edges** `E`: the selected edges plus every edge used by a loop (outer or inner) of a face
  in `F`.
- **Vertices** `V`: the ends of the edges in `E`.
- **Instances** `I`: the selected instances.

**Origin** `o`: the `min` corner of `tools::scale::bounds(model, body, selection)` (the
selection's axis-aligned box in body coordinates; it already covers instances). If that is
`None`, `o` is the zero vector. `T` is the translation by `o`.

### Steps (all in one `ModelEdit`, pushed once onto `undo` with `label`)

1. Add the definition: `info` with `name` replaced by
   `unique_definition_name(model, &info.name, info.group)`; `group` as given.
2. In the definition's body (`Model::edit_geometry` with the raw `Context` primitives
   `insert_vertex`, `insert_edge`, `insert_face`, not the sticky `add_edge`, so no extra faces
   form and nothing merges):
   - one vertex per vertex of `V` (ascending id) at `position - o`;
   - one edge per edge of `E` (ascending id) joining the copies of its two ends, in the same
     order (`ends[0]`, `ends[1]`);
   - one face per face of `F` (ascending id) with the same loops (outer and inner, same
     order, same `reversed` flags) on the copied edges.
3. Copy face materials: for each face of `F` whose `FaceMaterials` in `body` are not the
   default, set the same front and back on its copy (`Model::set_face_materials`).
4. For each instance of `I` (ascending id): add a copy to the definition's body with
   `transform = T⁻¹ * transform` (so it stays in place in world space) and the same
   `definition`, `name` and `material`; then remove the original from `body`.
5. In `body`:
   - delete every face of `F` (`delete_face`);
   - then delete every edge of `E` (ascending) that no face uses any more
     (`Context::faces_of_edge` empty), with `delete_edge`. Edges still used by unselected
     faces stay, so the rest of the body stays closed and valid (selecting the top face of
     a cube leaves the cube's 12 edges and 5 faces);
   - then delete every vertex of `V` (ascending) that no edge uses any more
     (`edges_of_vertex` empty), with `delete_vertex`.
   Face material entries of the deleted faces stay in `body` (the model keeps them for undo).
6. Add one instance to `body`: `definition` = the new one, `transform = T`, empty `name`,
   `material: None`.
7. If `model.validate()` now has a violation it didn't have before the call, roll back
   (apply the inverse of everything logged, newest first) and return
   `ComponentError::Invalid(new violations)`. Same rollback for any `ModelError` /
   `TopoError` on the way (`ComponentError::Model`, a `TopoError` wrapped as
   `ModelError::Topo`).
8. Push the edit and return the new instance's id.

Undo removes the instance and the definition and restores the selection exactly (the model
compares equal to before). Redo brings back the same ids.

### Examples

- Whole unit cube at `(2, 3, 0)` selected (faces and/or edges): the body is left with no
  vertices, edges or faces and one instance at translation `(2, 3, 0)`; the definition
  holds 8 vertices, 12 edges and 6 faces between `(0, 0, 0)` and `(1, 1, 1)`.
- Top face of a cube only: definition 4 / 4 / 1; body keeps 8 / 12 / 5.
- Four loose edges forming a square (no face): definition 4 / 4 / 0 (no face is made).
- One of two squares sharing an edge: definition 4 / 4 / 1; body keeps the other square
  4 / 4 / 1 (the shared edge stays, the selected square's two other vertices go).
- A face with a hole: the copy keeps the hole (one inner loop, 8 edges); the face filling
  the hole, if not selected, stays in the body with its 4 edges.

## Not in scope

- Exploding an instance (`m4-explode`), editing inside an instance (`m4-edit-path`), and
  the editor/menu wiring (`m4-app-components`).
- Choosing other axes than the box corner, gluing to faces, cutting openings.
