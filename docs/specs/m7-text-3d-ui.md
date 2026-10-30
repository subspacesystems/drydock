# m7-text-3d-ui: Tools > 3D Text

Tools > 3D Text... asks for a line of text, a font size, a height and emboss or engrave. After
OK the text follows the cursor over the faces of the part being edited as an outline preview;
a click puts it on the face under the cursor, raised or sunk, as one undo step. If the
kernel refuses, the status bar says why and the text stays pending, so the user can try
another spot. Esc or choosing a tool drops it.

The geometry is merged: `modeler_dae::text_outline` (letters to flat loops) and
`modeler_kernel::text3d::text_3d` (draw and push/pull, solid kept, refusals). This task is
the dialog, the frame that lays the text on a face, the editor's pending-text mode and the
wiring. No font picker, no multi-line text, no rotating the text: it reads upright.

Skeletons on main: `modeler/src/text_tool.rs` (new module, `unimplemented!`),
`Editor::begin_text`, `text_pending`, `text_preview` (`unimplemented!`) and the field
`Editor::text` with its `TextState`, `Action::Text3d`, `Effect::Text3dDialog` (mapped in
`ViewState::apply`, done), `MenuState::can_text`, the Tools menu item "3D Text..." (done,
between Shrink by Clearance and Print Setup...), an empty `app.rs` arm for the effect and
`can_text: false` where the menu state is filled, `look::ROBOTO` made `pub` (the bundled font).

Files to change: `modeler/src/text_tool.rs`, `modeler/src/editor.rs`, `modeler/src/app.rs`
(and `modeler/src/look.rs`, `chrome.rs` only if needed).
Acceptance tests: `app/crates/modeler/tests/text_tool.rs`, `tests/text_tool_ui.rs`.

## `text_tool` module (plain functions, no window)

- `parse_mm(text)`: trim; drop one trailing `mm` and trim again; parse as `f64`; accept only
  finite values in `MIN_MM..=MAX_MM`. Else `None`.
- `TextDialog::new()`: text empty, `size_text` "10", `height_text` "1", `Emboss`.
  `from_options(o)`: the text, then size and height as `o.size_m * 1000.0` printed with `{}`
  (0.01 m is "10"), the mode. `options()`: `None` when the text is empty or either number
  does not parse with `parse_mm`; else `TextOptions` with the numbers divided by 1000.0.
- `text_dialog(ui, dialog)`: three labelled single-line boxes `Text`, `Font size (mm)`,
  `Height (mm)` bound to `text`, `size_text`, `height_text`; radios `Emboss` and `Engrave`
  bound to `mode`; buttons `OK` (disabled while `dialog.options()` is `None`) and `Cancel`.
  Returns `Some(TextOutcome::Ok)` / `Some(TextOutcome::Cancel)` on the frame a button is clicked.
- `text_frame(ctx, face)`: `plane = ctx.face_plane(face)?`. `out` = `plane.normal`, negated
  when `check_geometry(ctx).reversed_faces` holds the face (away from the material, as
  `text3d` defines it). `up` = `Y` when `|out.z| > 0.999`, else world `Z` made perpendicular
  to `out` (`Z - out * out.z`, normalized). `right = up.cross(out)` normalized. So text
  reads left to right and upright to someone looking at the face from outside, and
  `right x up = out`.
- `place_loops(outline, frame, center)`: the middle `(cx, cy)` of the bounding box of every
  loop point; each outline point `(x, y)` becomes `center + right * (x - cx) + up * (y - cy)`.
  One output loop per `outline.loops`, in order. No loops: empty.
- `refusal(error)`: `"Can't place text: "` followed by the error's `Display`.

## Editor

- **Lengths.** The options are world lengths in meters. The open body may sit inside scaled
  groups: `s = clearance_offset::uniform_scale(&self.world())`. Size and height passed to
  `text_outline` and `text_3d` are `size_m / s` and `height_m / s` (body coordinates). `None`
  for `s` (uneven scale or shear) refuses with the message `Can't place text: the part is
  stretched unevenly` (hover shows no preview).
