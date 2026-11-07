# m10-tags-eye-icon: a real eye in the Tags panel

Feedback on M10: the show/hide toggle in the Tags panel is a ring with a dot, so it
reads as a radio button. The brand icon set now has `eye` and `eye-off` (already on main:
`design/brand/icons/eye.svg`, `eye-off.svg`, `Icon::Eye`, `Icon::EyeOff`, the README entry
and the icon-count tests). This task makes the panel use them.

## What to build

Change `tags_panel::eye_toggle` (src/tags_panel.rs) to draw the icon instead of the painted
circle, dot and strike:

- Visible tag: `Icon::Eye` in `ui.visuals().text_color()`.
- Hidden tag: `Icon::EyeOff` in `ui.visuals().weak_text_color()`.
- Draw with `icons::paint(painter, rect, icon, color)` into the toggle's 20 x 20 point rect.
- Keep everything else: the 20 x 20 `Sense::click()` allocation, the `WidgetInfo::selected`
  with label `Visible: <name>` and `selected = visible`, and the click returning the
  response. Remove the now unused imports (`Stroke`, `vec2`) if clippy says so.

Do not edit the SVGs or `generate_icons.py` (the artwork is done; SVGs are never
hand-edited).

## Tests

`tests/tags_eye_icon.rs` (given, do not change) drives the panel with egui_kittest and checks:
the Eye texture inside the toggle in the text color when visible; EyeOff in the weak text
color and no Eye when hidden; no circle or line shape of its own in the toggle; clicking
toggles and swaps the icon; the icon names. The existing `tags_panel`, `tags_row_layout`,
`tags_panel_width` and `icons_svg` tests must stay green.

## Where to look

- `tags_panel::eye_toggle`, `tags_panel::tags_panel` (src/tags_panel.rs): `show eye_toggle`
- `icons::paint`, `icons::Icon`, `icons::uri` (src/icons.rs)
- Test helpers: `harness`, `quads`, `texture_of` in `tests/tags_eye_icon.rs`
