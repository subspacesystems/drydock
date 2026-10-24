# m4-copy: copy a selection, clipboard snapshot and paste (kernel)

Implement `Clipboard::{is_empty, bounds}`, `extract`, `insert` and `copy_selection` in
`app/crates/modeler-kernel/src/copy.rs` (skeleton on main). Acceptance tests:
`app/crates/modeler-kernel/tests/copy.rs` and `tests/copy_props.rs`.

Read first: `docs/specs/m4-make-component.md` (definitions, instances, groups),
`docs/specs/m4-explode.md` (how copied geometry is placed and sticks), `docs/specs/m4-paint.md`
(materials), `docs/specs/m4-texture-follow.md` (texture maps follow moved faces).

You may change `src/explode.rs` (to share its placing code), and add `pub(crate)` helpers in
`src/model.rs` (for example to put an exact clone of a definition's body into another model).
All existing tests must keep passing; explode's behaviour must not change. Instance ids are
unique across a whole model: a helper that puts a cloned body into the clipboard's model must
move that model's next instance id past the clone's instance ids, or the next `add_instance`
there fails with `IdInUse`.

The app will use this for Move with copy (copy the selection to where the mouse took it), and
for Cut / Copy / Paste (snapshot now, put it somewhere later, maybe in another file).

## Terms

- The **selection's parts** in a body, for a selection: faces `F` = the selected faces;
  edges `E` = the selected edges plus every edge used by a loop (outer or inner) of a face in
  `F`; vertices `V` = the ends of the edges in `E`; instances `I` = the selected instances.
  Duplicates in a selection count once.
- A **bad matrix**: any non-finite entry, a bottom row other than `(0, 0, 0, 1)`, or
  `|det| < 1e-9` (the same test `transform::transform` uses).

## `Clipboard`

A snapshot held in a `Model` of its own (`Clipboard::model()`, already written):

- Its **root** holds a copy of the selection's parts, positions unchanged (the source body's
  coordinates), with the same loops, and each copied face with the same `FaceMaterials` as
  its source (maps included). Copied instances have the same definition, transform, name and
  material. Ids in the clipboard's root are its own (they need not match the source's).
  Nothing else is in the root: no other geometry, no guides.
- Its **definitions** are every definition the copied instances reach (their definitions, the
  definitions of instances inside those, and so on), each an exact copy of the source's:
  same `DefinitionId`, `DefinitionInfo` equal, body equal (`==`).
- Its **materials** are every material the snapshot uses, each with the same `MaterialId`
  and an equal `Material`. No other definitions or materials. A material is **used by a
  body** when a face material entry of that body (`Body::face_material_entries`) has it on
  either side, or an instance in that body has it as its material. The snapshot uses the
  materials on the copied faces' sides, the copied instances' materials, and the materials
  used by each copied definition's body.

`is_empty()`: the root has no edge, no face and no instance.

`bounds()`: the box `tools::scale::bounds` gives for the clipboard model's root with every
edge, face and instance of the root selected, as `(min, max)`; `None` when that is `None`.

## `extract(model, body, selection) -> Result<Clipboard, CopyError>`

Checks in this order: `selection` empty → `CopyError::Empty`; `body` missing →
`CopyError::Model(ModelError::MissingBody(body))`; an entity not in `body` →
`CopyError::Missing(entity)` (first in selection order). Never changes `model`.

## `insert(model, body, clipboard, matrix, undo, label) -> Result<Vec<Entity>, CopyError>`

### Checks, in this order (nothing changes on error)

1. `body` missing: `CopyError::Model(ModelError::MissingBody(body))`.
2. `clipboard.is_empty()`: `CopyError::Empty`.
3. `matrix` is bad: `CopyError::BadMatrix`.

### What happens (one `ModelEdit`, pushed once onto `undo` with `label`)