- `begin_text(model, options)`: resolve the open body (`self.path.body(model)`); `None` sets
  the message `Can't change parts here` and returns false. Otherwise make the Select tool
  current (as `place_definition` does, which drops other pending modes), store the options
  with no hover, clear the message, return true. `text_pending()` returns the options.
- **Hover** (in `pointer_move`, as the placing mode does): when text is pending, cast the
  cursor ray into the open body (`body_ray`, `modeler_kernel::infer::ray_face` on the body's
  geometry) and keep `(face, hit point)` or `None`; no inference hint. Never changes the model.
- `text_preview(model)`: with a hover, the outline: `text_outline(look::ROBOTO, text, size,
  size / 200.0)` (size in body units), `text_frame`, `place_loops` with the hit point as
  center, each loop closed (the last point back to the first), as world-coordinate segments
  (`to_world`). No hover, no pending text, or any step failing: empty.
- **Click** (in `click`, before the placing branch): on a miss the message is `Click a face of
  the part being edited`; nothing else changes and the text stays pending. On a hit, call
  `text3d::text_3d(model, body, face, &loops, mode, height, &mut self.undo, "3D Text")`.
  `Ok`: clear the pending text, hint and message, return true (one undo step labelled
  `3D Text`; the kernel pushed it). `Err(e)`: the message becomes `refusal(&e)`, the text
  stays pending, return false, no undo step, model unchanged.
- **Dropped by** `EditKey::Cancel` (like the placing mode; returns false) and `set_tool`
  (grep the places that set `self.placing = None` and drop the text with it). Undo and redo
  leave it pending.

## App (`app.rs`, checked by hand in the real window)

- `can_text`: a document with a kernel model is open.
- `Effect::Text3dDialog`: ignore with no document. Otherwise open a modal window "3D Text"
  showing `text_dialog` with a `TextDialog` made from the options used last in this session
  (`TextDialog::new()` the first time); keep the dialog's state across frames the way the
  export dialog does. `OK`: close the modal, remember the options, `editor.begin_text(model,
  options)`; the editor's message shows if it refuses. `Cancel`: close, nothing else.
- While text is pending, the viewport draws `editor.text_preview(model)` in the same
  preview colour and stroke as `place_preview`, next to it in the viewport drawing code.
- The status bar already shows `editor.message()`; the refusal and the miss text appear
  there. Mark the document modified the way other editor changes do (`refresh_document`
  after a click that returned true).
- Pending text is a pointer mode in the viewport: a click goes to `editor.click` as for
  other tools.

## Not in scope

A font picker, several lines, turning or moving the text after placing it, editing placed
text, text on curved faces, a toolbar button or shortcut, a preview that turns red when the
kernel would refuse.

## Acceptance

- `cargo test -p modeler --test text_tool --test text_tool_ui`
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
- By hand in the window: Tools > 3D Text..., "Hi", OK, hover a box's top face (outline
  follows), click: letters rise, Ctrl+Z removes them; engrave works; a size larger than the
  face shows the refusal in the status bar.

## Where to look

- `harness/bin/show Editor::place_definition`, `Editor::click_place`, `Editor::place_preview`
  and the `placing` branches in `pointer_move`, `click` and `key` (the model for pending
  placement), `show Editor::body_ray`, `show Editor::to_world`, `show Editor::world`.
- `show text_3d`, `show Text3dError`, `show TextMode` (`modeler-kernel/src/text3d.rs`),
  `show text_outline`, `show TextOutline`, `show TextLoop` (`modeler-dae/src/text.rs`),
  `show ray_face`, `show check_geometry`, `show uniform_scale` (`clearance_offset.rs`).
- `show export_dialog` and the `Effect::ExportDialog` / `Effect::ImportDialog` handling in
  `app.rs` (modal window and kept dialog state), `show App::menu_state`, the preview drawing
  near `editor.place_preview(model)` in `app.rs`.
- Test helpers: `make_box`, `face_facing`, `World` in `tests/text_tool.rs`; `tools_harness`,
  `dialog_harness` in `tests/text_tool_ui.rs`.
