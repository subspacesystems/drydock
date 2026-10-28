# m6-units: the model's length format (kernel)

Milestone M6 (UI), feedback round 4. Crate `modeler-kernel`: `src/units.rs`, `src/model.rs`, `src/format.rs`.

## Goal
Lengths show in inches in some places and meters in others. Each model gets a units setting, and every length and area in the app will show in it. That last part is the follow-up task `m6-units-ui`. This task is the kernel half: the setting's type, writing lengths and areas in it, storing it in the model (undoable), and saving it in the native file.

The user's choices: millimeters, centimeters, meters, inches or feet, each with a number of decimal places; or architectural (feet and inches with fractions), with the fraction the inches round to.

## Already on main (skeleton)
In `units.rs`: `LengthFormat` (`Decimal { unit, decimals }`, `Architectural { denominator }`), `Default` (inches, 2 places), `MAX_DECIMALS` (6), `DENOMINATORS` (1, 2, 4, 8, 16, 32, 64), and stubs for `is_valid`, `input_unit`, `format_with` and `format_area_with`. In `model.rs`: stubs for `Model::length_format` and `Model::set_length_format`. Doc comments are part of the contract.

## Rules
### `LengthFormat`
- `is_valid`: `decimals <= MAX_DECIMALS`; `denominator` in `DENOMINATORS`.
- `input_unit`: a bare typed number is read in the decimal unit; for `Architectural`, in inches.

### `format_with(meters, format)`
- A non-finite length is written as zero.
- `Decimal` in mm, cm, m or in: exactly what the existing `format_length(meters, unit, decimals)` writes (round to `decimals` places, drop trailing zeros and a trailing point, suffix `mm`, `cm`, `m` or `"`, no space, `-` only when the rounded value isn't zero). Call it.
- `Decimal` in feet: **decimal feet**, not feet and inches: the same rounding and trimming in feet with the suffix `'` (`5.25'`, `5'`, `0'`, `-2.5'`). (`format_length` with `Unit::Foot` writes feet and inches; leave it alone.)
- `Architectural { denominator }`: round the absolute length to the nearest 1/`denominator` inch (in integer steps, so it never shows 12 inches or a fraction of 16/16), then split it into feet, whole inches and a fraction reduced to lowest terms.
  - Feet shown (one foot or more): `F' I"`, `F' I N/D"`. The inches always show, even 0: `5' 0"`, `5' 4 1/2"`, `1' 0 3/4"`.
  - Under a foot: `I"`, `I N/D"`, or `N/D"` when there are no whole inches: `4"`, `4 1/2"`, `3/16"`, `0"`.
  - A leading `-` when the length is negative and doesn't round to zero (`-2' 1"`, `-1/2"`).
- Everything written reads back with `parse_length(text, format.input_unit())` to within half a rounding step, and writing that again gives the same text (the property tests check both).

### `format_area_with(square_meters, format)`
- `Decimal`: the area in the square of the unit, rounded to `decimals` places and trimmed like lengths, suffix `mm²`, `cm²`, `m²`, `in²` or `ft²`, no space (`1.5m²`, the same as `entity_info::format_area` today).
- `Architectural`: square feet to 2 places (`20ft²`, `0.33ft²`).
- A non-finite area is written as zero.

### Model
- A `length_format` field, default `LengthFormat::default()`, part of `Model` equality (as for geolocation).
- `ModelRecord::SetLengthFormat { from, to }` with the inverse swapping them. `apply` refuses a `to` that isn't valid (`BadSettings`) and a `from` other than the current format (`Mismatch`). Like the geolocation and shadow records, it does **not** bump `revision()`.
- `set_length_format(format, log)`: `BadSettings` for an invalid format (nothing changes, nothing logged). Nothing is logged when the format is already current. Otherwise apply one `SetLengthFormat` record and push it to `log`.

### Native file (version 5)
- `FORMAT_VERSION` becomes 5. The writer's header is the lowest version that holds the model: **5** when the length format differs from the default, else the rules as now.
- Record, written right after `geolocation` and `sun` and only when the format isn't the default: `units decimal <unit> <decimals>`, with `<unit>` one of `mm cm m in ft`, or `units architectural <denominator>`.
- Reader: a `units` record in a file older than version 5 is an `UnknownRecord`. A second one is a `BadField`. A wrong token count, an unknown kind or unit, or a number that doesn't parse as an unsigned integer is a `BadField`. A well-formed but invalid value (`units decimal m 7`, `units architectural 3`) is `Rejected(BadSettings)`. Apply it like the geolocation record. A file with no `units` record reads with the default format.
- I have updated `docs/format.md` on main to describe version 5.

## Notes
- A throwaway implementation that passes the tests took about 160 lines (units 80, model 40, format 45).
- `m6-units-ui` (written after this merges) will add the Model Info "Units" section and switch the app's displays and the measurement box to `format_with` and `format_area_with`.

## Acceptance
- `cargo test -p modeler-kernel --test units_model`.
- `geo_model.rs`: `version_is_four` becomes `version_is_at_least_four` (changed in the spec commit).
- Other tests unchanged and green (`units`, `units_props`, `format*`, `geo_model`), plus `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and all workspace tests.
