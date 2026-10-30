# m7-clearance-ui: the fit check in the app

Pick two parts, choose Tools > Fit Check, and the faces of either part that are closer to the
other than the clearance turn yellow in the viewport. The geometry is `m7-clearance`
(`modeler_dae::{instance_triangles, close_triangles, tint_fit, DEFAULT_CLEARANCE_M}`); this
task is the menu item, the clearance field, the check itself in a window-free module and the
upload.

Skeletons on main: `app/crates/modeler/src/fit_check.rs` (new module, `unimplemented!`),
`chrome::clearance_field` (`unimplemented!`), `Action::FitCheck`, `Effect::FitCheck`,
`Effect::RecomputeFit`, `ViewState::clearance_m` / `set_clearance_mm` (done),
`MenuState::fit_check` / `can_fit_check`, empty `app.rs` arms for the two effects.

Files to change: `modeler/src/fit_check.rs`, `modeler/src/chrome.rs`, `modeler/src/app.rs`.
Acceptance tests: `app/crates/modeler/tests/fit_check.rs`, `app/crates/modeler/tests/fit_check_ui.rs`.

## Design choices

- **Menu: Tools, not View.** View's items are display modes that stay on while you work
  (Axes, Overhangs). Fit Check is an analysis of a selection that goes away when the
  selection or the model changes, like a tool's result, so it sits at the end of Tools after a
  separator, as a check item.
- **The check is stale after any change** and is dropped, not recomputed, except when the
  clearance field changes.
- **Fit wins over overhang** where a face is both: the fit color is the one to act on.
- **Counts are triangles.** The report counts the triangles `close_triangles` flagged (a square
  face is two), and says "triangles", because mapping them back to faces is not worth it yet.

## `fit_check` module

Every item is documented in the skeleton; this is the whole behaviour.

- `selected_pair(selection)`: `Some((a, b))` only for exactly two entries, both
  `Entity::Instance`, with different ids. In selection order.
- `parse_clearance_mm(text)`: trim; drop one trailing `mm` (and spaces before it); what is left must be
  digits with at most one `.` (so no sign, no exponent, no `,`, no `NaN`/`inf`, and not empty;
  `.3` is fine); the number must be within `MIN_CLEARANCE_MM..=MAX_CLEARANCE_MM`. Result in
  meters (`mm / 1000`).
- `format_mm(meters)`: `meters * 1000` with 3 decimals, then trailing zeros and a trailing `.`
  removed.
- `FitCheck::run(model, path, selection, undo_state, clearance_m)`: pair from `selected_pair`
  (else the "Select two groups or components to check their fit" error); `body = path.body`,
  `outer = path.transform` (else "Can't check the fit here"); the triangles of A and of B from
  `instance_triangles(model, body, id, outer)`; `close_triangles(&ta, &tb, clearance_m)`;
  `flagged` is A's flagged triangles in index order, then B's; `in_a` / `in_b` their counts.
  The other fields are copied from the arguments.
- `recompute(model, clearance_m)`: the same, with the stored a, b, path and undo state. When the
  path no longer resolves, a copy with the new clearance and nothing flagged. (Instances that
  no longer exist give empty triangle lists, so nothing is flagged.)
- `is_current(path, selection, undo_state)`: path equal, undo state equal, and `selected_pair`
  is `(a, b)` or `(b, a)`.
- `message()`: see the skeleton. `format_mm` supplies the clearance text.
- `display_scene(scene, overhang_degrees, fit)`: the unchanged `scene` (`Cow::Borrowed`) when
  both tints are off; else `tint_overhangs` when on, then `tint_fit(.., &fit.flagged)` when
  there is a check. Fit last, so fit-colored triangles leave the overhang batch.

## Clearance field (`chrome::clearance_field`)

A horizontal row: the label "Clearance", a single-line text box about 48 px wide holding `text`,
the label "mm". Typing changes `text` and returns `None`. The value is **committed** when the
box loses focus after being edited (Enter, Tab or a click elsewhere): good text (by
`parse_clearance_mm`) is rewritten with `format_mm` and returned in meters; bad text is
replaced by `format_mm(current_m)` and `None` is returned. A box that lost focus without any
edit commits nothing. A commit must be reported on one frame only: remember "edited" in egui's
temporary data, set on `response.changed()`, and clear it when the commit is handled (the kittest
tests would catch a field that reports again on the frame after a rejected value).

## Tools menu

After the tool groups: `ui.separator()`, then a check item "Fit Check" (`menu_check_item`) checked
when `MenuState::fit_check`, enabled when `can_fit_check || fit_check`. Choosing it returns
`Action::FitCheck` and closes the menu.

## App (`app.rs`, checked by hand in the real window)

- **One upload path.** Today `upload_scene`, `refresh_document` and `show_editor_path` each tint and
  call `renderer.upload`. Replace the three with one method (say `show_scene`) that takes the
  document's scene and the current tints, calls `fit_check::display_scene` with
  `view.overhangs.then_some(view.overhang_degrees)` and `self.fit.as_ref()`, and uploads. After
  this task `app.rs` has no `tint_overhangs(` or `tint_fit(` call and one `gpu.renderer.upload(` call and one `display_scene(` call
  (a test reads the source to check this). The document's own `scene` stays untinted.
- **State.** `fit: Option<FitCheck>`, a `fit_note: Option<(String, Vec<Entity>)>` for refusals (the
  message and the selection it was made with), and `clearance_text: String` starting at
  `format_mm(DEFAULT_CLEARANCE_M)`.
- **`Effect::FitCheck`.** With a check showing: drop it and re-upload. Without: `FitCheck::run`
  on the document's kernel model with the editor's `path()`, `selection()`, the undo stack's state
  (`undo_state()` in the app) and `view.clearance_m`; `Ok` stores it and re-uploads, `Err(message)`
  sets `fit_note` and changes nothing else. No document or no kernel model: nothing.
- **`Effect::RecomputeFit`.** With a check showing, `recompute` it with `view.clearance_m` and
  re-upload; otherwise nothing.
- **Dropping a stale check.** Every frame (where `sync_editor_path` runs) and inside the upload
  method: when `fit.is_current(editor.path(), editor.selection(), undo_state())` is false, drop it
  and upload again. A new, opened or reverted document drops it too. `fit_note` is shown only
  while the editor's selection equals the one stored with it.
- **Status bar.** While a check shows: `fit.message()` in the normal color, then the clearance
  field (`chrome::clearance_field(ui, &mut clearance_text, view.clearance_m)`). A commit calls
  `view.set_clearance_mm` with the committed meters times 1000 (or add a meters variant if you
  prefer; the ViewState test pins `set_clearance_mm`), and handles the returned effect through the
  usual effect path. While `fit_note` shows: the text in the normal message color. `status_bar` is
  `&self` today; the field needs `&mut`, so change what it must.
- **MenuState.** `fit_check: self.fit.is_some()`, `can_fit_check:
  selected_pair(selection).is_some()` (fill them where the other fields are filled).
- Editing with the check on drops it (undo state changes); the user chooses Fit Check again.

## Not in scope

Naming the parts, a face count instead of triangles, a keyboard shortcut, remembering the
clearance between sessions, a check between a part and the rest of the model.

## Acceptance

- `cargo test -p modeler --test fit_check --test fit_check_ui`
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests (existing menu and overhang tests stay green).
