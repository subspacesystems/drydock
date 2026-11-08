# m9-offset-tool: the Offset tool in the app

A new tool, `Tool::Offset`, button label "Offset", key `I`. The user clicks a face, moves the
cursor (or types a length), and a second click or Enter offsets every boundary loop of the face
sideways in its plane: outward grows the face, inward shrinks it. The geometry is the kernel's
`faceoffset::offset_face` (done, task `m9-offset-face`; read the doc comments on `FaceOffset`,
`FaceOffsetError` and `offset_loops` in `faceoffset.rs`). This task is the app side only.
Not the clearance offset of M7 (`clearance_offset.rs`).

Main already has the skeleton: `Tool::Offset` exists and every `match` compiles, with
`unimplemented!("m9-offset-tool")` where behaviour is needed (`editor.rs` click, `chrome.rs`
`tool_shortcut`, `toolbars.rs` `tool_key`, `icons.rs` draw) and `Editor::offset_face` /
`Editor::offset_distance`. Replace those, and add the rest below. Tests:
`tests/editor_offset.rs` (editor, all behaviour) and `tests/offset_ui.rs` (button, key, menu,
icon, tooltip). Existing tests were updated for 17 tools (`chrome.rs`, `eyedropper_tool.rs`,
`icons_2.rs`, `input.rs`, `toolbar_customize_lists.rs`); they must pass too.

## Wiring (one place each)

- `chrome::TOOL_BUTTONS` (17 entries): `(Tool::Offset, "Offset")` right after Extrude.
  `chrome::TOOL_GROUPS`: Offset second in the modifying group, after `Tool::PushPull`.
  `chrome::tool_shortcut(Tool::Offset)` is `"I"`. `icons::TOOLS` follows `TOOL_BUTTONS`.
- `input::key`: plain `"I"` gives `Action::SelectTool(Tool::Offset)`. Shift+I stays unbound.
- `toolbars::tool_key` `"offset"`, `tool_from_key` the reverse. (A toolbar file saved before this
  keeps its buttons; the customize dialog can add Offset. Nothing to migrate.)
- `icons.rs`: draw `Icon::Tool(Tool::Offset)` with the `Draw` helpers, our own drawing, no
  external art: a rounded-square outline with a smaller one inside it, a gap between them, and
  an arrow from the inner one pointing out through the gap. It must differ from every other
  tool icon (`icons_2.rs` checks that) and stay inside the icon rect (`icons.rs` tests).
- `app.rs`: the window must route input to the tool. Add `Tool::Offset` to the `drawing` list in
  the viewport input handler (next to `Tool::PushPull`), and highlight the face as Push/Pull
  does (`editor.pushpull_face()` -> `overlay::face_highlight`): do the same for
  `editor.offset_face()`. The overlay already draws `editor.preview()` segments and the Value
  box already shows `editor.measurement()`; check both work for this tool.
- `editor.rs`: `takes_text` includes `Tool::Offset` (typed length). Esc, Undo, Redo and
  `set_tool` drop an offset in progress, as for Push/Pull. `draw_plane()` stays `None`.

## Picking and the distance

All on the root body of the edit context, like Push/Pull (`self.body(model)`).

- Idle: `pointer_move` records the face under the cursor (`infer::ray_face`, as `move_push`).
  `offset_face()` is that face, or the face being offset while an offset is in progress; `None`
  for other tools and when no face is under the cursor.
- First click on a face (`face_under` gives the face and the hit point): the offset starts.
  Nothing changes in the model (returns false). The nearest edge of any loop of the face to the
  hit point, measured in the face plane, is the **reference edge** (first in loop order on a
  tie). Remember the face, the reference edge's line, and its **outward direction**: the
  in-plane unit vector perpendicular to the edge pointing away from the face's material (for an
  outer loop that is out of the face; for a hole's edge it is into the hole). Derive it from
  the loop direction and the face normal, and check it with the hole test.
- `pointer_move` while offsetting: take the point where the cursor ray meets the face plane. If
  `infer_at` returns an Endpoint or Midpoint snap, use that point's position in the plane
  instead (as `move_push` does for its distance). The distance is `(p - edge_point) .
  outward`: positive outside the reference edge (the face grows), negative inside (it
  shrinks). `offset_distance()` returns it; the hint (`hint()`) is the point `p` with the
  inference's snap, so the usual snap marker shows.
