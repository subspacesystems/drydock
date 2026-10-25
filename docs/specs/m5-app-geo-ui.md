# m5-app-geo-ui: the Geolocation form

Milestone M5 (Sun). Crate `modeler`: `src/geo_panel.rs` (skeleton on main, fill in the bodies). The app puts it into the Model Info area in a later task; don't touch `app.rs`.

## Goal
A form where the user types latitude, longitude, north angle and UTC offset, sees what is wrong with them, and applies them as a `Geolocation` (`modeler_kernel::sun`).

## Form
- `GeoForm::from_geolocation(g)`: each field is the number printed with `{}` (45.0 reads "45", -122.6765 reads "-122.6765"). `Default` is the form of `Geolocation::default()`.
- `parse_geolocation(form)`: trims each field and parses it as `f64`; it must be finite and in range: latitude -90..=90, longitude -180..=180, north angle -360..=360, UTC offset -12..=14 (the same ranges the model accepts). Otherwise the error names the first bad field in that order, as `"<Name> must be a number from <lo> to <hi>"` with names "Latitude", "Longitude", "North angle", "UTC offset" and the bounds printed with `{}` (so "-90 to 90"). Empty text, "NaN", "inf", "1e999", and decimal commas are all errors.

## Panel
`geo_panel(ui, form) -> Option<Geolocation>`. Top to bottom:
- heading "Geolocation";
- four rows, each a label and a single-line text field, labels "Latitude (deg)", "Longitude (deg)", "North angle (deg)", "UTC offset (h)". Link each field to its label (`Response::labelled_by`) so it can be found by that label;
- when the form doesn't parse, the message from `parse_geolocation` in red;
- a button "Apply", disabled while the form doesn't parse.
Returns `Some` on the frame the button is clicked, or Enter is pressed in a field, with a valid form; never otherwise. The form text isn't rewritten by the panel.

## Acceptance
`cargo test -p modeler --test geo_panel`; all other tests unchanged; `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`.
