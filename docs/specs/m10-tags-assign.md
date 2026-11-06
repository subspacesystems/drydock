# m10-tags-assign: assign the selection to a tag

UI wiring only; the kernel's `Model::assign_tag` exists. Tests: `app/crates/drydock/tests/tags_assign.rs`
(read them first; they pin labels and undo steps). Skeleton on main: the signatures below exist with
`unimplemented!()` bodies, and `App` already calls them.

## What to build

1. **`Editor::assign_selection_tag(model, tag: Option<TagId>) -> bool`** (`editor.rs`, next to `add_tag`).
   - Takes the `Entity::Instance` entries of the current selection (groups and components alike); edges
     and faces are ignored. Instances the model doesn't know are skipped.
   - Calls `Model::assign_tag` for each into **one** `ModelEdit`, then one undo step: label
     "Assign Tag" for `Some`, "Remove Tag" for `None`. The selection is left as it is.
   - Returns false, with no undo step, when no instance is selected or nothing would change (every
     instance already has that tag). A missing tag (`ModelError::MissingTag`) returns false with
     `message` set ("Can't assign that tag") and no step; nothing is half-applied.
   - Success clears `message`. Do not touch `set_tag_visible` or other selection code
     (`m10-tags-hidden-selection` edits that area).
2. **`entity_info::selection_tag(model, body, selection)`**: looks only at `Entity::Instance` entries that
   `body` has. None of them: `None`. Else `Some(Shared::Same(tag))` (`Same(None)` = untagged) when all
   share the tag, `Some(Shared::Varies)` otherwise.
3. **`entity_info::tag_label(model, shared)`**: "No tag", "Mixed", the tag's name, or "?" when the
   model doesn't have the tag.
4. **Entity Info row** (`entity_info_panel`, after the Material/"In model" rows, before the solid rows):
   when `selection_tag` is `Some`, draw `egui::ComboBox::from_label("Tag")` whose selected text is
   `tag_label`. Its popup lists "No tag" first, then the tags in `tags_panel::listed_tags` order; the
   current choice is highlighted. Picking a different value returns `InfoAction::SetTag(Option<TagId>)`
   (picking the current value returns nothing). No row when the selection has no instance. The tests
   find the box with role ComboBox and label "Tag", and read its value.
5. **Tags panel buttons** (`tags_panel`): a "No tag" button next to "Add", and in each tag's row, before
   the delete button, a button with the visible text "Assign" and the accessible label
   "Assign selection to {name}" (use `widget_info` as the Delete button does). All are enabled only when
   `TagsPanel::selected > 0` (the app sets it each frame from the selection's instances). Clicks return
   `TagAction::Assign(Some(id))` / `TagAction::Assign(None)`; like the other actions, only one action
   per frame.
6. `app.rs` already maps `TagAction::Assign` and `InfoAction::SetTag` to `assign_selection_tag` and sets
   `tags.selected`; the existing `changed` handling refreshes the document. Check it still compiles and
   marks the document dirty like the other tag actions; add nothing else there.

## Not in this task

Kernel changes, drag-and-drop, tag colours, per-entity tags for edges and faces, changing how hidden
tags affect the selection.

## Where to look

`show Editor::add_tag`, `show Editor::remove_tag` (undo step pattern), `show Model::assign_tag`,
`show Model::instance_tag`, `show TagsPanel`, `show TagAction`, `show listed_tags`, `show tags_panel`,
`show entity_info_panel`, `show InfoAction`, `show Shared`, `show describe`; in the tests the helpers
`scene`, `panel_harness`, `info_harness`, `combo_value`. `PanelAction::Tag` and `apply_info_action` in
`app.rs` for the existing wiring.
