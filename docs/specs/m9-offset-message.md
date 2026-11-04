# m9-offset-message: Offset's refusal message stays in the status bar

## Problem
In the real window, Offset with a typed `5000` + Enter, or a click past the far side of the
face, is refused (right) but no message ever shows. The window calls `Editor::pointer_move`
on every frame while the viewport is hovered, and `Editor::move_offset` ends with
`self.message = None` on every call, so the message set by the refused commit is gone one
frame later. The m9-offset-tool tests call `move_to` once, so they passed.

## Rule
`move_offset` clears `self.message` only when the pointer move really changes the preview:

- Compute `distance` as now. Compare with the stored `offset.distance` (before it is
  overwritten): they differ when `(distance - stored).abs() > TOL`.
- If they differ **and** the new distance previews (`faceoffset::offset_loops` returns `Ok`,
  or the distance is within `TOL` of zero so there is nothing to preview), clear the message.
- Otherwise leave the message as it is: a repeat call at the same cursor keeps it, and moving
  from one too-big distance to another too-big one keeps it too.
- Everything else in `move_offset` stays: hover, stored distance, preview segments and hint
  are updated on every call as now.

This also keeps "Not a length" visible, since a bad typed value does not change the stored
distance.

Nothing else changes: commit success, Escape, undo and tool switch still clear the message
where they do now. Only `move_offset` in `editor.rs` is edited.

## Tests
`app/crates/drydock/tests/editor_offset_message.rs` (already written; do not edit): a refused
click, a refused typed length (also `5000`) and a bad typed value keep their message over five
repeated `pointer_move` calls at the same cursor; moving to a distance that fits clears it
(and a later click commits). `editor_offset.rs` must still pass.

## Where to look
- `harness/bin/show Editor::move_offset` (editor.rs ~3631): the only function to change. Do
  not read the whole 165 KB file.
- `harness/bin/show OffsetState` (field `distance`), `TOL`, `faceoffset::offset_loops`.
- `Editor::commit_offset` and `Editor::enter_offset` show where the message is set.
- Test helpers: the `World` struct in `tests/editor_offset_message.rs` (`move_to`, `click`,
  `enter`).
