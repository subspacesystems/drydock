# m10-tags-rename-width: the rename box stays inside the Tags panel

Sign-off finding: double-clicking a tag name in the Tags panel (docked right, about 240 px)
starts the rename, and the dock grows to about 600 px. It stays that wide after Enter.

## Cause
In `tags_panel` the rename `TextEdit` uses `desired_width(f32::INFINITY)` (the label it
replaces is as wide as the name). Together with the Assign and delete buttons that follow in
the same `ui.horizontal` row, the row needs more than the panel offers, so the resizable
dock panel grows to fit it and keeps the new width.

## Required behaviour
- While a tag is being renamed, the row is never wider than the room the panel has: the box
  takes the width left after the eye toggle, the Assign button, the delete button and the
  item spacing, and never more (a small floor such as 40 px is fine for very narrow panels).
- The panel's width is the same before, during and after the rename (Enter, focus loss or
  Escape).
- Rows that are not being renamed look and behave as today; accessible labels, actions
  (`TagAction::Rename`, one per confirmed change) and focus handling are unchanged.
- Long names (not only the rename box) must not widen the panel either: a name label wider
  than the room is truncated (`Label::truncate()`), not wrapped onto the buttons.

## Hint
Easiest is to lay the row out right-to-left for the two buttons first (inside the same
`ui.horizontal`, using `ui.with_layout(Layout::right_to_left(Align::Center), ..)`), then give
the left part (eye, label or box) `ui.available_width()` as its `desired_width`. Any layout
works if the tests pass and the buttons keep their order Assign, then x.

## Tests (already written, do not edit)
`app/crates/drydock/tests/tags_panel_width.rs`: the panel inside a resizable
`egui::Panel::right` with a `ScrollArea`, as the app docks it. A 40-character rename text
must not grow the panel (before 240, while renaming, after Enter) and the delete button
stays inside it. They fail today (240 -> 432/464). Existing `tags_panel.rs` and
`tags_assign.rs` tests must stay green.

## Where to look
`tags_panel` (the `editing_this` branch and the row's `ui.horizontal`), `eye_toggle`,
`TagsPanel::editing`, test helper `harness` / `settle` in `tags_panel_width.rs`.
