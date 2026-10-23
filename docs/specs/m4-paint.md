# m4-paint: paint faces and instances; create, edit and purge materials (kernel)

Implement the functions in `app/crates/modeler-kernel/src/paint.rs` (skeleton on main).
Change only that file. Acceptance tests: `app/crates/modeler-kernel/tests/paint.rs` (uses the
shared helpers in `tests/transform_common/`).

These are the kernel operations behind the Paint Bucket tool and the Materials panel. Every
function that changes the model does it as **one undo step** pushed onto `undo` with
`label`, and pushes **nothing** when it changes nothing or fails. On any error the model is
exactly as before (roll back records already applied: apply the inverse of what this call
logged, newest first).

The model already has what is needed: `Model::{add_material, set_material, remove_material,
set_face_materials, set_instance}`, `Body::{face_materials, face_material_entries,
instance}`. A face has a front and a back material (`FaceMaterials`, `None` = default); an
instance has one material that paints the default-material faces inside it.

## `paint(model, body, targets, side, material, undo, label) -> Result<usize, PaintError>`

Checks, in this order:

1. `targets` empty: `PaintError::Empty`.
2. `body` doesn't exist: `PaintError::Model(ModelError::MissingBody(body))`.
3. `material` is `Some(id)` and the model has no material `id`:
   `PaintError::Model(ModelError::MissingMaterial(id))`.
4. An edge, face or instance in `targets` is not in the body: `PaintError::Missing(entity)`
   for the first such entity in `targets` order. (A face that was deleted is missing.)

Then:

- **Faces** (each distinct face, ascending id): set the `side` material (`front` for
  `Side::Front`, `back` for `Side::Back`) to `material`, keeping the other side. Skip a face
  whose side already has that material.
- **Instances** (each distinct instance, ascending id): set `Instance::material` to
  `material`, keeping every other field. `side` doesn't matter. Skip an instance that
  already has it.
- **Edges** are accepted (they must exist) but not painted.

Returns the number of faces and instances that changed. Zero: `Ok(0)` and no undo step.
`material = None` clears paint (back to default).

## `sample(model, body, entity, side) -> Option<Option<MaterialId>>`

The eyedropper. Face: `Some(material on that side)`. Instance: `Some(instance.material)`,
`side` ignored. Edge, missing entity or missing body: `None`. Doesn't change anything.

## `unique_material_name(model, base) -> String`

Trim `base` (leading/trailing whitespace); if that is empty use `Material`. If no material in
the model has exactly that name, return it; otherwise return `"<name> <n>"` for the
smallest `n >= 2` that no material has. Example: with `Brick` and `Brick 3` present,
`Brick` gives `Brick 2`.

## `create_material(model, material, undo, label) -> Result<MaterialId, PaintError>`

1. A color channel (all four: r, g, b, alpha) that is not finite or is outside `0..=1`:
   `PaintError::BadColor`.
2. Store `material` with its name replaced by `unique_material_name(model, &material.name)`
   (every other field as given, texture included) and push one undo step. Returns the new
   id.

## `edit_material(model, id, material, undo, label) -> Result<(), PaintError>`

1. No material `id`: `PaintError::Model(ModelError::MissingMaterial(id))`.
2. Bad color (as above): `PaintError::BadColor`.
3. The name is trimmed (blank becomes `Material`). If **another** material has that name, it
   becomes `"<name> <n>"`, smallest `n >= 2` that no other material has. The material's own
   current name never counts as taken.
4. If the result equals the stored material, return `Ok(())` without an undo step.
   Otherwise replace it (`Model::set_material`) and push one undo step.

## `purge_materials(model, undo, label) -> Result<Vec<MaterialId>, PaintError>`

A material is **used** when an instance in any body has it as `Instance::material`, or when
a face that exists has it on its front or back (face material entries of deleted faces don't
count). Every other material is **unused**.

- No unused material: `Ok(vec![])`, no undo step, nothing changes.
- Otherwise, in one undo step:
  1. The model refuses to remove a material that any face material entry names, including
     entries kept for deleted faces. So first, in every body (root, then definitions by
     ascending id), for each entry of a face that doesn't exist (ascending face id), set
     each side that names an unused material to `None` (`Model::set_face_materials`; other
     sides keep their value).
  2. Remove each unused material, ascending id.
  3. Return the removed ids, ascending.

Undoing the purge brings the materials and those entries back, so undoing further (e.g. the
erase that deleted the face) restores the face with its paint.

## Not in scope

- Which side of a face the user clicked, painting selections vs. the clicked face, fill
  modifiers (connected faces, all matching faces): the editor task does that.
- Texture size and placement on faces: `m4-texture-model`.
