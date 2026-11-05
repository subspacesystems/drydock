# m10-brand-numbers: numbers in Roboto Mono, with units; the Measurements box corners

Part of the Subspace brand work. Needs `m10-brand-theme` and
`m10-brand-fonts` merged (the monospace family is Roboto Mono once the fonts task is in;
this task only chooses the family, it does not load fonts).

## What to build

1. **Measurements box corners.** `chrome::measurements_corners()` returns
   `CornerRadius { nw: 2, sw: 2, ne: 8, se: 8 }` (2 points on the left, 8 on the right).
2. **The box itself.** `chrome::status_measurement(ui, measurement, typed)` (the status
   bar's measurement label, value and typed text) draws what it shows inside one
   `egui::Frame` with `corner_radius: measurements_corners()`. Fill and stroke come from
   the theme, not constants: the noninteractive widget fill and `bg_stroke` of `ui.style()`.
   Inner margin about 8 points wide and 2 to 4 tall. When there is no measurement and no
   typed text, nothing is drawn, no frame either. Today the function returns after
   drawing plain labels; callers (`app.rs`, the status bar) do not change.
3. **Numbers are monospace.** The value and the typed text use the Monospace family
   (`ui.monospace` already does, keep it); the label ("Length", "Dimensions") stays
   Proportional.
4. **Entity Info.** In `entity_info_panel`, values that are numbers are drawn in the
   Monospace family: Length, Area, Width, Depth, Height, Volume and the other numeric rows
   from `solid_ui::status_rows` (counts such as "Open edges" too), and "In model". Words
   are not: material names ("Default", "Varies", a material's name), "Yes"/"No". Labels
   stay Proportional. Do it in `value_row` with a flag per call (for example a second
   function `number_row`), not by guessing from the text.
5. **Units.** Every length and area in these two places already comes from
   `units::format_with` / `format_area_with` / `format_volume`, which end in the unit
   (`mm`, `cm`, `m`, `"`, `'`, and `²` or `³` forms). Keep it so: do not add any number
   without its unit. Angles keep `°`. Counts and scale factors have no unit and stay as
   they are.

## Out of scope

Loading the fonts (`m10-brand-fonts`), colours (`m10-brand-theme`), other panels.

## Acceptance tests

`app/crates/drydock/tests/numbers.rs`: the corners function; the frame's rectangle with
those corners is among the shapes drawn; nothing to show draws no box; the status value
and typed text are Monospace and the label Proportional; Entity Info edge values and
dimensions, face area (`12m²`, `12000000mm²`) and unit-following dimensions (`300cm`) are
Monospace, labels and "Default" Proportional. The tests read the families from the shapes
egui paints, so no new API is needed for them.

## Where to look

- `chrome::status_measurement` (`show`), `chrome::measurements_corners` (skeleton).
- `entity_info::entity_info_panel`, `entity_info::value_row`, `solid_ui::status_rows`.
- `units::format_with`, `format_area_with`, `Editor::measurement` (the strings the box shows).
- `look::style`, `look::palette` (theme colours); `egui::Frame`, `egui::CornerRadius`.
- Test helpers in `tests/numbers.rs`: `draw`, `family_of`, `panel_texts`.
