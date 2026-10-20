# m2-app-editor: select and line tools in the app, without the UI

Module `modeler/src/editor.rs` (skeleton on main) and the key map in `modeler/src/input.rs`. Change only those two files (add private fields, types and helpers; keep public signatures). No new dependencies. Never panic. The window code (`app.rs`) is a later task: it will feed the editor pointer and key events and draw `hint`, `preview`, `selection` and `measurement`. This task is the logic it calls.

Read first: `modeler-kernel` `infer.rs` (`Probe`, `infer`, `pick`), `select.rs` (`box_select`, `bounded`, `connected`, `BoxMode`), `click.rs`, `tools/mod.rs` and `tools/line.rs` (`LineTool`, `Input`, `Target`, `ToolError`), `units.rs` (`format_length`), `model.rs` (`UndoStack`), `modeler-view` (`Camera::project`, `target_plane_point`, `basis`, `eye`).

The editor always works on the model's root body (`BodyId::Root`), in world coordinates. `TOL` = `modeler_kernel::geom::LINEAR_TOL`. Axes are `[X, Y, Z]` (red, green, blue).

## `Viewport`
`size_px` = (w, h). If w or h is not finite or not positive, use w = h = 1. `aspect` = w / h.
- `project(p)`: `camera.project(p, aspect)` gives NDC `n`; the pixel is `((n.x + 1) / 2 · w, (1 − n.y) / 2 · h)`. `None` if `camera.project` is `None` or the pixel is not finite.
- `ray(c)`: NDC `n = (2·c.x / w − 1, 1 − 2·c.y / h)`; `q = camera.target_plane_point(n, aspect)`; `forward` from `camera.basis()`.
  - Perspective: origin = `camera.eye()`, direction = unit `q − eye`.
  - Parallel: direction = `forward`, origin = `q − forward · (1000 · camera.distance)`.
  So `project` of any point on the ray in front of the camera is `c` again.

## `input.rs` additions
`drag` already returns `None` for a left drag with `Select` or `Line` (the app turns it into a box). New keys for `key(name, mods)` (egui key names; plain = no modifier held):

| key | mods | action |
| --- | --- | --- |
| `Space` | plain | `SelectTool(Select)` |
| `L` | plain | `SelectTool(Line)` |
| `Escape` | plain | `Edit(Cancel)` |
| `Enter` | plain | `Edit(Enter)` |
| `Backspace` | plain | `Edit(Backspace)` |
| `Z` | Ctrl | `Edit(Undo)` |
| `Y` | Ctrl | `Edit(Redo)` |
| `Z` | Ctrl+Shift | `Edit(Redo)` |
| `Right` | plain | `Edit(Lock(0))` |
| `Left` | plain | `Edit(Lock(1))` |
| `Up`, `Down` | plain | `Edit(Lock(2))` |

All existing keys keep their meaning. Other modifier sets on these keys give `None`.

## `Editor` state
`new()`: tool `Orbit`, unit `Unit::Inch`, empty selection, empty undo stack, no typed text, no hint, no lock, no message, idle line tool. "Drawing tool" means `Line`. "The anchor" is `LineTool::anchor()` while the tool is `Line`, else none.

`set_tool(t)`: the line tool goes idle (`Cancel`), typed text, both locks, the hint and the message are cleared; the selection and undo stack stay. Setting the current tool does the same.

After every change to the model (draw, undo, redo), the selection keeps only entities that still exist in the root body (`Edge`/`Face` in its geometry, `Instance` in its instances).

## Probes
Every pointer event builds one `infer::Probe` from `view` and the cursor: `cursor`; `origin`/`dir` from `view.ray(cursor)`; `parallel` true for a parallel camera; `project` = `view.project`; `radius` = `SNAP_RADIUS_PX`; `anchor` = the anchor; `axes` = `[X, Y, Z]`; `centers` empty; `lock` = `lock()`.

## Locks
Two locks; `lock()` is the arrow lock if set, else the Shift lock, else `None`.
- Arrow lock: `key(Lock(i))` with `i` in 0..3 while the tool is `Line`: if the arrow lock is axis `i`, clear it; else set it to axis `i` (`lock()` is then `axes[i]`). Other tools, or `i >= 3`: nothing.
- Shift lock, on `pointer_move` with the `Line` tool, before inferring: if `mods.shift` is false, clear it. If `mods.shift` is true, no Shift lock is set, there is an anchor, and there is a hint from before this move with `|hint.point − anchor| > TOL`: set it to `axes[hint.axis]` if `hint.axis` is `Some`, else the unit vector from the anchor to `hint.point`.
- Both locks are cleared by `Cancel`, by `set_tool`, and after a click or Enter that the line tool accepts (`Ok`).

