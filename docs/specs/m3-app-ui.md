# m3-app-ui: window wiring for the M3 tools

Last task of M3. `modeler/src/chrome.rs` has the skeleton (`MenuState`, `TOOL_BUTTONS`, `toolbar`, `menu_bar`); `app.rs` wires everything in. Files you change: `chrome.rs`, `app.rs` (and `editor.rs` only if a getter you need is missing; keep it additive).

## 1. `chrome.rs` (tested with egui_kittest, `tests/chrome.rs`)
Move the toolbar and menu code out of `App` into these two functions (they no longer need `&mut self`); `App` calls them, passing `MenuState` built from the editor (undo/redo labels, `camera.projection == Parallel`, selection non-empty).
- `toolbar`: one selectable button per `TOOL_BUTTONS` entry, in that order, drawn selected when it is `current`; a click returns `Action::SelectTool(tool)`.
- `menu_bar`: three menus.
  - File: `New    Ctrl+N`, `Open...    Ctrl+O`, `Save    Ctrl+S`, `Save As...    Ctrl+Shift+S` (as today).
  - Edit: `Undo <label>    Ctrl+Z` / `Redo <label>    Ctrl+Y` (as today: label only when present, disabled when absent), a separator, then `Flip Along Red`, `Flip Along Green`, `Flip Along Blue` returning `Action::Flip(0|1|2)`; disabled without a selection.
  - View: `Switch to Parallel Projection    Shift+P` (when perspective) or `Switch to Perspective    Shift+P` (when parallel) returning `Action::ToggleProjection`; `Zoom Extents    Shift+Z`; a separator; the seven standard views with their F-keys (`Iso    F2`, `Top    F3`, `Front    F4`, `Right    F5`, `Back    F6`, `Left    F7`, `Bottom    F8`) returning `Action::View`.
  - Choosing an item closes the menu.

## 2. `app.rs` wiring (checked by the orchestrator with `harness/bin/gui-session`, not by unit tests)
- `Effect::Flip(axis)`: call `Editor::flip(axis, model)`, then `refresh_document` as other edits do; do nothing when there is no kernel model.
- `viewport_input`: the new tools (`PushPull`, `Rotate`, `Move`, `Scale`, `Tape`) take the same pointer path as the drawing tools (pointer move, click counts, drag release). Scale also needs left-button press-drag: a press picks up the hovered handle through the editor's existing click/move API, as `m3-app-scale-flip` defines; do not invent new editor behaviour, just feed it events. Tape: holding Ctrl toggles the point-guide flag (`set_point_guides`) while pressed.
- `paint_overlays`, using `overlay.rs` and the editor getters:
  - Push/Pull: `face_highlight` of `pushpull_face()` in the accent colour.
  - Scale: `scale_handles(bounds, hover, grabbed)` when the scale tool has bounds; handles as squares of 7 px, hover and grabbed in brighter colours.
  - Rotate: `protractor` while the tool has a center.
  - Guides: `guides` of the root body, drawn dashed-looking or thin grey lines, points as small crosses.
  - Primitive styles map to colours/widths of your choice; keep them distinct per `Style`. Polygons are closed outlines, markers are 6 px squares or crosses.
- The status bar names the active tool for all new tools (already done for tools with labels in `app.rs`; check none is missing).
- Existing behaviour (Orbit/Pan/Zoom drags, select, shapes, dialogs) must not change.

## Done when
`tests/chrome.rs` passes, the whole workspace is green (fmt, clippy, tests), and the app still builds and starts.
