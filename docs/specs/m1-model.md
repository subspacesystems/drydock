# m1-model: definitions, instances, materials and undo

Milestone M1, kernel task. Crate `app/crates/modeler-kernel`, file `src/model.rs`. Implement every `unimplemented!("m1-model")`. Don't change public signatures or public types; add private fields (to `Model`, `UndoStack`) and private helpers freely. `src/geom.rs` and `src/topology.rs` are done: use them, don't edit them. Dependencies: `glam` only. No panics on any input.

Design background: `docs/adr/004-kernel-design.md`. A `Model` has a root `Body` and any number of definitions, each with its own `Body`. A body holds one topology `Context` (its geometry), instances (placed copies of definitions, with a transform) and face material entries. Geometry in different bodies never touches. Every change is a `ModelRecord` with an inverse, so an `UndoStack` can undo and redo whole edits.

## Ids
- Materials, definitions and instances each have their own counter. The first id handed out is `0`, then `1`, ... Instance ids are unique across all bodies (one counter for the whole model).
- Ids are never reused: removing (or undoing) doesn't lower a counter. Applying an `Add*` record whose id is at or above the counter moves the counter to `id + 1`.
- `Model` equality (already written) ignores counters. `Clone` must copy them.

## Records
`ModelRecord::inverse`: `AddX` <-> `RemoveX` with the same data; `Set* { from, to }` swaps `from` and `to`; `Geometry(body, r)` -> `Geometry(body, r.inverse())`. `ModelEdit::inverse`: the inverse of each record, last record first.

`Model::apply(record)` checks in the order listed and changes nothing on error:
- `Geometry(body, r)`: body missing -> `MissingBody(body)`; the body's `Context::apply(r)` fails -> `Topo(e)`.
- `AddMaterial(id, m)`: id present -> `IdInUse`.
- `RemoveMaterial(id, m)`: missing -> `MissingMaterial(id)`; stored differs -> `Mismatch`; used by any instance's `material` or by the front or back of any face material entry, in any body -> `MaterialInUse(id)`. Entries for faces that no longer exist count.
- `SetMaterial { id, from, to }`: missing -> `MissingMaterial`; stored differs from `from` -> `Mismatch`.
- `AddDefinition(id, info)`: id present -> `IdInUse`. The new definition has an empty body.
- `RemoveDefinition(id, info)`: missing -> `MissingDefinition(id)`; differs -> `Mismatch`; it has instances anywhere -> `DefinitionInUse(id)`; its body is not empty (`Body::is_empty`) -> `DefinitionNotEmpty(id)`.
- `SetDefinition { id, from, to }`: missing -> `MissingDefinition`; differs -> `Mismatch`.
- `AddInstance(body, id, inst)`: body missing -> `MissingBody`; id present in any body -> `IdInUse`; `inst.definition` missing -> `MissingDefinition`; `inst.material` names a missing material -> `MissingMaterial`; placing it would make a cycle -> `Cycle`.
- `RemoveInstance(body, id, inst)`: body missing -> `MissingBody`; no such instance in that body -> `MissingInstance(body, id)`; differs -> `Mismatch`.
- `SetInstance { body, id, from, to }`: `MissingBody`; `MissingInstance`; stored differs from `from` -> `Mismatch`; then the `AddInstance` checks for `to` (`MissingDefinition`, `MissingMaterial`, `Cycle`).
- `SetFaceMaterials { body, face, from, to }`: `MissingBody`; the current value (`Body::face_materials`) differs from `from` -> `Mismatch`; `to.front`, then `to.back`, names a missing material -> `MissingMaterial`. The face doesn't have to exist. Setting the default value (both `None`) removes the entry; anything else stores it.
- Cycle: an instance of definition `D` placed in body `Definition(P)` is a cycle when `D == P` or `D`'s body contains, directly or through nested instances, an instance of `P`. Instances in the root never make a cycle.
- "Differs" is `!=` on the stored data (exact, transforms included).

`apply_edit`: applies records in order; on the first error, undoes the records already applied (in reverse) and returns that error. The model ends equal (`==`) to how it started.

## Operations
Each builds its record from the stored data, applies it through `apply` and pushes it onto `log` only on success; errors are those of the record. A failed call doesn't use up an id.
- `add_material(m)` -> new id; `set_material(id, m)`; `remove_material(id)`.
- `add_definition(info)` -> new id; `set_definition(id, info)`; `remove_definition(id)`.
- `add_instance(body, inst)` -> new id; `set_instance(body, id, inst)`; `remove_instance(body, id)`.
- `set_face_materials(body, face, m)`: `from` is the current value.
- `edit_geometry(body, log, f)`: `MissingBody` if the body is absent (and `f` isn't called). Otherwise call `f` with the body's `Context` and a new empty `Edit`, append each record of that `Edit` to `log` as `Geometry(body, record)` (in order), and return `f`'s result.

## Queries
- `instances_of(d)`: every `(body, instance)` whose instance has `definition == d`. Root first, then definition bodies by ascending id; ascending instance id within a body. Empty for an unknown id.
- `Body::face_materials(f)` is the stored entry or the default. Face material entries stay when the face is removed, so undoing the removal brings the paint back.

## validate
Returns, in this order:
1. For each body (root, then definitions by ascending id): its geometry's `validate()` violations, each as `Geometry(body, v)` in the order returned; then `BadTransform(body, id)` for each of its instances (ascending) whose transform has a non-finite entry or a determinant with absolute value `<= 1e-12`.
2. `GroupInstances(d, n)` for each definition with `group: true` (ascending) whose number of instances `n` (`instances_of(d).len()`) is not 1.

## UndoStack
- `push(label, edit)`: an empty edit is ignored (nothing changes). Otherwise it goes on top of the undo list and the redo list is cleared.
- `undo(model)`: `Ok(None)` when there is nothing to undo. Otherwise `model.apply_edit(&edit.inverse())`; on error, return it and leave the stack as it was; on success move the entry to the redo list and return its label.
- `redo(model)`: the same with `model.apply_edit(&edit)`, moving the entry back to the undo list.
- `undo_label` / `redo_label`: the label `undo` / `redo` would act on next, or `None`.

## Tests (fixed; don't edit)
`tests/model.rs` (each record and error, ids, cycles, face materials surviving removal, `edit_geometry`, validate, undo stack) and `tests/model_props.rs` (proptest: random operation sequences keep the model consistent; each operation's log undoes and redoes exactly; a failed operation changes nothing; the undo stack walks back to an empty model and forward again).
