# m5-app-sun-windows: the Shadows and Geolocation windows

Milestone M5 (Sun). Crate `modeler`: `src/sun_windows.rs` (skeleton on main, fill in the bodies) and the wiring in `src/app.rs`. `chrome::model_info_window_with` is already on main.

## Goal
The user can open a Shadows window from View > Shadows and a Geolocation window from Model Info, change the model's sun settings, and see the viewport follow.

## `SunWindows`
- `new()`: both windows closed, nothing previewed.
- `apply_shadows_event(event, settings, model, editor) -> bool`:
  - `Preview`: remember `settings` as the previewed sun; record nothing; return false.
  - `Commit`: forget the preview; record `settings` with `Editor::set_shadow_settings` (one undo step, "Shadow Settings"); return true when the model changed, false when `settings` equal the model's (then no step is recorded).
  - `None`: do nothing, return false (a preview in progress stays).
- `sun_light(model)`: while a preview is remembered, `shading::sun_light_for(&model.geolocation(), &preview)`; otherwise `shading::sun_light(model)`.
- `show(ctx, model, editor) -> bool`, true when the model changed:
  - Shadows window, titled "Shadows", while `shadows_open`; a window close button (title bar) or a "Close" button clears it. It holds `shadows_panel` editing a copy of the model's settings; each frame the copy starts as the model's settings (so undo and redo show up) unless a drag is in progress. The panel's event goes to `apply_shadows_event`. Closing the window drops any preview.
  - Geolocation window, titled "Geolocation", while `geo_open`; same ways to close. It holds `geo_panel` on a `GeoForm` made from the model's geolocation when the window opens (and dropped when it closes, so reopening shows the model's current values). When the panel returns `Some(g)`: `Editor::set_geolocation` (one step, "Set Location"), return true, close the window.

## App wiring (`app.rs`)
- `shadows_open` moves into a `SunWindows` field of `App`; View > Shadows toggles `sun_windows.shadows_open` and the menu check mark reads it.
- The Model Info window is drawn with `chrome::model_info_window_with` and a "Geolocation..." button that sets `sun_windows.geo_open`.
- Each frame, with a model and editor: `show` is called; when it returns true the document is marked changed the same way other editor edits are (title star, scene refresh). The style's sun comes from `sun_windows.sun_light(model)` instead of `shading::sun_light`, and a preview change redraws the viewport.
- The windows are closed when there is no model, like Model Info.

## Acceptance
`cargo test -p modeler --test sun_windows`; all other tests unchanged; `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`. The orchestrator checks the real window on 05-house.
