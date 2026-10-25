# m5-geo-model: geolocation and shadow settings in the model

Milestone M5 (Sun). Kernel crate `modeler-kernel`: `src/model.rs`, `src/format.rs` (skeleton on main). File format: `docs/format.md` (version 4 is already described there).

## Goal
A model knows where it is on Earth (`sun::Geolocation`) and how the sun lights it (`sun::ShadowSettings`). Both are changed as undoable records, compared by model equality, and saved in the native file. They are settings, not geometry: they don't change `Model::revision()`, which keys geometry caches.

## Model (`src/model.rs`)
The fields `geolocation` and `shadows`, the getters, the record variants `ModelRecord::SetGeolocation { from, to }` / `SetShadows { from, to }` (with `inverse`) and `ModelError::BadSettings` are on main. Implement:

- Valid values. Geolocation: every field finite; latitude in −90..=90, longitude in −180..=180, north angle in −360..=360, UTC offset in −12..=14 (all inclusive). Shadow settings: `time.is_valid()`, `light` and `dark` finite and in 0..=1.
- `set_geolocation(g, log)` / `set_shadow_settings(s, log)`: if the value isn't valid, return `Err(BadSettings)` and change nothing (no record). If it equals the current value, return `Ok(())` and push no record. Otherwise apply one record (`from` = current value, `to` = new) and push it to `log`.
- `apply` of `SetGeolocation`/`SetShadows`: `BadSettings` if `to` isn't valid; `Mismatch` if `from` isn't the current value; else store `to`. On error nothing changes (so `apply_edit` rolls back as for other records).
- These two records don't change `revision()` (applying, undoing or redoing them). Every other record still does.
- `PartialEq for Model` also compares the geolocation and the shadow settings. Update the doc comment on `Model`.
- Nothing else carries them: a clipboard model, a new model and DAE import have the defaults.

## Native file (`src/format.rs`)
- `FORMAT_VERSION` becomes 4.
- Writer: version 4 when the geolocation or the shadow settings differ from their defaults (otherwise the version rules are unchanged, so existing files and tests keep their versions). Right after the header write `geolocation <latitude> <longitude> <north angle> <utc offset>` when the geolocation isn't the default, then `sun <year> <month> <day> <hour> <minute> <shadows> <sun shading> <light> <dark> <on ground>` when the shadow settings aren't the default. Reals print with the same shortest round-trip form as other numbers (`f64` `Display`); flags are `on` or `off`. Non-finite values give `WriteError::NonFinite`.
- Reader, version 4 only (in 1-3 both names are `UnknownRecord`): each record may appear anywhere before `end`, at most once (a second one of the same name is `BadField`). Wrong token count, a malformed or non-finite real, a non-integer or negative month/day/hour/minute, a year that isn't an `i32`, or a flag other than `on`/`off` is `BadField`. Apply the value through `Model::apply` (`from` = the current value), so out-of-range values come back as `Rejected(BadSettings)`.
- Round trip is exact: every `f64` reads back bit for bit and `to_text(from_text(t)) == t` for writer output.

## Rules
- Don't change public signatures that exist on main; add private helpers as needed.
- The one old test that pinned `FORMAT_VERSION` to 3 already accepts 4 on main.

## Acceptance
`cargo test -p modeler-kernel --test geo_model`, plus every existing kernel test (format, undo, copy and texture tests must stay green), `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
