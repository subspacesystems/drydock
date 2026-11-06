# m10-tags-row-layout: tag name next to its eye again

Sign-off finding: since the rename-width fix, a Tags panel row reads eye, a big gap, then
`Tag 1 [Assign] [x]`: the name label (and the rename box) are right-aligned next to Assign.
Before that fix it read eye, name, Assign, x from the left.

## Required behaviour
- Eye toggle first, at the left.
- The name label, or the rename `TextEdit` while renaming, starts right after the eye
  (within about 12 px) and is left-aligned.
- Assign and the delete button stay right-aligned at the panel's right edge, in the order
  name, Assign, x.
- The rename box fills the space between the eye and Assign and never widens the panel;
  long names are truncated, not wrapped. Every test in `tests/tags_panel_width.rs` and
  `tests/tags_panel.rs` keeps passing unchanged.
- Widgets, texts and accessible labels do not change (eye, 'x', `Delete <name>`,
  `Assign selection to <name>`). Nothing else in the panel changes.

## Hint
In `tags_panel`, keep the outer `right_to_left` block for delete and Assign, and put the
name or `TextEdit` in a nested `ui.with_layout(Layout::left_to_right(Align::Center), ..)`
that takes the remaining width (use `ui.available_width()` after the buttons for the box's
`desired_width`, floor 40). The eye is drawn before the right-to-left block. Any layout works
if the tests pass.

## Tests (already written, do not edit)
`app/crates/drydock/tests/tags_row_layout.rs`, in a 240-px resizable right panel: the name's
left is within 12 px of the eye's right; delete ends at the panel's right edge, Assign is
between the name and delete; while renaming, the box starts next to the eye and ends before
Assign. The first fails today (the name starts 170 px after the eye); the others guard against regressions.

## Where to look
`tags_panel` (the row's `ui.horizontal`, the `editing_this` branch), `eye_toggle`,
`TagsPanel::editing`; test helpers `harness` in `tests/tags_row_layout.rs` and
`tests/tags_panel_width.rs`.
