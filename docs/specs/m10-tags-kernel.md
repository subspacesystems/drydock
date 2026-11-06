# m10-tags-kernel: tags in the model and the native file (v7)

Milestone M10. Kernel only: no rendering, no app, no UI (those are `m10-tags-hidden`,
`m10-tags-panel`, `m10-tags-assign`).

## What a tag is
A tag is a named label with a visible flag. Groups and component instances (`Instance`, in
any body) can carry at most one tag; loose geometry can't. An instance whose tag is hidden is
not shown: later tasks leave it out of drawing, picking and export. Here we only store tags,
change them through undoable records, and save them.

The skeleton is on main: `drydock-kernel/src/tags.rs` (types, `TagRecord::inverse`, the
read-only `Tags` queries, and the `Model` methods to fill in) and the plumbing in
`model.rs` (already done, don't redo it):
- `Model` has a `tags: Tags` field; `Model::tags()` reads it, `Model::tags_mut()` (crate
  only) writes it. Model equality includes the tags (`Tags` equality ignores the id counter).
- `ModelRecord::Tag(TagRecord)`; its inverse is `TagRecord::inverse`. `Model::apply` sends it
  to `Model::apply_tag_record` (yours) and moves the revision like every non-settings record.
- New `ModelError` variants: `MissingTag(TagId)`, `BadTagName`, `TagInUse(TagId)`,
  `InstanceTagged(InstanceId)`, `UnknownInstance(InstanceId)`.
- A raw `RemoveInstance` record for an instance that carries a tag is refused with
  `InstanceTagged`. `Model::remove_instance` first calls `assign_tag(id, None)` when the
  instance is tagged, so the log is `SetInstanceTag(Some -> None)`, then `RemoveInstance`,
  and undo gives the tag back. Explode, erase and component replacement all go through
  `remove_instance`, so no assignment is ever left pointing at a missing instance.
- `Model::instance_location` is now `pub(crate)`: it finds the body holding an instance
  (instance ids are unique across the whole model).

You fill in `tags.rs` (the `unimplemented!` bodies; drop the two `#[allow(dead_code)]` lines,
on `Tags::next` and on `Model::tags_mut`, once they are used) and `format.rs`.
`docs/format.md` already describes version 7.

## Records: `Model::apply_tag_record`
On any error nothing changes. Checks in this order:
- `AddTag(id, tag)`: `BadTagName` when `!tag_name_is_good(&tag.name)`; `IdInUse` when the id
  exists. Inserts it and moves `Tags::next` to at least `id + 1` (never down).
- `RemoveTag(id, tag)`: `MissingTag(id)`; `Mismatch` when the stored tag differs from `tag`;
  `TagInUse(id)` when any instance carries it. Removes it. The counter stays.
- `SetTag { id, from, to }`: `BadTagName` for `to.name`; `MissingTag(id)`; `Mismatch` when
  the stored tag differs from `from`. Stores `to`.
- `SetInstanceTag { instance, from, to }`: `UnknownInstance(instance)` when no body holds
  it (`instance_location`); `Mismatch` when its current tag (`None` if untagged) isn't
  `from`; `MissingTag(t)` when `to` is `Some(t)` and `t` doesn't exist. Then sets the
  assignment, or removes it for `None`.

## Operations
Each one checks first, then applies its records with `self.apply` and pushes them to `log`;
on error the model and `log` are untouched. Unchanged values push nothing and return `Ok`.
- `add_tag(name)`: `BadTagName`; else one `AddTag(TagId(next), Tag::new(name))`, visible.
  Ids are never handed out twice in a model, even after a remove or an undone add. Two tags
  may share a name.
- `rename_tag(id, name)`: `MissingTag(id)`, then `BadTagName`; same name: nothing; else one
  `SetTag` that keeps `visible`.
- `set_tag_visible(id, visible)`: `MissingTag(id)`; unchanged: nothing; else one `SetTag`
  that keeps the name.
- `remove_tag(id)`: `MissingTag(id)`; else one `SetInstanceTag(Some(id) -> None)` per
  instance carrying it, in ascending instance id, then `RemoveTag(id, stored tag)`.
- `assign_tag(instance, tag)`: `UnknownInstance(instance)` first, then `MissingTag(t)` for
  `Some(t)`; same as the current tag: nothing; else one `SetInstanceTag { from: current, to:
  tag }`. `None` untags.
- `instance_visible(instance)`: false only when the instance carries a tag whose `visible`
  is false. True for untagged and unknown instances. It looks at the instance's own tag only;
  hiding a group does not change what `instance_visible` says about instances inside it
  (drawing skips them anyway because it doesn't descend into a hidden instance; that is
  `m10-tags-hidden`).

## Native format, version 7 (`format.rs`, see `docs/format.md`)
- `FORMAT_VERSION` becomes 7. Versions 1 to 6 read as before and give a model without tags;
  in them `tag` and `tagged` are `UnknownRecord`.
- `tag <id> <visible> <name>`: exactly 4 tokens, else `BadField`; id `u64`; visible `on` or
  `off` (`parse_flag`); name a quoted string (`quoted`). Applied as `AddTag`; model errors
  are `Rejected(...)` (`IdInUse`, `BadTagName`). Allowed anywhere before `end`, inside or
  outside a body; it doesn't change the current body.
- `tagged <instance id> <tag id>`: exactly 3 tokens, both `u64`, else `BadField`. When the
  instance already carries a tag: `BadField` (a second `tagged` for it, even the same tag).
  Else applied as `SetInstanceTag { from: None, to: Some(tag) }`; a missing instance is
  `Rejected(UnknownInstance(id))`, a missing tag `Rejected(MissingTag(id))`. Allowed anywhere
  before `end`; it doesn't need or change the current body.
- Writer: version 7 when the model has at least one tag (even one nobody carries); else the
  old rules. `tag` records in ascending id go after the `definition` records and before
  `body root`; `tagged` records in ascending instance id go after the last body, before
  `end` (`Tags::assignments` is already in that order).

## Tests (spec commit)
- `tests/tags.rs`: records, operations, errors, undo/redo, revision, equality.
- `tests/tags_format.rs`: exact text, round trip, older versions, error kinds and lines.
- `tests/tags_props.rs`: random edits (definitions, instances in any body, instance removal,
  tag add/rename/show/remove/assign): save/load round trip, undo/redo restores every step,
  `remove_tag` leaves no dangling reference, failed operations change nothing.
- `tests/curves_format.rs`: no longer pins version 6.

Out of scope: copy/paste, make-component and import carrying tags; drawing, picking,
snapping and export skipping hidden instances; any UI.

## Where to look
- `tags.rs`: `Tag`, `TagId`, `TagRecord`, `Tags` (`Tags::get`, `Tags::instance_tag`,
  `Tags::assignments`, `Tags::instances_with`), `tag_name_is_good`, the `impl Model` block.
- `model.rs`: `Model::apply`, `Model::apply_record` (the `RemoveInstance` and
  `ModelRecord::Tag` arms), `Model::remove_instance`, `Model::instance_location`,
  `Model::tags_mut`, `Model::set_length_format` (the check, record, apply, push pattern),
  `ModelError`.
- `format.rs`: `to_text` (version choice and record order), `apply_line` (the `units` and
  `sun` arms for version gating), `expect_count`, `parse_u64`, `parse_flag`, `quoted`,
  `push_quoted`, `rejected`, `FORMAT_VERSION`.
- Test helpers: `scene`, `assert_undo_redo`, `tag_record` (tests/tags.rs); `small`,
  `kind_at` (tests/tags_format.rs); `play`, `run_op`, `consistent` (tests/tags_props.rs).
