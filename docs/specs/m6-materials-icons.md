# m6-materials-icons: icon buttons in the Materials panel

Milestone M6 (UI). Crate `modeler`: `src/materials_panel.rs`.

## Goal
The Materials panel's five text buttons become icon buttons drawn with the app's own icons (`src/icons.rs`, merged in `m6-icons`), each with a tooltip. Everything else about the panel stays as it is.

## Already on main
`icons::icon_button(ui, icon, label, tooltip, selected)` and `Icon::{New, Edit, Purge, List, Tiles}`. The panel (`materials_panel`) with its text buttons, and its tests (`materials_panel`, `materials_layout`, `materials_panel_width`, `materials_purge_confirm`), which must keep passing unchanged.

## Rules
- Use `icons::icon_button` for each button, with the label it has today as the accessible label (tests and screen readers find the buttons by it) and this tooltip:

  | Label | Icon | Tooltip | Selected |
  | --- | --- | --- | --- |
  | `New...` | `New` | `New material` | never |
  | `Edit...` | `Edit` | `Edit the current material` | never |
  | `Purge Unused` | `Purge` | `Remove the materials nothing uses` | never |
  | `List` | `List` | `Show as a list` | when the list shows |
  | `Tiles` | `Tiles` | `Show as tiles` | when the tiles show |

- Edit is disabled (and does nothing when clicked) when there is no current material the model has, as today: draw it inside a disabled `Ui` (`add_enabled_ui`).
- Clicking does what the text button did (open the new form, open the edit form, ask to purge, switch list / tiles).
- Layout as today: New, Edit, Purge in that order on one row under the heading; the search box under them; List and Tiles on one row under the search box; the materials under that.

## Acceptance
- `cargo test -p modeler --test materials_icons`: each button is a `BUTTON_SIZE` square button found by its label; rows and order; each tooltip on hover; List / Tiles selected state; the buttons still act; Edit disabled with no current material.
- The existing Materials panel tests, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
