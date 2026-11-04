# m9-followme-undo: Undo and Redo drop Follow Me's picks and preview

## Problem
With a profile and path picked (preview showing), Ctrl+Z undoes earlier work but Follow Me keeps
its picks and keeps drawing the swept preview, even of a face that no longer exists. Undo and
Redo already clear Push/Pull (`self.push = None`) and Offset (`idle_offset()`); Follow Me is missing.

## Rule
`EditKey::Undo` and `EditKey::Redo` in `Editor::key` call `self.follow.reset()` (picks, preview,
message), whether or not the undo or redo succeeds, next to `self.idle_offset()`. After either key:
`follow_profile()` is `None`, `follow_path()` is empty, `preview()` is empty and `follow_hint()` is
`Some(HINT_PROFILE)`. Enter afterwards changes nothing (returns false, model untouched). The tool
works normally again when the user picks anew.

## Change
Only the two arms `EditKey::Undo` and `EditKey::Redo` in `app/crates/drydock/src/editor.rs`: add
`self.follow.reset();`. Nothing else. Do not edit the tests.

## Tests
`app/crates/drydock/tests/followme_undo.rs` (5 tests: undo, redo, undo that really removes a step,
Enter after either, picking again).

## Where to look
- `Editor::key`, arms `EditKey::Undo` / `EditKey::Redo`, and `Editor::idle_offset` (editor.rs)
- `FollowTool::reset` (follow_tool.rs)
- tests/followme_tool.rs for the original World helper
