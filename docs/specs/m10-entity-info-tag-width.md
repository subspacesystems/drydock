# m10-entity-info-tag-width: Entity Info's Tag box stays inside the panel width

Sign-off finding: with a group tagged with a long name selected, Entity Info's Tag combo
box shows the whole name, pushes the right-hand dock wider for good, and clips its "Tag"
label (drawn right of the box) at the screen edge.

## Required
- **Label left:** draw "Tag" first, then the box, in one `ui.horizontal`, like the Name and
  Material rows.
- **Width from the row:** the box is as wide as the space left in the row, never less than
  60 px, and never wider than that. It must not widen the panel.
- **Truncate:** the selected text is truncated with an ellipsis (no wrapping); the dropdown
  entries truncate too.
- **Unchanged:** texts "No tag" / "Mixed" / the name, `InfoAction::SetTag`, and the box is
  still found as a combo box with accessible label "Tag" (tests/tags_assign.rs looks it up
  by role and label; its value must stay the full text). Link the label to the box
  (`labelled_by`) so that keeps working.

## Tests
`crates/drydock/tests/entity_info_tag_width.rs` (kittest, 240-px resizable right panel).
Existing `tests/entity_info.rs` and `tests/tags_assign.rs` must keep passing. Only
`entity_info.rs` needs changing.

## Where to look
`show` these: `entity_info_panel`, `tag_label`, `selection_tag`, `value_row`, `name_field`
(in entity_info.rs); `listed_tags` (tags_panel.rs); tests `tags_panel_width.rs` for the
panel harness; `tags_assign.rs` `combo` helper for the lookup.
