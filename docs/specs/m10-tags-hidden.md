# m10-tags-hidden: hidden tags leave instances out of drawing and export

An instance (group or component) whose tag is hidden is not shown. This task covers what is
**drawn** and what is **exported**. Picking, snapping and box selection are
`m10-tags-hidden-pick`; dropping hidden things from the selection is
`m10-tags-hidden-selection`. Hiding never deletes anything: showing the tag brings the
instance back unchanged.

Depends on `m10-tags-kernel` (merged): `Model::instance_visible`, `Model::assign_tag`,
`Model::set_tag_visible`.

## The one filter

`Model::visible_instances(body)` (already on main, `drydock-kernel/src/tags.rs`) yields the
instances of a body that are not hidden, in id order. An instance is hidden when it carries
a hidden tag. Its own tag decides; what an instance contains does not matter. An instance
inside a hidden one is never reached because the walk does not enter the hidden parent. A
tag on a nested instance affects only that instance and what is inside it.

Replace the instance walks named below with `visible_instances`; do not add per-item checks
further down. Do not touch the other walks (pick, infer, select: next task).

## Drawing (`drydock-dae/src/scene.rs`)

- `kernel_scene` and `kernel_scene_editing` (every `RestView`) draw no triangles and no
  lines for a hidden instance, at any depth. This includes the open part while editing: a
  hidden instance nested in the open instance is not drawn, and a hidden instance in the
  rest of the model is not drawn faded.
- `FlatScene::bounds` covers only what is drawn: a model whose instances are all hidden has
  `bounds == None` and zero triangles and lines.
- Toggling visibility changes `Model::revision` (the kernel already does), so nothing
  needs a separate cache flush; if a cache in the scene path is keyed by something else,
  key it by the revision.
- `drydock/src/document.rs`: the face and edge counts in the document stats (the walk over
  definitions near line 300) count only what is shown: do not count definitions reachable
  only through hidden instances. Keep it a walk from the root over `visible_instances`.

## Export (`drydock-mesh/src/print_mesh.rs`, `drydock/src/mesh_export.rs`)

- `print_mesh` (so STL and 3MF, one file or one per object) skips hidden instances in the
  root loop and in the nested walk. A skipped hidden instance is not an object and not in
  `skipped` (it is not an error). `PrintScope::Instances` naming a hidden instance exports
  nothing for it.
- `mesh_export::can_export` is false when the root has no visible instance.
  `mesh_export::export` of a model with only hidden instances returns the existing
  `Nothing to export` error and writes no file.
- There is no DAE export; nothing to do for it.

## Out of scope

The Tags panel, assigning tags, picking/snapping/selection, the clearance check
(`drydock-dae/src/clearance.rs`) and solid checks: they keep seeing all instances.

## Tests (already written; you cannot change them)

- `drydock-dae/tests/tags_hidden_scene.rs`: scene contents, bounds, nesting, editing views.
- `drydock-mesh/tests/tags_hidden_export.rs`: print mesh, scope, nesting.
- `drydock/tests/tags_hidden_export_ui.rs`: `export`, `can_export`.
- `drydock-render/tests/tags_hidden_render.rs`: headless render of a hidden box has only
  background pixels (`drydock-kernel` is added as a dev-dependency; already on main).

## Where to look

`show` these: `Model::visible_instances`, `Model::instance_visible`, `Draw` and `build_scene`
(the `instances` collection near scene.rs:266), `kernel_scene`, `kernel_scene_editing`,
`print_mesh`, `walk` (print_mesh.rs), `can_export`, `export` (mesh_export.rs), the stats
walk in document.rs (`Stats`, search `pending`). Test helpers: `print_common::{box_group,
place, definition}` in `drydock-mesh/tests/print_common/mod.rs`.
