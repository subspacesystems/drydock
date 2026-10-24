# m4-app-texture-handles: position a texture with handles in the viewport

Today a texture is positioned by typing values in the Texture Position panel. This task
adds handles drawn on the face: one copy of the image is outlined, and the user slides it,
turns it or scales it by clicking a grip and clicking again where it should go (the same
click, move, click pattern as Move, Rotate and Scale). Each placement is one
`Editor::position_texture` call, so it is one undo step "Position Texture". The typed
values stay in the panel for exact numbers.

Skeletons on main:
- `app/crates/modeler/src/texture_handles.rs`: `GRIP_PX`, `Grip`, `TextureHandles`,
  `plane_point_at`, `tile_corners`, `primitives`.
- `app/crates/modeler/src/editor.rs`: `Editor::{start_texture_handles,
  end_texture_handles, texture_handles}`, plus the routing below in `click`,
  `pointer_move`, `key`, `text` and the methods that end the handles.
- `app/crates/modeler/src/texture_panel.rs`: `handles_button`.
- `app/crates/modeler/src/app.rs`: wiring (untested, see the end).

Tests: `app/crates/modeler/tests/texture_handles.rs` (geometry and primitives),
`app/crates/modeler/tests/editor_texture_handles.rs` (editor),
`app/crates/modeler/tests/texture_handles_button.rs` (kittest). Read
`docs/specs/m4-app-texture-position.md` (`position_texture`, `TextureOp`) and
`docs/specs/m4-texture-model.md` (`TextureMap`, `side_map`, `Texture::size`) first.

## Geometry (`texture_handles.rs`)

Texture space is meters, as in `TextureMap`; one copy of the image covers
`[0, size.x] × [0, size.y]` and repeats.

- `plane_point_at(map, point, normal, st)`: the point `p` on the plane through `point`
  with normal `normal` with `map.apply(p) == st`. Take any two orthonormal directions
  `e1`, `e2` in the plane; `map.apply(point + a·e1 + b·e2)` is `map.apply(point)` plus a
  2×2 linear function of `(a, b)`; solve it. `None` when that 2×2 determinant has
  `|det| < 1e-12`, the normal is zero, or any input or the result isn't finite.
- `tile_corners(map, size, point, normal, center)`: let `c = map.apply(center)` and
  `A = (floor(c.x / size.x) · size.x, floor(c.y / size.y) · size.y)`. The corners are
  `plane_point_at` of `A`, `A + (size.x, 0)`, `A + size`, `A + (0, size.y)` in that
  order. `None` when `size` isn't finite and > 0 on both axes, or any corner is `None`.
- `primitives(handles)`: exactly four primitives, in this order:
  1. `Polygon { points: corners (all four, in order), style: BoundsBox }`;
  2. `Marker { at: corners[0], style: Handle }` (the pin; never hovered or grabbed);
  3. `Marker { at: corners[1], .. }` for `Turn`;
  4. `Marker { at: corners[2], .. }` for `Scale`.

  A grip marker's style is `HandleGrabbed` when `held` is that grip, else `HandleHover`
  when `hover` is that grip, else `Handle`. (`Slide` has no marker.)

## Editor

The handles are a mode on top of the current tool (like a paste waiting for its click).
The face is in the open body (`self.body(model)`); handle corners are open-body
coordinates, and the cursor ray is `body_ray` (so it works inside an open instance).

### Starting and the current handles

`start_texture_handles(model, face, side)`:
1. The face must exist in the open body and `side` must have a material with a texture
   (the same test as `position_texture`). Otherwise message `That side has no texture`,
   nothing else changes, false.
2. The handles must be computable now (below). Otherwise message
   `Can't position that texture`, false.
3. Success: ends a paste waiting for its click, cancels the current tool's operation in
   progress and typed text (as `set_tool` with the same tool would, but the tool stays),
   clears the message, keeps the selection. Nothing is held or hovered. True. Starting
   again while handles show replaces them (new face/side, nothing held).

