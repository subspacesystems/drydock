# m0-app: the viewer window

Milestone M0. Crate `app/crates/modeler`. Three parts; only the first has fixed tests, the others must build cleanly (`cargo clippy -D warnings`) and follow the description.

Allowed dependencies: those already in the crate's `Cargo.toml` (`eframe`, `egui-wgpu`, `wgpu`, `glam`, the workspace crates). If you need `rfd` (MIT/Apache-2.0) for the file dialog, add it to the workspace and the crate; any other new dependency needs a note in your commit message. Check `cargo deny` passes.

Depends on `Document` (`src/document.rs`, task `m0-document`) and `input` (`src/input.rs`), both on main.

## 1. `src/viewstate.rs` (tests: `tests/viewstate.rs`, fixed)
Implement the three `unimplemented!("m0-app")` items; keep public signatures.
- `ViewState::new()`: `Camera::default()`, `Tool::Orbit`, no bounds.
- `scene_bounds(scene)`: `scene.bounds` converted to `modeler_view::Bounds` (f32 to f64), `None` if absent.
- `set_bounds(bounds, viewport_px)`: store; if `Some`, `zoom_extents` the camera with aspect = width / height. A viewport with a zero side counts as aspect 1.
- `apply(action, viewport_px)`:
  - `Orbit` / `Pan` / `Zoom` call `Camera::orbit(dx, dy)` / `pan(dx, dy, height)` / `zoom(steps, cursor_ndc, aspect)`.
  - `ZoomExtents`: `zoom_extents` on the stored bounds; no-op without bounds.
  - `View(v)`: `set_standard_view(v)` (target and distance unchanged).
  - `ToggleProjection`: switch Perspective/Parallel.
  - `SelectTool(t)`: set `tool`.
  - `Open`: return `Effect::OpenDialog`; everything else returns `Effect::None`.
  - A zero-size viewport never produces NaN: use height 1 and aspect 1 in that case.

## 2. Window (`src/app.rs`, `src/main.rs`; no automated tests)
`modeler [file.dae]` starts an eframe app (wgpu backend).
- Central panel is the viewport. Render with `modeler_render::Renderer` offscreen into a `Viewport` sized to the panel in physical pixels (recreate on resize), then show it in egui through an `egui_wgpu` callback or by registering the texture; pick whichever is simpler with eframe 0.36's API. Redraw only when something changed (input, load finished, resize).
- Input: translate egui pointer/scroll/key events with `input::drag/scroll/key` and feed `ViewState::apply`. Left button uses the current tool, middle button works always, wheel zooms toward the cursor. Cursor positions to NDC: x right, y up. Ignore input while a modal dialog is open.
- Open: command-line argument, `Open` action (Ctrl+O, native file dialog filtered to `.dae`), and dropping a file on the window. Loading runs on a background thread via `Document::open`; the UI stays responsive and shows "Loading <name>..." in the status bar. On success: upload the scene, `set_bounds(scene_bounds(..))`, set the window title from `Document::title()`. On error: show the one-line message in the status bar in a red-ish color and keep the previous model. Texture warnings from `GpuScene::warnings` show as a count in the status bar with the lines in a tooltip.
- Status bar (bottom): left, file name or "No model - Ctrl+O to open"; middle, triangles, lines, materials and `format_size`; right, active tool name and "Perspective"/"Parallel".
- Default `Style` from `modeler_render::Style::default()` (if there isn't a Default, use white-ish background `[0.96, 0.96, 0.96, 1]`, default face `[0.8, 0.8, 0.8, 1]`, lines black).
- No names, icons or art from other modeling products. Plain egui widgets only.

## 3. Smoke
Add a hidden flag `--screenshot <out.png>` that loads the file, renders one frame headless via `modeler_render::render_headless` at 1280x800 in the iso view with zoom extents, writes the PNG and exits without opening a window. Document it in `--help` output of the binary only. This is what the orchestrator uses to check the result on a machine without a display.
