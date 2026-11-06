# m10-tags-hidden-selection: hiding a tag drops its instances from the selection

Milestone M10. Builds on `m10-tags-hidden` (hidden instances are skipped by drawing) and
`m10-tags-panel` (which adds `Editor::set_tag_visible` with the plain show/hide step).

## What

Hiding a tag must not leave invisible things selected or open. Change
`Editor::set_tag_visible(&mut self, model: &mut Model, id: TagId, visible: bool) -> bool`
in `app/crates/drydock/src/editor.rs`. Read it with `harness/bin/show Editor::set_tag_visible`;
the file is large, so do not read it whole.

## Behaviour

- **One undo step**, labelled "Hide Tag" when `visible` is false, "Show Tag" when true. The step
  is made as for `rename_definition`: a `ModelEdit` log, `Model::set_tag_visible`, then
  `self.undo.push(label, log)`.
- **False, nothing changes** (no undo step, selection and path untouched) when the tag does not
  exist or already has that visibility.
- **Hiding drops from the selection** every `Entity::Instance(i)` whose instance is no longer
  visible (`Model::instance_visible(i)` is false after the change). Edges and faces stay.
  The remaining order is kept. Showing a tag never changes the selection.
- **Hiding closes the edit path** when any step of `self.path().steps` is now a hidden
  instance: close back to the level above the outermost hidden step, that is, keep only the
  steps before it. Use `close_path()` repeatedly (it also clears the selection and recomputes
  the transforms); do not edit the path field by hand. Order: drop from the selection first,
  then close the path (closing clears the selection inside the path anyway).
  If no step is hidden the path stays open and its selection is as described above.
- **Undo restores visibility only.** The selection and path stay as they were after the hide.
  No code is needed in the undo path for this.
- Message: leave `self.message` as the plain show/hide step sets it; add none.

## Tests

`app/crates/drydock/tests/editor_tag_selection.rs` (headless, no window). They fail until the
function is done. Do not edit them.

## Where to look

- `Editor::set_tag_visible` (editor.rs), `Editor::rename_definition` (undo step pattern),
  `Editor::close_path`, `Editor::select`, `Editor::selection`, `Editor::path`.
- `Model::instance_visible`, `Model::set_tag_visible`, `Model::tag`, `Model::instance_tag`
  (`drydock-kernel/src/tags.rs`).
- `EditPath` (`steps: Vec<InstanceId>`), `Entity` (`drydock-kernel/src/select.rs`).
- Test helpers: `world()` in the new test file; undo is checked by cloning
  `editor.undo_stack()` and calling `undo(&mut model)` (see `tests/entity_info.rs`).
