# m10-start-state: open on a new empty model, Select first

Tyrel: opening the app must be the same state as File > New. Today nothing can be
drawn until File > New (status bar says "No model", the panels say "No model", Rectangle
draws nothing), and the starting tool is Orbit; he wants Select.

## Behavior

1. **Start state.** `App::new` without a file argument ends in the state `new_document`
   gives: a new empty `Document`, an `Editor`, a fresh `ViewState` (bed, bed visibility and
   overhang angle from the print settings), the title of the new document, no error, nothing
   loading. With a file argument nothing changes: it is loaded as today and the model stays
   `None` until the load finishes.
2. **One shared function.** `start::Fresh::new(&PrintSettings)` (skeleton on main,
   `src/start.rs`) builds the document, editor, view and title. `new_document` and `App::new`
   both use it, so the two can't drift apart. `Fresh::new` needs no `eframe::Frame` or egui
   context.
3. **GPU scene.** `App::new` has no `Frame`, so it can't call `show_scene`. It leaves
   the empty startup scene it already uploads (an empty model's scene is empty anyway) and sets
   `dirty`. The window title is set on the first frame: if the app has no title command to
   send in `new`, send `ViewportCommand::Title` once from `ui` on the first call (a bool
   field), or use the window title `main.rs` already passes if that equals the new document's
   title. Keep it simple.
4. **Default tool is Select.** `Editor::new()` and `ViewState::new()` start with
   `Tool::Select`. Update both doc comments ("Orbit tool" becomes "Select tool"). Leave
   `Tool`'s `Default` derive alone unless a compile error or test needs it.
5. **File > New and File > Open.** New gives Select (same function). Open of a file keeps
   whatever `Editor::new()` gives, which is now Select.

The tests that pinned Orbit (`tests/editor.rs`, `tests/viewstate.rs`) are updated in the spec
commit. Run `harness/bin/run-tests -p drydock`; nothing else relied on Orbit.

## Tests

`tests/start_state.rs` (already written, failing until `Fresh::new` exists): Select in editor
and view; `Fresh` has a kernel model, an untitled unmodified document, the New title, empty
selection and undo stack; bed/overhang taken from the print settings; a rectangle drawn on a
fresh model adds one face with four edges.

Not testable in-process: `App` lives in the binary. Check by hand that the window opens with
Select highlighted in the toolbar, the status bar no longer says "No model", and a rectangle
can be drawn at once (`harness/bin/gui-session` can do this).

## Where to look

`show App::new`, `show App::new_document`, `show App::show_scene`, `show Document::new_empty`,
`show Document::title`, `show Editor::new`, `show ViewState::new`, `show Fresh`,
`show PrintSettings`; test helpers in `tests/editor_shapes.rs` (`World`, `view`).
