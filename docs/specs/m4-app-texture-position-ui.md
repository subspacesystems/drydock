# m4-app-texture-position-ui: Texture Position panel

A small panel lets the user position the texture on a face by typing values: slide it, turn
it, stretch it, or put it back to its default placement. The editor side exists
(`Editor::position_texture`, `TextureOp`; `docs/specs/m4-app-texture-position.md`). This task
adds the panel and wires it into the app.

Code: `app/crates/modeler/src/texture_panel.rs` (skeleton on main: fill in `texture_panel`),
and the wiring in `app/crates/modeler/src/app.rs`.
Tests: `app/crates/modeler/tests/texture_panel.rs` (`egui_kittest`).

## `TextureForm`

Public fields, edited in place by the panel. `Default` is `shift [0, 0]`, `turn 0`,
`scale [1, 1]`.

## `texture_panel(ui, form, available) -> Option<TextureOp>`

Draws, top to bottom, these widgets (labels exactly as written):

- heading "Texture Position"
- when `available` is false, a label "Select a face with a texture"
- "Move X (m)" and "Move Y (m)" drag values on `form.shift`, then a button "Move"
- "Turn (deg)" drag value on `form.turn`, then a button "Turn"
- "Scale X" and "Scale Y" drag values on `form.scale`, then a button "Scale"
- a button "Reset Texture"

Every control is disabled while `available` is false (clicking does nothing, returns `None`).

Buttons, on the frame they are clicked, return:

- "Move": `TextureOp::Move(DVec2::new(shift[0], shift[1]))`; the shift goes back to `[0, 0]`.
- "Turn": `TextureOp::Rotate { angle: turn.to_radians(), about: DVec2::ZERO }`; the turn
  goes back to 0.
- "Scale": `TextureOp::Scale { factor: DVec2::new(scale[0], scale[1]), about: DVec2::ZERO }`;
  the scale goes back to `[1, 1]`.
- "Reset Texture": `TextureOp::Reset`; the form is left alone.

Validity: "Move" is disabled unless both shift values are finite; "Turn" unless the angle is
finite; "Scale" unless both factors are finite and greater than 0. A disabled button returns
nothing and leaves the form alone. At most one op is returned per frame.

## App wiring (not covered by tests; the orchestrator checks it in the real window)

- The panel sits in the right-hand column below the Materials panel, with one `TextureForm`
  kept in `App`.
- `available` is true when exactly one face is selected and the side of it facing the camera
  (the side the Paint Bucket would paint from the same view) carries a material with a
  texture. Compute it from the editor's selection and the model each frame.
- An op from the panel goes to `Editor::position_texture(model, face, side, op)` for that
  face and side, then the document refreshes its scene like after any other edit. A false
  return changes nothing; the editor already set its own status message.
