# m5-geo-globe: pick the place on a globe

Milestone M5 (Sun), follow-up from the first try. Crate `modeler`: `src/globe.rs`, `src/geo_panel.rs`, `src/sun_windows.rs`.

## Goal
The Geolocation window shows a small globe above the typed fields. Dragging turns it; clicking a place fills in its latitude and longitude (and a guessed UTC offset). Nothing is applied until the user presses Apply (or Enter in a field), as today.

## Already on main
- `src/globe.rs` with stubs: `GlobeView { latitude, longitude }` and its methods, `wrap_longitude`, `land`, `utc_offset_estimate`, `globe`. `LAND_110M` embeds `assets/land_110m.bin` (Natural Earth 1:110m land, public domain; ADR-006).
- Asset format (little-endian): u16 ring count; per ring a u16 point count, then that many (i16 longitude, i16 latitude) pairs in hundredths of a degree. Rings are not closed (draw back to the first point yourself).
- `geo_panel::geo_panel_with_globe(ui, form, view)` stub. `geo_panel(ui, form)` stays as it is.

## `globe.rs`
- `GlobeView::facing(lat, lon)`: latitude clamped to -90..=90, longitude through `wrap_longitude`.
- `wrap_longitude`: into -180..=180 (180 stays 180, 181 becomes -179, -181 becomes 179).
- `project(lat, lon)`: orthographic view of a unit sphere seen from straight above the place the view faces. Disk coordinates, radius 1, x right, y up: from the faced place, east is right and north is up. With `(φ0, λ0)` the faced place: `x = cos φ sin(λ-λ0)`, `y = cos φ0 sin φ - sin φ0 cos φ cos(λ-λ0)`; `None` when `sin φ0 sin φ + cos φ0 cos φ cos(λ-λ0) < 0` (far side).
- `unproject(p)`: the inverse for `|p| <= 1` (center gives the faced place exactly); `None` outside the disk. Longitude in -180..=180.
- `turn(delta)`: a drag of `delta` disk units (x right, y up) moves the surface with the pointer: the faced longitude decreases by `delta.x` radians (as degrees), the faced latitude decreases by `delta.y` radians; latitude stops at ±90, longitude wraps.
- `land()`: decode the asset once (e.g. `OnceLock`) into rings of (latitude, longitude) degrees.
- `utc_offset_estimate(lon)`: nearest whole hour of `lon / 15` (-180 gives -12, 180 gives 12); never `-0`.
- `globe(ui, view, marker, size)`: allocate a `size` x `size` square that senses clicks and drags; give it accessible info labelled `"Globe"` (e.g. `response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Globe"))`) so tests and screen readers find it. The disk fills the square (radius `size / 2`, centered). Draw: the disk in a sea color, a faint latitude/longitude grid every 30 degrees, land outlines (lines; split a ring where it passes to the far side so nothing draws across the disk), a thin limb circle, and `marker` as a small red dot with a light outline when it is on the near side. A drag turns `view` by the drag in disk units (screen y is down: flip it). A click (press and release without a drag) inside the disk returns the place under the pointer; outside the disk, or a drag, returns `None`. Optional: while hovering the disk, show the place under the pointer (e.g. "47.61° N, 122.33° W") as hover text.

## `geo_panel_with_globe(ui, form, view)`
- Draws the globe (about 220 points; at least 150) above the fields, then `geo_panel(ui, form)` and returns what it returns.
- The marker is the form's location when the form parses (`parse_geolocation`), else none.
- A click on a place sets `form.latitude` and `form.longitude` to its numbers rounded to 4 decimals (about 10 m), printed with `{}` (no `-0`), and `form.utc_offset` to `utc_offset_estimate(longitude)` printed with `{}`. The north angle is untouched. This works even when the form had bad text. Nothing is applied by a click.
- A small hint line under the globe is welcome: "Drag to turn, click to pick. UTC offset is estimated from longitude."

## `SunWindows`
- Keep a `GlobeView` next to the form: when the Geolocation window opens (form created), the globe faces the model's geolocation; dragging persists while the window is open; closing the window (Close, the title-bar x, or applying) forgets it, so reopening faces the current place again. `close()` forgets it too.
- Call `geo_panel_with_globe` instead of `geo_panel` in the window. Everything else unchanged (`tests/sun_windows.rs` keeps passing).

## Acceptance
- `cargo test -p modeler --test globe` (math, land data, property tests) and `--test geo_globe` (egui_kittest: globe above the fields, click picks the middle / an off-center place / nothing outside the disk, drag turns without picking, picked place applies, works with bad text, the app window faces the model's place and again after reopening).
- `tests/geo_panel.rs`, `tests/sun_windows.rs` unchanged and green; `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
- Reviewer checks in the real window (`gui-session`): Model Info > Geolocation shows the globe with recognizable continents; drag turns; click moves the marker and fills the fields.
