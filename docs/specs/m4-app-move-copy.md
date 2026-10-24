# m4-app-move-copy: Move with copy (editor)

Make the Move tool able to copy. Acceptance tests: `app/crates/modeler/tests/editor_move_copy.rs`.
Read `docs/specs/m3-app-move.md` (how Move works) and `docs/specs/m4-copy.md`
(`modeler_kernel::copy::copy_selection`, already on main).

The skeleton on main has `Editor::move_copy()` and `Editor::set_move_copy(bool)` (a plain flag).
Everything below is yours to build in `src/editor.rs` and `src/app.rs`.

## Editor

- Copy mode is off at start. `set_move_copy` turns it on or off; `set_tool` (any tool, even
  Move) turns it off again.
- With the Move tool, copy mode on, and the tool in its second stage (a from point is set), a
  click that would commit a move, or a typed distance that would commit one, commits a **copy**
  instead: `copy_selection` into the open body with the translation by the same offset the move
  would have used. The originals stay where they are, unchanged.
- After a copy:
  - the selection is exactly the entities `copy_selection` returned (so the next copy copies the
    copy, and the originals are no longer selected);
  - the tool is back at its first stage (as after a move), copy mode stays on;
  - there is one undo step, labelled `Copy`; undoing it removes the copy and nothing else;
  - `message()` is `None`.
- A click with an offset no longer than the linear tolerance, or a typed zero, does nothing, as
  for a move (no undo step, no copy).
- If the copy is refused, nothing changes, the tool is left as it was, and `message()` is
  `Can't copy that`.
- Copy mode only matters when the stage-two commit happens, so it may be turned on or off
  between the two clicks. With it off the commit is the ordinary move.
- Retyping a distance right after a commit (stage one, the "retype last move" feature) keeps
  working for moves; after a copy there is nothing to retype and it behaves as if no move had
  been made (`message()` is `Click a start point first`).
- Inside an open instance the copy goes into the open body and the offset is the same one the
  move would use there.

## App

- While the Move tool is active, pressing and releasing Ctrl (Cmd on macOS) with no other key
  or button in between toggles copy mode. Ctrl held for another gesture (Ctrl+click, Ctrl+Z,
  Ctrl+drag with other tools) does not toggle it. Leaving Move for another tool clears it
  (through `set_tool`).
- The status bar help text for Move says `Ctrl: copy` and, while copy mode is on, `Copy` shows
  in the measurement label's place of the status as `Distance (copy)`. If the existing
  measurement label code makes that awkward, show the word `Copy` next to the label instead.
  This is not covered by tests; it is checked in the real window.
