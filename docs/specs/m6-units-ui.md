# m6-units-ui: the model's units in Model Info and everywhere lengths show

Milestone M6 (UI), feedback round 4. Crate `modeler`: `src/units_ui.rs`, `src/editor.rs`, `src/chrome.rs`, `src/entity_info.rs`, `src/texture_panel.rs`, `src/materials_panel.rs`, `src/app.rs`.

## Goal
`m6-units` gave each model a length format (`LengthFormat`, `Model::length_format`, `format_with`, `format_area_with`). This task lets the user choose it in Model Info and makes every length and area in the app use it. Some places show inches now and others meters; after this task they all use the model's format.

## Already on main (skeleton)
- `src/units_ui.rs`: `UNIT_NAMES` and stubs for `unit_index`, `pick_unit`, `precisions`, `precision_label`, `unit_abbrev`, `units_section`, `length_drag`. Doc comments are part of the contract.
- `Editor::set_length_format` stub. The editor already reads typed numbers in `model.length_format().input_unit()`, and the measurement box already writes with `format_with`.
- `chrome::model_info_lines(stats, kernel, format)`, `texture_panel(ui, form, available, unit)` and `entity_info_panel` already receive the format or unit. Their bodies still write the old way.

## Rules
### Helpers (`units_ui.rs`)
- `unit_index`: row of `UNIT_NAMES`, in order: Millimeters, Centimeters, Meters, Inches, Feet (decimal feet), Architectural.
- `pick_unit(format, index)`:
  - A decimal row keeps `format`'s decimal places.
  - Architectural picked from a decimal unit starts at 1/16".
  - A decimal row picked from Architectural starts at 2 places.
  - The row already shown, or an index past the list, returns `format` unchanged.
- `precisions`: for a decimal unit, that unit with 0 to `MAX_DECIMALS` places; for Architectural, each of `DENOMINATORS` in order.
- `precision_label`: `0`, `0.0`, `0.00` ... (as many zeros after the point as places); `1"` for denominator 1, else `1/N"`.
- `unit_abbrev`: `mm`, `cm`, `m`, `in`, `ft`.
- `length_drag(meters, unit)`: an `egui::DragValue` that shows and accepts the value in `unit` and keeps `*meters` in meters (`kernel::units::meters_per`; `DragValue::from_get_set` works).

### The Units section
- `units_section(ui, format)`: a strong "Units" heading, then two rows, each a field label (`dialog::field_label`, so "Unit:" and "Precision:") and a combo box labelled by it (`labelled_by`). The closed box shows `UNIT_NAMES[unit_index]` and `precision_label(format)`. The lists show every unit row and every `precisions(format)` row, with the current one selected.
- It returns `Some(new)` only on the frame the user picks a row that gives a format other than `format` (unit rows go through `pick_unit`). Picking the row already shown returns `None`.

### Editor
- `set_length_format(model, format)`: apply it with `Model::set_length_format` into one undo step labelled **"Set Units"** and clear the message. Return true. When the format is already current, also return true but push no undo step. An invalid format: return false, set a message, change nothing.

### Where lengths show
- **Model Info** (`model_info_lines`): `Size: A x B x C`, each written with `format_with` in the model's format (e.g. `Size: 13' 6" x 22' 0" x 8' 0"`). `document::format_size` is no longer used there.
- **Model Info window** (`app.rs`): for a kernel model, the Units section goes between the info lines and the Geolocation... button, with a separator above and below. A pick goes through `Editor::set_length_format`. Models without a kernel model (view-only imports) don't show the section.
- **Entity Info**: lengths (Length, Width, Depth, Height) with `format_with`, Area with `format_area_with`, in the model's format.
- **Texture Position panel**: "Move X (u):" and "Move Y (u):" with `u` = `unit_abbrev(unit)`, values through `length_drag`, so the form and the `TextureOp::Move` it reports stay in meters. The other labels get a colon: "Turn (deg):", "Scale X:", "Scale Y:". The app passes `model.length_format().input_unit()`.
- **Material form** (New... / Edit... in Materials): "Width (u):" and "Height (u):" with the model's `input_unit()` (inches for Architectural), values through `length_drag`; the material's texture size stays in meters. Each label is the drag value's `labelled_by` label.

## Notes
- A throwaway implementation that passes the tests changed about 200 lines. The units section, model info lines and set_length_format took about 120 of them. I checked it in the real window: Inches then Architectural 1/16" changed the size line to `7 1/8" x 2 1/2" x 1' 1 7/16"`, and Ctrl+Z undid it.
- The spec commit updates `status.rs` (sizes now `4.115m`, the meters format given explicitly), `texture_panel.rs` (labels with colons) and `materials_panel.rs` (the default model is in inches: "Width (in):").

## Acceptance
- `cargo test -p modeler --test units_ui`, plus `status`, `texture_panel`, `materials_panel` as changed in the spec commit.
- All other tests unchanged and green, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`.
