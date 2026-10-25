# m5-app-sun: the model's sun in the viewport

Milestone M5 (Sun). Crate `modeler`: `src/shading.rs` (skeleton on main), `src/editor.rs` (`set_geolocation`, `set_shadow_settings`), `src/app.rs`, `src/main.rs`. Builds on `m5-solar` (`modeler_kernel::sun::sun_direction`), `m5-geo-model` (`Model::geolocation`, `shadow_settings`, `set_geolocation`, `set_shadow_settings`) and the renderer's `Style::sun` / `SunLight` (`m5-render-sun`, `m5-render-shadows`).

## Goal
The viewport and `--screenshot` draw with the sun the model's settings ask for, and the editor changes those settings as undoable steps.

## `shading`
- `sun_light_for(geolocation, settings)`: `None` when both `settings.shadows` and `settings.sun_shading` are false. Otherwise `Some(SunLight)` with `direction` exactly `sun_direction(geolocation, &settings.time)`, `light` and `dark` the settings' values as `f32`, `shadows` = `settings.shadows`, and `on_ground` = `settings.shadows && settings.on_ground`.
- `sun_light(model)`: `sun_light_for` of the model's geolocation and shadow settings.
- `update_style(style, sun)`: sets `style.sun = sun`; returns true when the value changed, false when it was already equal. Other fields of `style` are untouched.

## Editor
- `set_geolocation(model, g)` and `set_shadow_settings(model, s)` call the kernel setters and push what they record as one undo step, labelled "Set Location" and "Shadow Settings". When the value equals the model's, no step is pushed and the result is true. When the kernel refuses (`BadSettings`), nothing changes, no step is pushed, the result is false and `message()` says why ("Can't use that location" / "Can't use those shadow settings"). An accepted call clears the message.
- Undo and redo (`key(EditKey::Undo | Redo, model)`) take these steps back and forth like any other.

## App
- Every frame, before the viewport renders, the app passes `sun_light` of the open model to `update_style` on its `Style`; when it returns true the viewport redraws (these settings do not change the model's revision, so the scene is not rebuilt and the existing dirty check does not see them). Without a kernel model, `sun` is `None`.
- `--screenshot` renders with the model's sun too (`Style::default()` plus `sun_light` of the document's kernel model, `None` without one).

## Acceptance
`cargo test -p modeler --test app_sun` (includes two `--screenshot` runs on lavapipe), all other tests unchanged, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`.
