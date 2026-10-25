# m4-texture-turn: turns that never skew the texture, and press-drag on the handles

Milestone M4 (Tyrel, round 4: after resizing corners at an angle the image copy
became a slanted quadrilateral he could not get back; and he expects to press, hold and
drag the corner grips, not click, release, move and click again).

## Problem

`TextureOp::Rotate { angle, about }` turns the map in texture space
(`map.then(rotate(angle) about about)`). After an unequal stretch (`Scale` with
`factor.x != factor.y`) texture space is no longer square on the face, so a turn there is
a shear on the face: the image copy stops being a rectangle. The turn grip and its edge
snap measure angles in texture space too.

## What to build (`modeler` app crate)

### Turn (`Editor::position_texture`, the turn grip, the typed Turn)

`Rotate { angle, about }` turns the image **rigidly on the face**:

- The texture point `about` keeps its place on the face.
- Lengths and right angles measured on the face are kept: the image copy keeps its width,
  height and square corners (if it had them).
- The sense and amount: the same as the old texture-space turn gives on a texture with
  equal scale on both axes. For such a texture the new result must equal the old one
  (`map.then(translate(about) · rotate(angle) · translate(-about))`) to 1e-9. One way:
  with `L` the 2x2 matrix of the map's linear part in an orthonormal basis of the face
  plane, apply `A = L · R(σ·angle) · L⁻¹` about `about`, where `σ = sign(det L)`; this
  doesn't depend on which orthonormal basis you take.
- Turning by `+a` then `-a` about the same point gives back the map.
- A map that is already skewed (an imported one) is turned the same way; Reset Texture
  still restores the default.

The turn grip (while held): the tile turns about the image copy's centre so that its top
(the direction from corner 0 to corner 3) points from the centre at the cursor's point on
the face, measured on the face. Its snap compares directions **on the face**: when the
copy's bottom or left edge comes within `SNAP_ANGLE` of being parallel to a face edge,
it lines up exactly. Everything else about the grip (preview, Shift, one undo step
"Position Texture", Esc) stays.

Scale (corner grips and typed) and Move stay as they are. Once turns are rigid, a scale
in texture space stretches along the copy's own edges, so the copy stays a rectangle.

### Press, hold and drag (app wiring, `app.rs`)

While the texture handles show, a primary-button drag works like the two clicks:

- The press grabs what is under it, as a click there would (`Editor::click` at the press
  point). Moving with the button down previews. The release lets go, as a click at the
  release point would. One undo step, the same result as click-move-click to the same
  points.
- If the press is not on a grip or inside the copy, the handles end (as a click away
  does) and the drag then does what the active tool's drag does without handles (a
  selection box for Select).
- Click, release, move, click keeps working.

The Scale tool already uses this pattern (`viewport_input`, `drag_started_by` /
`drag_stopped_by`); follow it.

## Tests

`modeler/tests/editor_texture_turn.rs`: the unstretched turn is unchanged, typed turns
after a stretch keep a rectangle of the same size and turn by the same angle as on an
unstretched texture, the `about` point stays, the turn grip points the copy's top at the
cursor and snaps on the face, a corner scale after a turn keeps a rectangle, `+a`/`-a`
restore the map, and a property test over random typed steps on a floor and a slanted
wall, both sides. The existing texture handle and position tests must stay green.

The drag wiring has no automated test; the orchestrator checks it in the real window
(`gui-session`) when reviewing.
