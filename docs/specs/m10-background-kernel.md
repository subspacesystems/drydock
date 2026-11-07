# m10-background-kernel: the model's background, sky and ground

A model carries its own background settings, so the viewport no longer depends on the
OS theme for them: a plain background color, a sky (on or off, with a color) and a
ground (on or off, with a color). This task is the kernel part only: the value, setting
it as one undoable record, and saving it in the native file (version 8). Drawing the sky
and ground and the Model Info section come in later tasks.

Crate: `drydock-kernel`. Acceptance tests: `tests/background.rs`; `tests/tags_format.rs`
no longer pins the version to 7.

## The value (`src/background.rs`, skeleton on main)

`Background { color, sky, sky_color, ground, ground_color }`: three sRGB colors as
`[f32; 3]` with channels 0 to 1 (like `Material::color` without alpha) and two flags.
`color` is shown wherever neither the sky nor the ground is drawn.

The defaults are already written as constants and `Default`:

| | value | constant |
| --- | --- | --- |
| background | `#F4F8FC` (light blue-grey) | `DEFAULT_BACKGROUND` |
| sky | on, `#A8C8E8` (light blue) | `DEFAULT_SKY` |
| ground | on, `#9A9A96` (neutral grey) | `DEFAULT_GROUND` |

Each constant is the 8-bit value divided by 255.0 (`168.0 / 255.0`, ...). Don't change
them.

`Background::is_valid`: true when all nine channels are finite and in `0.0..=1.0`
(`-0.0` counts as 0). The flags don't matter: a color that isn't drawn must still be valid.

## The model setting (`src/model.rs`)

Follow the length format (`Model::length_format`, `Model::set_length_format`,
`ModelRecord::SetLengthFormat`) exactly:

- `Model` gets a `background: Background` field. A new model (and `Model::default()`) has
  `Background::default()`. Model equality (`impl PartialEq for Model`) compares it too.
- `Model::background()` returns it.
- `ModelRecord::SetBackground { from, to }` (variant already on main): the inverse swaps
  `from` and `to`. `apply` refuses a `to` that isn't valid with `BadSettings` and a `from`
  other than the current value with `Mismatch`; on error nothing changes.
- Like the geolocation, shadow and length-format records, `SetBackground` does **not**
  change `revision()` (applying, undoing or redoing): the revision keys geometry caches,
  and the background is a setting. Add it to the list in `Model::apply`.
- `Model::set_background(background, log)`: `BadSettings` and nothing recorded when the
  value isn't valid; `Ok(())` and nothing recorded when it equals the current value; else
  one `SetBackground` record applied and pushed onto `log`.

Copying to the clipboard, making components and importing components don't copy the
background: it belongs to the model, not to any body. Nothing to do for that.

## The native file (`src/format.rs`, `docs/format.md`)

`docs/format.md` on main already describes version 8; implement what it says:

- `FORMAT_VERSION` becomes 8.
- The record: `background <r> <g> <b> <sky> <r> <g> <b> <ground> <r> <g> <b>`, exactly 12
  tokens: the background color, `on`/`off` and the sky color, `on`/`off` and the ground
  color. Numbers are written with `{}` (Rust's shortest form of the `f32`, which reads
  back bit for bit), like the `material` record's color; read with `parse_f32` and
  `parse_flag`.
- Writer: the header is `8` when `model.background() != Background::default()`, checked
  before every other version rule (tags still give 7, and so on: the lowest version that
  holds the model). The record is written only then, right after `units` (after
  `geolocation`, `sun` and `units`, before the first `material`).
- Reader: in a version 1 to 7 file the record is `UnknownRecord`. Like `units`, it may
  stand anywhere before `end`, at most once (a second one is `BadField`, even when the
  values are the same; use `settings_seen`). Wrong token count, a flag other than bare
  `on`/`off`, or a number that doesn't parse or isn't finite (`NaN`, `inf`) is `BadField`.
  A channel outside 0..=1 is rejected by the model: `Rejected(BadSettings)`, which
  `apply(SetBackground { from: model.background(), to })` gives on its own.
- A file without the record reads as `Background::default()`.

Example, a model whose only change is its background:

```
drydock-model 8
background 0.5 0.5 0.5 off 0 0.25 1 on 1 0.75 0
body root
end
```

## Done when
- `harness/bin/run-tests -p drydock-kernel` passes (`tests/background.rs` and every older
  test, including the format tests of earlier versions).
- clippy `-D warnings` and fmt are clean.

## Where to look
- `Model::length_format`, `Model::set_length_format`, `ModelRecord::SetLengthFormat`,
  `ModelRecord::inverse`, `Model::apply`, `Model::apply_record` (the `SetLengthFormat`
  arm), `impl PartialEq for Model` (`src/model.rs`).
- `Background`, `DEFAULT_BACKGROUND`, `DEFAULT_SKY`, `DEFAULT_GROUND` (`src/background.rs`).
- `to_text` (the `has_units` / version chain and the `units` writer), `apply_line` (the
  `"units"` and `"sun"` arms), `parse_f32`, `parse_flag`, `expect_count`, `rejected`,
  `FORMAT_VERSION` (`src/format.rs`).
- `docs/format.md`: the `background` row, the reader rules, "What writers produce".
- Tests: `tests/background.rs`; the pattern it follows is `tests/units_model.rs`.