## `pointer_move(model, view, cursor, mods)`
Takes the model mutably only because `LineTool::handle` needs a `Target`; it never changes the model or the undo stack. Don't clone the model. `Line` tool: update the Shift lock, then `infer(root geometry, probe)`. `Some(inf)`: the hint is `inf` and the line tool gets `Input::Move(inf.point)`. `None`: the hint becomes `None`, the tool gets nothing. Other tools: the hint is `None`; nothing else happens.

## `click(model, view, cursor, mods, count)`
Returns true exactly when the click pushed an undo step (compare `undo_count()` before and after).
- `Line`: infer as for `pointer_move` (without changing the Shift lock). `None`: nothing happens (false). `Some(inf)`: the hint is `inf`; send `Input::Click(inf.point)` to the line tool with a `Target` on the root body, this editor's undo stack and `unit`. `Ok`: the message is cleared, both locks are cleared. `Err(e)`: the message is set (see Messages). `count` is ignored.
- `Select`: `hit = infer::pick(root geometry, probe)`. With `ClickMods` from `mods`:
  - `count` 1 (or 0), or `hit` is `None`: `selection = click::apply_click(selection, hit, mods)`.
  - `count` 2: `click::apply(selection, select::bounded(ctx, hit), mods)`.
  - `count` 3 or more: `click::apply(selection, select::connected(ctx, hit), mods)`.
  Returns false.
- `Orbit`, `Pan`, `Zoom`: nothing (false).

## `box_select(model, view, start, end, mods)`
`Select` only (other tools: nothing): `picked = select::box_select(model, BodyId::Root, view.project, start, end, BoxMode::from_drag(start, end))`; `selection = click::apply(selection, picked, mods)`.

## Typed text
- `text(s)`: only while the tool is `Line`; other tools return false and keep nothing. Characters that are control characters are dropped. If the typed text is empty, `s` is taken only when its first non-control character is an ASCII digit, `.`, `-`, `+` or `,`; otherwise return false and keep nothing. If the typed text is not empty, every non-control character of `s` is appended. Returns true when anything was appended. (So `10cm`, `5' 4"`, `1/2` type through once started; the app sends letters to `key` only while `typed()` is empty.)
- `key(Backspace)`: drop the last character (no-op when empty). False.
- `key(Enter)`: empty text: nothing (false). Otherwise send `Input::Typed(text)` to the line tool (tool `Line` only), then clear the text whatever the result. `Ok`: message cleared, both locks cleared. `Err(e)`: message set. Returns true exactly when an undo step was pushed. With another tool the text is just cleared.
- `typed()`: the text.

## Other keys
- `key(Cancel)`: the line tool goes idle, the typed text, both locks and the message are cleared. The hint and the selection stay. False.
- `key(Undo)`: first the line tool goes idle and the typed text is cleared; then `undo_stack.undo(model)`. `Ok(Some(_))`: true (selection pruned, see above). `Ok(None)`: false. `Err`: message set, false.
- `key(Redo)`: the same with `redo`.
- `key(Lock(i))`: see Locks. False.

## Output for the window
- `hint()`: the hint (see above).
- `preview()`: `Line` tool: `LineTool::preview()` as a one-element list, else empty. Other tools: empty.
- `measurement()`: `Line` tool: `Some(("Length", v))`, `v` = `units::format_length(LineTool::length(), unit, 2)` when there is a preview, else the empty string. Other tools: `None`.
- `message()`: set by refused input, as one line of text; tests only check whether it is `Some`. Suggested wording: `Parse`: "Not a length", `NotStarted`: "Click a start point first", `NoDirection`: "Move the cursor to show the direction", `Geometry`/`Model`: "Can't draw that here", undo/redo error: "Can't undo"/"Can't redo".
- `selection()`, `undo_stack()`, `tool()`.

## Tests
`tests/editor.rs` (top view, parallel camera, 800×600 px; points are clicked at `view.project` of world points), `tests/editor_props.rs` (ray/project round trip in both projections; random clicks never panic and keep the model valid), and the key table in `tests/input.rs`.