1. **Materials.** Each clipboard material `m` (id ascending) maps to a target material:
   - the target's material with the same id, if it exists and is equal (`==`);
   - else the lowest-id target material equal to it;
   - else a new target material equal to it except its name, which is
     `paint::unique_material_name(model, &name)` (so `Brick` becomes `Brick 2` when a
     different `Brick` is there).
2. **Definitions.** Each clipboard definition `d` maps to a target definition. It is
   **reused** (maps to itself) when all of these hold:
   - it is not a group;
   - the target has a definition with id `d`, its `DefinitionInfo` equal and its body equal
     (`==`) to the clipboard's;
   - every material used by the clipboard's body of `d`, and every definition its instances
     place, maps to itself.

   Otherwise it maps to a **new** definition: name
   `component::unique_definition_name(model, &name, group)`, same `group`, and a body built
   from the clipboard's copy: same geometry (new ids are fine), face materials and instances
   with material and definition ids mapped. Work bottom-up so a definition's nested
   definitions are mapped first. A group is never reused, so every pasted group gets a
   definition of its own (`Model::validate` allows one instance per group).

   So pasting into the file it was copied from, with the component unchanged, places another
   instance of the same definition (editing one edits all); after the definition was edited,
   or in another file, the paste brings its own copy.
3. **Place.** Put the clipboard root's contents into `body` exactly as explode places an
   instance's contents (`docs/specs/m4-explode.md` steps 1 to 7) with `M = matrix` and no
   instance material: copies at `matrix · p`, loops reversed when `det < 0`, face materials
   mapped by step 1 with texture maps carried as `texture::moved_maps` does, instances with
   transform `matrix * transform` (same name, mapped definition and material), the copy sticks
   to the geometry of `body`, no new faces besides those inside an old or copied face, and the
   orientation and side-of-space materials rules.
4. **Validate.** A `Model::validate` violation that wasn't there before: roll everything
   back, `CopyError::Invalid(new violations)`. A `ModelError` or `TopoError` on the way: roll
   back, `CopyError::Model(..)` (a `TopoError` as `ModelError::Topo`). A paste that would put
   an instance inside its own definition (directly or deeper) fails this way with
   `CopyError::Model(ModelError::Cycle)` and changes nothing.
5. Push the edit. Undo restores the model exactly (`==`); redo brings the result back.

### Return value

The entities of `body` that came from the paste, sorted ascending, no duplicates, by
explode's rule: edges with both ends on one copied edge's segment, faces whose interior
point lies inside a copied face, and every instance added. The app selects these.

## `copy_selection(model, body, selection, matrix, undo, label) -> Result<Vec<Entity>, CopyError>`

`extract(model, body, selection)` then `insert(model, body, &clipboard, matrix, undo,
label)`; errors from either are returned as they are (so an empty selection is `Empty`, a
bad matrix `BadMatrix`). One undo step. The originals stay as they were (except where the copy
sticks to them: a copy that touches them shares edges and splits faces like any geometry
drawn there).

## Examples

- A unit cube's 6 faces in the root, `copy_selection` with a translation by `(3, 0, 0)`:
  the root has 16 vertices, 24 edges, 12 faces, normals out; the result lists 12 edges and
  6 faces at `x` 3..4.
- Same, translated by `(1, 0, 0)` (touching): the shared square at `x = 1` is one face
  of the root used by both cubes' edges; 12 vertices, 20 edges, 11 faces.
- A single loose edge copied up by 1 m: two separate edges.
- A component instance copied: the root has two instances of the same definition; the
  definition count is unchanged.
- A group instance copied: two groups, each with its own definition (`Group` and `Group 2`
  when the original was named `Group`), both bodies equal in geometry.
- Copy in file A, paste into a new empty `Model`: the materials and the component definitions
  arrive with their names, the new model validates.
- A rectangle face copied with a mirror matrix (`scale(-1, 1, 1)`): the copy's normal points
  the way the mirrored original showed it (the same direction as the source normal, for a
  horizontal face).
