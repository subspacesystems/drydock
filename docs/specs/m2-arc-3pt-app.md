# m2-arc-3pt-app: three-point arc in the editor and window

The kernel tool `Arc3Tool` (`m2-arc-3pt`, `modeler-kernel/src/tools/arc3.rs`) draws an arc from a start, an end and a point it passes through; typed: chord length, then bulge height. This task makes it a tool in the app, next to the existing center/start/end `Arc`. Tests: `modeler/tests/editor_arc3.rs`; `modeler/tests/input_shapes.rs` now allows Shift+A.

Change only `modeler/src/input.rs`, `modeler/src/editor.rs`, `modeler/src/app.rs`. No new dependencies. Do not change `Arc3Tool`.

## `input.rs`
- New `Tool::Arc3` (after `Arc`; doc comment: three-point arc, this spec).
- `key("A", Shift only)` → `Action::SelectTool(Tool::Arc3)`. Plain `A` stays `Tool::Arc`; `A` with any other modifiers stays `None`.
- `drag`: `Arc3` leaves left drags to the editor (`None`), like the other shape tools; middle drag orbits as before.

## `editor.rs`
`Arc3` is a shape tool, exactly like `Rectangle`, `Circle` and `Arc`: everything that treats shape tools alike (`is_shape`, the draw plane from the face under the cursor, plane lock with the arrow keys while idle, `shape_input_point`, `lock()` is `None`, typed text, Esc, tool change and undo dropping the shape in progress, messages from `error_message`) covers it too. Add an `arc3: Arc3Tool` field, started on the ground plane (`plane_for_normal(Z)`), with its own segment count (it is separate from `Arc`'s and survives switching tools).
- `preview()`: `arc3.preview()` points as consecutive pairs `(p[i], p[i+1])` (one pair while only the start is placed, the arc's segments after that).
- `measurement()`:
  - nothing placed: `("Segments", segments)`;
  - start placed: `("Length", chord)`, the chord formatted like other lengths, `""` without one;
  - start and end placed: `("Bulge", bulge)`, `""` when `radius()` or `preview()` is `None`.
  - Bulge: with `r = radius()`, `c` = chord, `h0 = sqrt(max(r² − c²/4, 0))`: `r − h0` when the arc is at most half a turn, else `r + h0`. The arc is more than half a turn when the preview point at index `len / 2` lies farther than `r` from the line through the start and end. (It is the height of the arc's highest point above the chord, the value a typed bulge sets.)
- `draw_plane()`, `shape_anchor`, `shape_plane`, `set_shape_plane`, `handle_shape` dispatch to `arc3` for `Arc3`.

## `app.rs` (window; not unit tested)
- Toolbar button `3-Point Arc` right after `Arc`; status bar tool name `3-Point Arc`.
- Every place that lists the drawing tools (left drags go to the editor, hint/preview drawing, the preview tint by draw plane) includes `Arc3`.

I check the window with `harness/bin/gui-session` (toolbar button, Shift+A, three clicks, typed chord and bulge).
