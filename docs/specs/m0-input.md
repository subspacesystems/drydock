# m0-input: mouse and keyboard to viewport actions

Milestone M0. Crate `app/crates/modeler`, file `src/input.rs`. Implement `drag`, `scroll` and `key`. They're pure functions: no egui, no camera, no state. Don't change the public types, signatures or `ZOOM_DRAG_PIXELS_PER_STEP`; add private helpers freely. No new dependencies.

Conventions: screen pixels with y down; `delta` is in pixels; NDC is x right, y up, -1..1. The window code (a later task) feeds these functions egui input and applies the `Action` to the camera (`modeler_view::Camera::orbit/pan/zoom`, which take the same pixel deltas).

## `drag(tool, button, mods, delta, start_ndc)`

A zero `delta` gives `None`. Otherwise:

| Button | Condition | Action |
| --- | --- | --- |
| Middle | Shift held | `Pan { dx, dy }` |
| Middle | otherwise | `Orbit { dx, dy }` |
| Left | tool `Orbit`, Shift held | `Pan { dx, dy }` |
| Left | tool `Orbit` | `Orbit { dx, dy }` |
| Left | tool `Pan` | `Pan { dx, dy }` |
| Left | tool `Zoom` | `Zoom { steps: -delta.y / ZOOM_DRAG_PIXELS_PER_STEP, cursor_ndc: start_ndc }` |
| Right | any | `None` |

`dx`, `dy` are `delta.x`, `delta.y` unchanged. The middle button ignores the tool. Ctrl and Alt don't change the result. With the Zoom tool, dragging up (negative `delta.y`) zooms in; horizontal movement alone gives `Zoom` with 0 steps (it's not a zero delta).

## `scroll(lines, cursor_ndc)`

`Zoom { steps: lines, cursor_ndc }`. Positive lines (wheel away from the user) zoom in.

## `key(key, mods)`

`key` is the egui key name: upper-case letters (`"O"`), `"F2"` etc. Match the key and the exact modifier set; anything not in the table is `None` (including lower-case names, and a listed key with extra modifiers).

| Key | Modifiers | Action |
| --- | --- | --- |
| `O` | none | `SelectTool(Orbit)` |
| `H` | none | `SelectTool(Pan)` |
| `Z` | none | `SelectTool(Zoom)` |
| `Z` | Shift | `ZoomExtents` |
| `P` | none | `ToggleProjection` |
| `O` | Ctrl | `Open` |
| `F2` | none | `View(Iso)` |
| `F3` | none | `View(Top)` |
| `F4` | none | `View(Front)` |
| `F5` | none | `View(Right)` |
| `F6` | none | `View(Back)` |
| `F7` | none | `View(Left)` |
| `F8` | none | `View(Bottom)` |

## Tests (fixed; don't edit)
`tests/input.rs`. Run with `cargo test -p modeler --test input`.

## Out of scope
Applying actions to the camera, egui event handling, cursor icons, the window (task `m0-app`).
