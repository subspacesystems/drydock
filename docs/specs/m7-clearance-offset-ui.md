# m7-clearance-offset-ui: Grow by Clearance and Shrink by Clearance

Select one or more groups or components, choose Tools > Grow by Clearance (or Shrink by
Clearance), and each flat-faced solid among them gets bigger (or smaller) by the fit check's
clearance on every face. The geometry is `modeler_kernel::offset` (`offset_body`, merged in
`m7-clearance-offset`); this task is the two menu items, the scale handling, the editor
method, the status line and the wiring.

Skeletons on main: `app/crates/modeler/src/clearance_offset.rs` (new module, `unimplemented!`),
`Editor::offset_instances` (`unimplemented!`), `Action::GrowByClearance` /
`ShrinkByClearance`, `Effect::OffsetByClearance(Direction)` (mapped in `ViewState::apply`,
done), `MenuState::can_offset`, an empty `app.rs` arm for the effect and `can_offset: false`
where the menu state is filled.

Files to change: `modeler/src/clearance_offset.rs`, `modeler/src/editor.rs`,
`modeler/src/chrome.rs`, `modeler/src/app.rs`.
Acceptance tests: `app/crates/modeler/tests/clearance_offset.rs`,
`app/crates/modeler/tests/clearance_offset_ui.rs`.

## Design choices

- **The clearance is the fit check's field.** One number, `ViewState::clearance_m`; the items
  use it as it stands (no new field). Grow and Shrink both pass a positive distance; the
  direction picks the sign.
- **Distance is in the world, the kernel works in the definition.** An instance's
  definition is offset by `clearance_m / s`, where `s` is the uniform scale of the instance's
  world transform: `path.transform(model) * instance.transform` (the open path's parents
  included). Each face then moves by the clearance as seen in the model.
- **Only similarities.** An instance whose world transform has uneven scale or shear would
  move its faces by different world distances. It is refused in the UI (`UNEVEN_SCALE`),
  never passed to the kernel. A mirror is fine.
- **A definition is offset once.** Several selected instances of one component share a
  definition; it changes once, with the scale of the first of them in selection order. Every
  other copy of the component changes with it (as with Repair); no message about that.
- **One undo step, whatever the scales.** `offset_bodies` takes one distance, so call
  `offset_body` for each part with its own distance into one shared `ModelEdit`, and when at
  least one succeeded push it once on the editor's undo stack with the direction's label. A
  part that fails leaves the model and the edit as they were (`offset_body`'s contract).
  Nothing succeeded: no undo step, undo state unchanged.
- **The fit-check tint** needs no code: the check is dropped by its existing stale rule
  (`FitCheck::is_current` sees the undo state change). A fully refused offset changes nothing,
  so a showing check stays.

## `clearance_offset` module

Every item is documented in the skeleton; this is the whole behaviour.

- `Direction::label()`: "Grow by Clearance" / "Shrink by Clearance".
- `selected_instances(selection)`: the `Entity::Instance` ids in selection order, each once.
- `uniform_scale(world)`: the lengths of the three axis columns (xyz of `x_axis`, `y_axis`,
  `z_axis`) are all `s = |x_axis|`, `s` finite and `> 0`, each within `1e-9 * s` of `s`, and
  every pair is at right angles (`|dot| <= 1e-9 * s * s`). Returns `Some(s)`, else `None`.
- `reason(error, direction)`: the strings in the skeleton's doc comment, exactly.
- `OffsetOutcome::message()`: see the skeleton. The number is `fit_check::format_mm`.

## `Editor::offset_instances`

1. `path.body(model)` and `path.transform(model)`; if either is `None`: Err "Can't change
   parts here".
2. The ids are `selected_instances(selection)` that are instances of the open body. None:
   Err "Select a group or component first". `clearance_m` not finite or not `> 0`: Err "The
   clearance is not usable". Every Err also sets the editor's message; nothing else changes.
3. Walk the ids in order. A definition seen before is skipped. Name = the instance's name,
   else its definition's name, else "part". `uniform_scale(outer * instance.transform)`:
   `None` puts `Refused { name, UNEVEN_SCALE }` in the list; `Some(s)` queues the definition
   with `s`.
4. For each queued part, `offset_body(model, BodyId::Definition(def), sign * clearance_m / s,
   &mut edit)` (sign +1 grow, -1 shrink). `Ok` counts as changed; `Err(e)` adds
   `Refused { name, reason(&e, direction) }`. Keep one refused list in
   walk (selection) order; scale refusals come out when met, kernel refusals after the
   offsets are tried, and the tests pin only lists with one kind.
5. If any changed: `self.undo.push(direction.label(), edit)`. Clear the editor's message and
   return the outcome. A part refused for scale or by the kernel is not an `Err`.

## Tools menu

After Fit Check, before Print Setup: "Grow by Clearance" and "Shrink by Clearance", plain
items (`menu_item`), enabled when `MenuState::can_offset`, returning
`Action::GrowByClearance` / `Action::ShrinkByClearance` and closing the menu.

## App (`app.rs`, checked by hand in the real window)

- `can_offset`: `selected_instances(editor.selection())` is not empty.
- `Effect::OffsetByClearance(direction)`: `editor.offset_instances(kernel_model, direction,
  view.clearance_m)` on the open document's kernel model. `Ok(outcome)`: set the status note to
  `outcome.message()` (reuse the `repair_note` tuple: message, selection, undo state, taken
  after the call, so it shows while both stay the same), and when `outcome.changed > 0`
  `refresh_document`. `Err(_)`: the editor's message is already shown in the status bar; clear
  the note.
- Mark the document modified the way other editor changes do (`refresh_document` does).

## Not in scope

A distance different from the fit check's, offsetting a face or an edge, curved parts,
naming the parts' copies, a keyboard shortcut.

## Acceptance

- `cargo test -p modeler --test clearance_offset --test clearance_offset_ui`
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.

## Where to look

- `harness/bin/show Editor::repair_instance` (the instance-level kernel edit this follows),
  `show offset_body`, `show OffsetError`, `show ModelEdit`, `show UndoStack::push`.
- `show EditPath::transform`, `show EditPath::body` (path and its world transform).
- `show format_mm` (`fit_check.rs`), `show menu_item` and the Fit Check item in `chrome.rs`'s
  Tools menu, `show clearance_field` for how the clearance is kept.
- `show App::fit_effect` and the `InfoAction::Repair` arm in `app.rs` (the note tuple,
  `refresh_document`), `show MenuState`.
- Test helpers: `add_box`, `add_instance`, `volume` in `tests/clearance_offset.rs`.
