# m10-slider-visuals: sliders with a visible rail and a handle that reacts to hover

Feedback on M10: in the Light theme the Shadows sliders show a handle but no
rail, and a slider does not change under the pointer the way a button does.

## Cause

egui 0.36 paints a slider's rail with `visuals.widgets.inactive.bg_fill` (always the
inactive state, whatever the pointer does) and its handle (a rectangle by default) with
`bg_fill` and `fg_stroke` of the state the pointer puts it in. In `look::style` the
inactive `bg_fill` is `surface_raised`: white in Light, the same as the panel, so the rail
vanishes; in Dark it is only slightly lighter than the panel. The hovered `bg_fill` is the
12 % tint `accent_soft` and `fg_stroke` is `ink` in every state, so hovering changes
almost nothing.

## Behaviour

Slider colours change for sliders only. `inactive.bg_fill` also fills checkboxes and other
widgets and `fg_stroke` is the label colour of buttons, so `style()` stays as it is.

- `look::slider_widgets(theme, &mut visuals)` edits a copy of the visuals:
  - `widgets.inactive.bg_fill = palette(theme).border_control` (rail, and the resting
    handle's fill). That is at least 3:1 against `surface` in both themes (non-text
    contrast).
  - `widgets.hovered.bg_fill` and `widgets.active.bg_fill` = the opaque `accent`.
  - `widgets.hovered.fg_stroke` and `widgets.active.fg_stroke` = 2 points wide in `ink`.
    The resting `fg_stroke` stays 1 point `ink`.
  - Nothing else changes (noninteractive, `weak_bg_fill`, `panel_fill`, ...).
- `look::slider(ui, slider) -> Response` adds the slider inside `ui.scope`, after
  `slider_widgets(theme_of(ui.visuals()), ui.visuals_mut())`, so the rest of `ui` keeps its
  style.
- Every slider in the app goes through `look::slider`: the four in `shadows_panel.rs` and
  the opacity slider in `materials_panel.rs` (`grep -rn "Slider::new" app/crates`). Keep
  the `Response` handling as it is (the Shadows panel passes the response to its local
  `slider` event helper); only the way the widget is added changes. In `materials_panel.rs`
  the `slider_width` setting stays in effect (set it on `ui` before the call, as now).

## Tests

`app/crates/drydock/tests/slider_visuals.rs` (already written; do not change it): the
colours of `slider_widgets` in both themes, that `style()` is untouched, and an
`egui_kittest` run of the Shadows panel that hovers the first slider and checks exactly
one painted rectangle changes, to the accent fill with a 2-point stroke. It fails until
the panel calls `look::slider`. `look.rs` tests (`tests/look.rs`) must keep passing.

## Where to look

- `look::slider_widgets`, `look::slider` (skeleton, `unimplemented!`), `look::palette`,
  `look::theme_of`, `look::style` (`show <name>`).
- `shadows_panel::shadows_panel` (four `egui::Slider::new` calls), the opacity slider in
  `materials_panel.rs` (~line 289).
- egui 0.36 `widgets/slider.rs` painting code (~lines 760-835) in the cargo registry.
- Test helpers in `tests/slider_visuals.rs`: `harness`, `circles` (all painted rectangles),
  `slider_visuals`.
