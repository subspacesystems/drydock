# m4-fix-materials-width: long names don't widen the Materials panel

Milestone M4, follow-up from Tyrel's M4 review ("The Materials view gets really
wide if opening a DAE that has a material with a long name").

## Problem

The right column (`egui::Panel::right("materials")`, default 180 points, resizable) grows
to fit its widest content. `materials_panel` (`crates/modeler/src/materials_panel.rs`)
draws each material name in full on one line, the New / Edit / Purge buttons on one line,
and the form's opacity slider and "Texture image" field at their natural widths, so a
long imported name (or path) widens the whole column, up to hundreds of points.

## What to build

`materials_panel` stays within the width its `ui` offers (the tests use 160 points, the
content width of the default column):

- Material rows: the swatch, then the name cut short with an ellipsis when it doesn't fit
  (`egui::Button::selectable(...).truncate()` or equivalent). Hovering a row shows the
  full name; for a textured material the hover text also has the texture path (the full
  name on the first line, the path on the second). The row stays clickable over its whole
  visible width and keeps an accessibility label that starts with the material's name.
- The New... / Edit... / Purge Unused buttons wrap onto a second line when they don't fit
  (`horizontal_wrapped`).
- The form: the Name and Texture image fields take the available width rather than their
  default width (put "Texture image" on its own line above its field); the Opacity
  slider's length is what is left of the row after its label and value.
- No other behaviour changes: the existing `tests/materials_panel.rs` must keep passing.

The Texture Position panel in the same column already fits; a test guards it.

## Tests

`crates/modeler/tests/materials_panel_width.rs` (7): the panel drawn in a 160-point column
with the app's font installed (the workspace builds egui without bundled fonts, so a test
without a font measures every text as zero wide) must not use more than 160 points with
short names, a long name, a long current name, the edit form for a long name, or a long
texture path in the form; the long-named row still chooses its material; and the
Texture Position panel fits too.

## Out of scope

Resizing behaviour of the column itself, a search box, thumbnails.