Computing the handles: `map = texture::side_map(model, body, face, side)`, `size` is the
side's texture `size`, the plane is `geometry().face_plane(face)`, and `center` is the
mean of the face's outer loop points (`loop_points`). Corners = `tile_corners(map, size,
plane.point, plane.normal, center)`.

`texture_handles(model)`: `None` when the handles aren't showing, or the face or the
side's texture is gone (do not end the mode here; it takes `&self`). Otherwise
`TextureHandles { face, side, corners, hover, held }` where:
- nothing held: corners computed from the model as above;
- a grip held: the corners of the image copy grabbed (the one at grab time, `A` fixed)
  under the preview map, i.e. `plane_point_at(preview, A + offset)` for the four offsets,
  where `preview = m0.then(affine of the last valid op)` (`m0` is the map at grab time;
  before any valid pointer move the op is "no change", so the corners are the grab-time
  corners). The model does not change while a grip is held.

`end_texture_handles()`: handles hidden, a held grip dropped, model untouched. Also ended
by: `set_tool` (any tool), `paste`, `open_instance`, `close_path`, `erase_selection`,
`make_component`, `explode_selection`, Enter (below), and a click that grabs nothing.

### Grips under the cursor

With the handles computed (nothing held), the grip under cursor `c` (pixels):
1. `Scale` if `view.project(world corners[2])` is within `GRIP_PX` of `c`; `Turn` if
   `view.project(world corners[1])` is. If both are within, the nearer wins (Scale on a
   tie).
2. Else `Slide` if the cursor ray meets the face plane at `p` and `map.apply(p)` lies in
   the grabbed cell's closed rectangle `[A, A + size]` (tolerance 1e-9 m).
3. Else none.

World corner = the open body's world transform applied to the corner.

### Following the cursor (held grip)

`p` = where the cursor ray (`body_ray`) meets the face plane (no point when
`|dir · normal| < 1e-12`; the line may meet it behind the eye, that's fine);
`q = m0.apply(p)`. With `A` the pin's texture point and grab-time data:
- `Slide`: `g = m0.apply(p_grab)` (the plane point under the grabbing click). Op
  `Move(g − q)`: the texture point that was under the cursor stays under it.
- `Turn`: `H = A + (size.x, 0)`. Op `Rotate { angle, about: A }` with `angle` the signed
  angle from `q − A` to `H − A` (`atan2(cross, dot)`), so the image's bottom edge points
  at the cursor. Invalid when `|q − A| < 1e-9`.
- `Scale`: `H = A + size`. Op `Scale { factor: (H − A) / (q − A) componentwise,
  about: A }`, so the image's top right corner lands under the cursor. With Shift held
  (`Modifiers::shift`), uniform: `k = |H − A|² / ((q − A) · (H − A))`, factor `(k, k)`.
  Invalid when a factor component isn't finite or isn't > 0.

`pointer_move` while handles show: if a grip is held, recompute the op; a valid op
becomes the preview, an invalid one (or no plane point) leaves the last valid preview.
If nothing is held, update `hover` (the grip under the cursor, or `None`). Either way the
current tool's own pointer handling does not run and `hint()` is `None`.

### Clicks while handles show

These come before the tool and before anything else in `click` (count is ignored).
- The face or side's texture is gone: end the handles, false.
- Nothing held: the grip under the cursor is grabbed (`held`, `hover` cleared; record
  `m0`, `A` and for Slide `p_grab`), false. No grip: end the handles, false.
- A grip held: compute the op for this cursor and modifiers. Invalid op or no plane
  point: message `Can't position that texture`, still held, false. Otherwise
  `position_texture(model, face, side, op)`; the grip is released whatever it returns, the
  handles stay, and the click returns what `position_texture` returned. (A zero move makes
  no undo step, as in `position_texture`.)

### Keys and text

- `Cancel` (Esc): a held grip is dropped (preview gone, model unchanged), handles stay.
  With nothing held, the handles end. Returns false. Nothing else that Esc normally does
  happens while the handles show (no path close, the tool keeps its state).
- `Enter`: ends the handles (dropping a held grip), false; typed text is not used.
- `Undo`/`Redo`: drop a held grip first, then behave as usual; the handles stay and show
  the restored map.
- `text` while handles show: ignored, false.

## Panel button (`texture_panel.rs`)

`handles_button(ui, available, on)`: one `egui::Button`. Its text is
`Position in Viewport` when `on` is false and `Done` when `on` is true. It is enabled when
`on` or `available`. Returns true on the frame it is clicked (never when disabled).

## App wiring (`app.rs`, untested; checked in the real window at review)

- In the Texture Position bottom panel, above `texture_panel`, call
  `handles_button(ui, target.is_some(), on)` with `on = editor.texture_handles(model)
  .is_some()`. Clicked while off: `editor.start_texture_handles(model, face, side)` with
  `texture_target`'s face and side. Clicked while on: `editor.end_texture_handles()`.
- Draw `texture_handles::primitives` through `overlay::transformed(.., open_transform)`
  and `paint_primitives` with the other overlays.
- While `texture_handles` is `Some`, show this text where the paste text goes:
  `Click inside the image, then click again to slide it. Click the turn or scale corner
  the same way (Shift keeps proportions). Esc when done.`
- A placement that changed the model refreshes the document as the panel's ops do.

## Out of scope

Moving the pin, snapping angles or points while dragging, live texture preview on the
face while a grip is held (the outline previews), handles for more than one face.
