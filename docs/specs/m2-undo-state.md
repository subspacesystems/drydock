# m2-undo-state: the `*` follows the saved state; Edit menu

Tyrel's report: save, undo, redo, and the title still shows `*` although the model is back at the saved state. Also, undo and redo have keys but no menu.

## Kernel: `UndoStack::state() -> u64` (`modeler-kernel/src/model.rs`)
A value that identifies the point in the edit history the stack is at.
- Nothing applied (new stack, or everything undone): `0`.
- Otherwise a number unique to the newest applied edit. Give each pushed edit a fresh id from a counter kept in the stack (ids are never reused, also after a redo list is dropped). Undo then redo returns the same values as before.
- An empty edit is not pushed, so it leaves the state unchanged (already how `push` behaves).
Tests: `modeler-kernel/tests/undo_state.rs`.

## Document (`modeler/src/document.rs`)
- `saved_state: u64` (field exists, 0 on open/new): the `UndoStack::state` at the last open or save.
- `mark_saved_at(state)`: `saved_state = state`, `modified = false`.
- `sync_modified(state)`: `modified = (state != saved_state)`.
- `refresh()` stays as is (sets `modified`); the app calls `sync_modified` right after it.
Tests: `modeler/tests/document_edit.rs` (new tests at the end).

## Window (not unit tested)
In `modeler/src/app.rs`:
- After every call that refreshes the document, call `doc.sync_modified(editor.undo_stack().state())`; after a successful save or save-as call `doc.mark_saved_at(...)` the same way. The title and the unsaved-changes prompt then use the corrected flag.
- Add an Edit menu next to File with Undo and Redo. Each shows its label (`undo_label`/`redo_label`, e.g. "Undo line") and is disabled when there is nothing to do. They do what Ctrl+Z / Ctrl+Y do now (go through the editor's key handling, then refresh as for any edit).
- Touch only `model.rs`, `document.rs`, `app.rs`.
