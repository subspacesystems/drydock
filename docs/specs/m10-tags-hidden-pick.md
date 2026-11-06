# m10-tags-hidden-pick: hidden instances can't be picked, snapped to or box-selected

A group or component instance whose tag is hidden is not drawn (`m10-tags-hidden`). This
task makes the rest of the viewport agree: the cursor must not find what isn't shown. A
hidden instance is not picked, its points and edges are not snapped to, the eyedropper sees
through it, and a selection box ignores it. Whatever is behind it is found instead. Showing
the tag again brings everything back as it was.

Depends on `m10-tags-kernel` (`Model::assign_tag`, `Model::set_tag_visible`,
`Model::instance_visible`) and `m10-tags-hidden`.

## The one filter

`Model::visible_instances(body)` (`drydock-kernel/src/tags.rs`) yields the instances of a
body that are not hidden, in id order. Replace each instance walk named below with it. Don't
add checks anywhere else. An instance inside a hidden one is never reached because the walk
doesn't enter the hidden parent. A tag on a nested instance (an instance inside a
definition) hides it in every placement of that definition.

## `drydock-kernel/src/infer.rs`

- `gather_flat_body` and `gather_flat_rest`: walk `visible_instances(body)`.
- `flatten_in`, the rest-of-model loop over the siblings of each path step: walk
  `visible_instances(body)`. The path's own steps are still followed whatever their tag
  (`m10-tags-hidden-selection` closes a path through a hidden instance).
- `pick_model` and `infer_model`: the early-out that falls back to the root context when the
  root has no instances checks `visible_instances(BodyId::Root)`. So a model whose instances
  are all hidden picks and snaps exactly like its root geometry.
- `MaterialWalk::body` and `MaterialWalk::rest` (used by `material_in`, the eyedropper):
  walk `visible_instances(body)`. In `rest`, the open step is still followed.
- `pick_in` and `infer_in` go through `cached_flat_in` -> `flatten_in` and need nothing
  more.

The flat caches are keyed by `Model::cache_key` (model id and revision). Showing or hiding
a tag and assigning one are model records, and every record moves the revision. So the caches
need no change. Don't add a cache flush.

## `drydock-kernel/src/select.rs`

- `box_select`: the loop over the target body's instances walks `visible_instances(body)`.
  A hidden instance is never in the result.
- `gather_body` (the geometry that decides whether an instance is inside or touching the
  box): walk `visible_instances(body)`. Hidden geometry inside a visible instance neither
  keeps it out of a window box nor gets it into a crossing box. An instance with nothing
  visible inside it gathers no points and so is never selected (`selected` already returns
  false then).
- `gather_instance_segments` (the selection outline from `instance_segments`): walk
  `visible_instances`. The instance that `instance_segments` is asked about is not itself
  checked; only what is inside it is filtered.

## Out of scope

- Dropping hidden instances from the current selection, and the Editor's undoable show or
  hide: `m10-tags-hidden-selection`.
- `bounded`, `connected`, `ray_face`, `pick` and `infer` work on one context and contain no
  instances, so they need no change.
- Paint, copy, explode, solid checks and the clearance check keep seeing all instances.

## Tests (already written; you can't change them)

`drydock-kernel/tests/tags_hidden_pick.rs`: `pick_model` (hidden instance, an instance
behind a hidden one, nested, a hidden outer instance with a visible-tagged inner one, the
early-out), `infer_model` (endpoint, midpoint, on-edge, direction lock), `pick_in` and
`infer_in` (hidden in the open body, a hidden sibling and a hidden nested instance in the
rest), `material_in`, `box_select` (window, crossing, hidden contents, a definition body)
and `instance_segments`. Each one also checks that showing the tag again restores the
result. Run them with `harness/bin/run-tests -p drydock-kernel`. The other kernel tests
(`infer_model.rs`, `edit_path.rs`, `infer_material.rs`, `select.rs`, `infer_cache.rs`) must
stay green.

## Where to look

`show` these: `Model::visible_instances`, `Model::instance_visible`, `gather_flat_body`,
`gather_flat_rest`, `flatten_in`, `cached_flat`, `pick_model`, `infer_model`,
`MaterialWalk` (its `body` and `rest`), `box_select`, `gather_body`, `selected`,
`gather_instance_segments`, `instance_segments`, `Model::cache_key`, `Model::apply`.
Test helpers are in the test file itself: `simple`, `nested`, `nested_with`, `tag`, `hide`,
`show`, `top`.
