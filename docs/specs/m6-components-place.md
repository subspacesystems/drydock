# m6-components-place: place a component definition

Editor-side placing for the Components panel (`m6-components-panel`, which reports
`ComponentAction::Place(id)`). Fill in the three stubs in `app/crates/modeler/src/editor.rs`:
`place_definition`, `placing`, `place_preview`. Model it on the existing paste flow
(`paste`, `click_paste`, `paste_preview`): same cursor inference (`infer_move_at`), same
cancel and tool-switch handling. Tests: `app/crates/modeler/tests/editor_components_place.rs`.

## Behaviour

- `place_definition(model, id)` starts placing an instance of definition `id`:
  - Refused (returns false, sets `message()` to a short text, changes nothing, does not start
    placing) when `id` does not exist, is a group, or is the definition the editor currently
    has open for editing.
  - Otherwise makes Select the current tool, ends any paste or other tool in progress, clears
    the message, remembers `id`, returns true.
- `placing()` is the definition waiting for its click, else `None`.
- The definition's origin (0,0,0) is what the cursor holds. After a pointer move over the
  viewport, `place_preview(model)` returns the definition's own edges (its direct geometry,
  not nested instances) shifted so the origin sits at the inferred cursor point, in world
  coordinates (use the open body's world transform like `paste_preview`). Empty when not
  placing or before the first pointer move.
- A click while placing adds one instance of the definition to the body being edited, with
  transform = translation to the inferred point, empty name, no material. It is one undo
  step labelled "Place Component". The new instance becomes the whole selection, placing
  ends, the click returns true. If the model refuses the instance, set a message, return
  false, keep placing off.
- Escape (`EditKey::Cancel`) ends placing without changing the model. `set_tool` (any tool),
  erasing, and the other editor operations that already end a paste end placing too.
- Placing the same definition again afterwards works.

## Out of scope

Drag-from-panel, rotation while placing, wiring into `App` (a later task connects the panel
action to `place_definition` and draws `place_preview`).
