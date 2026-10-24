# m4-app-copy-paste: Cut, Copy and Paste (editor)

Editor-level Cut / Copy / Paste. Acceptance tests: `app/crates/modeler/tests/editor_copy_paste.rs`.
Read `docs/specs/m4-copy.md` (`modeler_kernel::copy::{extract, insert}`, `Clipboard::bounds`) and
`docs/specs/m3-app-erase.md`. The skeleton on main has the method signatures
(`Editor::{clipboard, copy, cut, paste, pasting, paste_preview}`, bodies `unimplemented!`);
build them in `src/editor.rs`, and wire the menu/keys in `src/app.rs` (below).

## Editor

- `clipboard()` is `None` until something was copied or cut. It is kept while the document
  changes and survives undo.
- `copy(&model)`: extracts the selection of the open body into the clipboard
  (`copy::extract`). The model, the undo stack and the selection do not change. Returns true
  when the clipboard was set. With nothing selected: false, the old clipboard is kept,
  `message()` is `Select something to copy`. If the kernel refuses: false, old clipboard
  kept, `message()` is `Can't copy that`. On success `message()` is `None`.
- `cut(&mut model)`: like `copy`, then erases the selection from the open body as one undo
  step labelled `Cut` (an erase with that label; selection cleaned up as after Erase). True
  when the model changed. Nothing selected: false, `Select something to cut`, clipboard kept.
  If the copy or the erase is refused: nothing changes, the old clipboard is kept,
  `message()` is `Can't cut that`. Undoing the cut brings the entities back; the clipboard
  still holds them.
- Like the other editing commands, `copy`, `cut` and `paste` drop any tool in progress
  (line, shapes, move, ... and the typed text).
- `paste(&model)`: with an empty or missing clipboard returns false and `message()` is
  `Nothing to paste`. Otherwise it makes the Select tool current (through `set_tool`),
  starts pasting and returns true; `pasting()` is true. The model does not change yet.
- While pasting, `pointer_move` finds the point under the cursor the way the Move tool does
  with no from point (`infer_move_at`: snaps apply; empty space gives the ground/plane point)
  and remembers it. The paste is anchored by the point at the middle of the clipboard's
  `bounds()` in x and y and at its lowest z: that anchor is put on the cursor point, so the
  copy is translated by `cursor point - anchor` (in the open body's coordinates).
  `paste_preview()` returns the copy's edges at that position in world coordinates: the
  clipboard root's edges translated, or, when the clipboard holds any instance, the 12 edges
  of its bounds box translated. Empty when not pasting or before the first `pointer_move`
  of this paste.
- A `click` while pasting (any count, any modifiers) places the paste at the point under the
  click, using `copy::insert` into the open body with label `Paste`, and returns true:
  - the selection is exactly the entities `insert` returned; pasting ends; the tool stays
    Select; `message()` is `None`; one undo step labelled `Paste`; undo removes it all;
  - the clipboard is unchanged, so pasting again works;
  - if the click has no point (nothing under it to infer), nothing happens and pasting goes on;
  - if `insert` refuses: false, nothing changed, still pasting, `message()` is
    `Can't paste that`.
- `EditKey::Cancel` while pasting ends the paste (no model change, `paste_preview()` empty,
  `pasting()` false) and does nothing else. `set_tool` (any tool) and `erase_selection`,
  `make_component`, `explode_selection` also end it. Opening or closing an instance
  (`open_instance`, `close_path`) ends it too, since the anchor is in the old body's coordinates.
- Pasting is not limited to the body the clipboard came from: a clipboard copied at the
  root can be pasted inside an open instance and the other way round (the kernel handles
  definitions and materials).

## App (not covered by tests; checked in the real window)

- Edit menu gets Cut, Copy, Paste (Ctrl+X, Ctrl+C, Ctrl+V, Cmd on macOS) with the usual
  enablement: Cut and Copy need a selection, Paste needs a clipboard. Add them to `chrome.rs`
  as `Action::{Cut, Copy, Paste}` following how `MakeComponent` is done, with `MenuState`
  fields for enablement, and handle them in `app.rs`. Key handling must not fire while a
  text field has focus.
- While pasting, draw `paste_preview()` as the Move tool previews its segments, and show
  `Click to place the paste (Esc cancels)` as the help text.
- Document refresh after cut and paste, like after erase.
