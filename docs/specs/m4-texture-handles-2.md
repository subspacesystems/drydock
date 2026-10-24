# m4-texture-handles-2: texture handles, second round

Tyrel's feedback on `m4-app-texture-handles` (docs/specs/m4-app-texture-handles.md, read it
first; this spec changes it where it says so): the help text is hard to read over the model
and belongs in the status bar, the grips are hard to tell apart, scaling should work from any
corner, positions should snap to the face's geometry (to size a texture to fit a surface),
and the turn grip should not sit on a corner.

The skeleton is on main: `Grip::Scale(usize)`, `Grip::cursor`, `SNAP_PX`, `SNAP_ANGLE`,
`TURN_STEM`, `SnapKind`, `turn_grip_point`, `snap_point`, `snap_corner`, `snap_turn`,
`help_text` in `app/crates/modeler/src/texture_handles.rs` (bodies `unimplemented!`), and the
editor still compiles with the old behaviour (corner 2 = Scale, corner 1 = Turn). Tests:
`tests/texture_handles.rs` (pure helpers) and `tests/editor_texture_handles.rs` (editor;
existing tests updated to the new grips).

## Grips

- Corner `i` of `TextureHandles::corners` (0 bottom left, 1 bottom right, 2 top right, 3 top
  left) is the grip `Grip::Scale(i)`. The pin of the old design is gone: dragging corner `i`
  scales the texture about the opposite corner `(i + 2) % 4`, which stays put.
- `Grip::Turn` is at `turn_grip_point(corners)`: the middle of the top edge (corners 2 and 3)
  plus `TURN_STEM` (0.25) of the height vector `corners[3] - corners[0]`. It turns the texture
  about the center of the image copy (texture point `base + size / 2`).
- Grip under the cursor (nothing held): among the four corners and the turn grip, the one
  whose projected position is nearest the cursor and within `GRIP_PX` (ties: lower corner
  index, then Turn). Else `Slide` / none as before.

## Operations (held grip, following `p` = cursor ray on the face plane, `q = m0.apply(p)`)

All as in the old spec except:
- `Scale(i)`: `anchor = base + offset[(i + 2) % 4]`, `target = base + offset[i]`, where
  `offset = [0, (w, 0), (w, h), (0, h)]` is the image copy's corner offsets from `base` (the
  copy's bottom left in texture space). Factor `(target - anchor) / (q - anchor)`
  componentwise; with Shift uniform: `k = |target - anchor|² / ((q - anchor) · (target -
  anchor))`. Op `Scale { factor, about: anchor }`. Invalid when a factor component isn't
  finite or isn't > 0.
- `Turn`: `about = base + size / 2`; the angle is the signed angle from `q - about` to
  `(0, 1)` (so the top of the image points at the cursor); invalid when `|q - about| < 1e-9`.
  Op `Rotate { angle, about }` with the angle passed through `snap_turn`.
- Snapping, using the face's loops (outer loop and holes, open-body points from
  `loop_points`) and `view.project` of `to_world(point)`:
  - `Scale(i)` without Shift: `p` is replaced by `snap_point(...)`'s point for the cursor
    when there is one, before computing `q`. With Shift no snapping.
  - `Slide`: first compute the move `d` as before. The four image copy corners after that
    move are `plane_point_at(map, ..., base + offset[k] - d)`; if `snap_corner` finds a
    corner `k` and face vertex `v` (all loop points), use `d = base + offset[k] -
    map.apply(v)` instead, so that corner lands on `v`.
  - `Turn`: `snap_turn(angle, dirs)` where `dirs` has, for every loop edge `a → b`,
    `map.apply(b) - map.apply(a)`.
  - The outline shown while a grip is held follows the snapped op (it is the same op).

## Pure helpers (see the doc comments in the skeleton for the exact contract)

- `snap_point`: nearest vertex within `SNAP_PX` wins over any edge; else nearest edge point
  (pixels, clamped to the segment, returned at the same parameter along the 3D edge).
- `snap_corner`: closest (corner, vertex) pair within `SNAP_PX`.
- `snap_turn`: candidates are `θ = -φ + k·90°` for each edge direction at angle `φ`; the
  nearest within `SNAP_ANGLE` of `angle` wins.

## Look and feel

- `primitives(handles)`, in this order: the outline `Polygon` (`BoundsBox`); four corner
  `Marker`s at `corners[0..4]` styled by `Grip::Scale(i)` (`HandleGrabbed` when held,
  `HandleHover` when hovered, else `Handle`); a `Line` from the middle of the top edge to
  `turn_grip_point` (`BoundsBox`); a `Marker` at `turn_grip_point` styled by `Grip::Turn`.
  (ADR-005: all grips show together; no modifier changes which are shown.)
- `Grip::cursor`: Slide `Move`, Turn `Grab`, even corners `ResizeNwSe`, odd `ResizeNeSw`.
- `help_text(handles)`: the three exact strings in `tests/texture_handles.rs`, chosen by
  `handles.held` (nothing held ignores hover).

## App wiring (`app.rs`, untested; checked in the real window at review)

- Delete the on-canvas texture-handles text. Show `help_text` in the status bar instead,
  next to the other left-aligned items, while the handles show. Move the paste text
  ("Click to place the paste (Esc cancels)") to the status bar the same way and delete the
  on-canvas one.
- While handles show, set the cursor icon to `held.or(hover).cursor()` over the viewport
  (default cursor when neither).
- Nothing else changes; undo steps, Esc, Enter and ending rules are as before.

## Out of scope

Moving the pin, snapping to anything but the face's own loops, disabling snaps with a
modifier, live texture preview.
