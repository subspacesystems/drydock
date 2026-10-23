# m4-edit-view-scene: drawing the model while editing inside an instance

While a user edits inside an instance, the rest of the model should be drawn faded, or not
at all. This task adds the scene function for that and fixes the status counts. Wiring it
into the window is a later task (`m4-app-edit-view`).

Files: `app/crates/modeler-dae/src/scene.rs` (skeleton on main: `RestView`,
`kernel_scene_editing`, both exported from the crate) and `app/crates/modeler/src/document.rs`
(`stats_from_scene`). Tests: `app/crates/modeler-dae/tests/kernel_edit_view.rs`,
`app/crates/modeler/tests/document_stats.rs`. Every other test must keep passing;
`kernel_scene` itself must not change.

## `kernel_scene_editing(model, path, rest) -> FlatScene`

`path` is an `EditPath` (`modeler_kernel::editpath`). The **open part** is the open body's
own geometry and everything nested below it through instances, placed in the world by the
path's transform (an instance's own placement is part of the path transform, so a nested
instance inside the open body is placed relative to it). The **rest** is every other face
and edge the normal scene would draw: the root's own geometry, the other bodies along the
path, and other instances of the same definitions (instance `b` of the same definition as
the open instance `a` is rest).

- Empty path, or a path that does not resolve in `model` (`path.body(model)` is `None`):
  the result equals `kernel_scene(model)` for every `RestView`.
- `RestView::Normal`: the result equals `kernel_scene(model)`.
- `RestView::Hidden`: only the open part is drawn, faces and edges, with the model's own
  materials (the `materials` list is the same as `kernel_scene`'s). Bounds cover what is
  drawn.
- `RestView::Faded`: the open part is drawn exactly as in `Normal`. Rest **faces** are
  drawn too; rest **edges** are not. Rest faces use faded materials:
  - `materials` starts with the model's materials, unchanged and in the same order. After
    them come faded copies, one per model material that a rest face ends up using (including
    through an instance's own material), in the model's material order, then the faded
    default if a rest face has no material.
  - Faded copy of material `M`: name `"<M name> (faded)"`, color rgb as `M`, alpha 1.0,
    opacity `M.opacity * 0.25`, same texture (texture coordinates are computed as usual), `unlit`
    as `M`, and a new unique `id`.
  - Faded default: name `"default (faded)"`, color `[0.75, 0.75, 0.75, 1.0]`, opacity
    0.25, no texture.
  - All rest faces that use one material share one faded copy; batches are in material
    index order, as in `kernel_scene`.
  - No faded material is added when no rest face uses it (a model with no rest geometry
    gives the same scene as `kernel_scene`).
  - Bounds cover everything drawn (open and rest).

Implement it by sharing the walk in `kernel_scene` (add a flag for "inside the open part" and
a way to pick faded materials), not by copying the module. Do not slow down `kernel_scene`.

## Status counts

`document::stats_from_scene` counts faces and edges of the root and of every definition.
Count a definition only when it is reachable from the root through instances (an instance in
the root, or in a reachable definition). A definition counts once however many instances it
has. After Explode of the only instance, the emptied or unused definition no longer counts.