- `preview()`: while offsetting, the segments of `faceoffset::offset_loops(ctx, face,
  distance)`: one segment per consecutive pair of points of each loop, closed. Empty when
  there is no distance yet (zero or tiny) or when `offset_loops` refuses it.
- `measurement()`: label `"Distance"`. Offsetting: the absolute distance, formatted like the
  Push/Pull box (`units::format_length(.., unit, 2)` as the other arms); idle after an offset:
  the last distance, absolute; otherwise the empty string. The box's value is typed through the
  Value box (`units_ui`) like the other tools; nothing new there.

## Commit

- Second click, or Enter with a typed length, calls `faceoffset::offset_face(model, body, face,
  distance, &mut self.undo, "Offset")`: one undo step. On `Ok`: clear the offset state, the
  hint and the message; remember the distance for the box; `self.prune(model)`; return true.
- A distance whose absolute value is `TOL` or less on a click does nothing: returns false, the
  offset stays in progress, no message.
- Typed text: `Enter` takes the typed text; `units::parse_length` in the model's input unit; a
  parse error sets the message `"Not a length"`, returns false, keeps the offset going. With an
  offset in progress, the distance is `|length|` on the side the cursor shows (the sign of the
  current distance; outward when the current distance is zero), flipped when `length` is
  negative, then committed as above. With no offset in progress: message `"Click a face
  first"`, returns false.
- Refusals leave the model and undo stack untouched, keep the offset in progress, return false,
  and set the status-bar message (`message()`):

| Error | Message |
| --- | --- |
| `Collapsed` | "That offset is too big for the face" |
| `LoopsCross` | "The offset edges would cross" |
| `Overlap(_, _)` | "The offset would run into another face" |
| `BadDistance` | no message (it is the zero click above) |
| any other | "Can't offset that" |

  The next successful move or commit clears the message (a successful commit sets none).
- A click that hits no face while idle: nothing, returns false. A click that hits no face while
  offsetting still commits the current distance (the cursor may be anywhere in the plane).

## Order of work

1. Editor behaviour until `tests/editor_offset.rs` passes. Keep the state in an `OffsetState`
   struct (face, reference edge point, outward direction, distance) and a `last_offset`
   distance, next to `PushState`; reset them wherever `push` is reset.
2. Wiring and icon until `tests/offset_ui.rs` and the updated tests pass.
3. Guide: add a `## Offset` section to the end of `docs/guide/extrude.md` (no new page: the
   guide tests pin the page list) in the guide's plain voice: press `I`, click a face, move the
   cursor toward the side you want or type a length and press `Enter`; outward grows, inward
   shrinks, holes move the other way; a distance that is too big, or that would run into another
   face, is refused with a message; `Esc` abandons it; one undo takes it back. Never say
   "push/pull" or "print" (tests scan for both). Add the row `| Offset | \`I\` |` to the table in
   `docs/guide/shortcuts.md` next to Extrude. `guide_coverage.rs` checks both.

Run `harness/bin/run-tests -p drydock` at the end; the window tests need Xvfb and
`DRYDOCK_GUI_TESTS=1` as usual.

## Where to look

- `Tool`, `EditKey`, `key` in `input.rs`; `TOOL_BUTTONS`, `TOOL_GROUPS`, `tool_shortcut` in
  `chrome.rs`; `tool_key`, `tool_from_key` in `toolbars.rs`; `TOOLS`, the `Draw` helpers and the
  `Icon::Tool(Tool::PushPull)` / `Tape` arms in `icons.rs`.
- Push/Pull is the pattern: `PushState`, `click_push`, `move_push`, `enter_push`,
  `commit_push`, `pushpull_face`, `pushpull_distance`, `face_under`, `body_ray`, `closest_on_line`,
  `infer_at`, `idle_*` and the `EditKey::Cancel/Undo/Redo` arms in `editor.rs` (`show <name>`).
- Kernel: `faceoffset::offset_face`, `offset_loops`, `FaceOffsetError` (`show offset_loops`);
  `Context::face`, `loop_points`, `face_plane`; `infer::ray_face`.
- `app.rs`: the `drawing` `matches!` list near `editor.set_point_guides`, the `pushpull_face`
  highlight, the status label match (`Tool::Offset => "Offset"` is already there).
- Tests: `World` in `tests/editor_offset.rs` (iso camera, `draw`, `ring`), `toolbar_harness`
  in `tests/chrome.rs`.
