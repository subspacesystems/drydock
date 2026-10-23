# m4-app-materials: create, edit and purge materials in the editor

Three `Editor` methods (`app/crates/modeler/src/editor.rs`, on main as `unimplemented!()`)
wrap the kernel calls `create_material`, `edit_material` and `purge_materials` in
`modeler_kernel::paint`; read `docs/specs/m4-paint.md`. Only `editor.rs` changes. Tests:
`app/crates/modeler/tests/editor_materials.rs`. The panel that calls them is a later task.

None of them touch the open body, the selection or any tool in progress.

## `create_material(model, material) -> Option<MaterialId>`

- Kernel `create_material` with label `New Material`.
- Success: the new id becomes the Paint Bucket's current material (`material()`), the
  message is cleared, the id is returned.
- Kernel error: message `Can't make that material`, nothing changes (current material
  too), `None`.

## `edit_material(model, id, material) -> bool`

- Kernel `edit_material` with label `Edit Material`. Success (including "nothing changed",
  which pushes no undo step): message cleared, true.
- Kernel error: message `Can't change that material`, nothing changes, false.

## `purge_materials(model) -> usize`

- Kernel `purge_materials` with label `Purge Materials`; returns how many went.
- If the current material was removed, it becomes `None` (the default).
- Nothing to remove: message `No unused materials`, returns 0, no undo step. Otherwise the
  message is cleared.
- Kernel error: message `Can't purge materials`, returns 0.
