# m2-app-ui: drawing in the window

Wire the editor (`modeler/src/editor.rs`, done and tested) into the egui window so a person can draw, select, undo and save. Files: `modeler/src/document.rs` (the tested part), `modeler/src/app.rs`, `modeler/src/viewstate.rs`, `modeler/src/main.rs` and `modeler/src/input.rs` only where needed. No new dependencies. Never panic. Read `docs/specs/m2-app-editor.md` and `m2-app-shapes.md` for what `Editor` offers (`pointer_move`, `click`, `box_select`, `text`, `key`, `measurement`, `hint`, `preview`, `selection`, `draw_plane`, `message`, `undo_stack`).

## `Document` (acceptance tests in `tests/document_edit.rs`)
- `new_empty()`: path `untitled.modeler`, kind `Native`, `kernel` an empty `Model`, `untitled` true, `modified` false, zero stats (`size_m` zeros), an empty scene. `title()` stays "untitled.modeler - modeler".
- `refresh()`: with a kernel model, `scene = kernel_scene(model)`, `stats` recomputed the same way `open` does (including `faces`, `edges`), `modified = true`. Without one: no change.
- `open` leaves `untitled` and `modified` false. A successful `save` or `save_as` sets `modified` false and `untitled` false (`save_as` also moves `path`/`kind` as before). A failed save leaves both unchanged. `save` on an untitled document is an error that names the file and says to use Save As.

## Window (not unit tested; keep logic that can be tested in `Editor`/`ViewState`)
- The app owns one `Editor` per open document; opening or creating a document makes a new one. New: File > New and `Ctrl+N`, a `Document::new_empty()` with the camera on a default view.
- Toolbar (left side): Select, Line, Rectangle, Circle, Arc, then Orbit, Pan, Zoom. Plain text labels (no icons or art from elsewhere); the active tool is highlighted. Keys already in `input.rs` do the same.
- Pointer: in the drawing and select tools events go to the editor with a `Viewport` built from the current camera and the viewport rect (see `m2-app-editor.md` for the projection), not to the camera. Orbit/Pan/Zoom tools and the middle button / scroll wheel still navigate as in M0 in every tool. A left drag with Select is a box (`box_select`).
- After any call that returns true (undo step pushed, undo or redo done, selection changed does not count) call `doc.refresh()` and re-upload the scene.
- Measurement box: bottom right of the viewport, shows `measurement()` label and value, and the typed text (`typed()`); typed characters go to `text`, Enter and Escape to `key`.
- Overlays drawn on top of the 3D view with egui painters from `Editor` output: `preview()` as thin dark lines, the `hint()` point as a small square whose color depends on the snap (endpoint green, midpoint cyan, on edge red, on face blue, axis/ground gray; pick your own palette), `selection()` highlighted in an accent color (edges thick, faces tinted), `draw_plane()` normal shown by coloring the preview.
- Status bar: `message()` in place of the stats while it is set; the title shows `*` after the name when `modified`.
- File > Save (`Ctrl+S`) on an untitled document opens the Save As dialog. Closing or opening another file while `modified` asks first (Save / Discard / Cancel).
- `--screenshot` must keep working unchanged and must not need an editor.

## Tests
`tests/document_edit.rs`: the `Document` rules above, using `draw_segments` on `doc.kernel` to make an edit.
