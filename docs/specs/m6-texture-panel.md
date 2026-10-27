# m6-texture-panel: Texture Position becomes a panel of its own

Milestone M6 (UI), feedback round 1. Crate `modeler`: `src/texture_panel.rs`, `src/chrome.rs`, `src/app.rs`.

## Goal
Texture Position stops being the lower half of the Materials panel. It is a fifth docked panel, with the same header, collapse, close, dock and float behaviour as the other four.

## Already on main
- `PanelId::TexturePosition` (title "Texture Position", layout key `texture-position`), last in `PANELS`: docked in the right column after the others, closed by default. Older layout files without its line simply get that default.
- `MenuState::texture_position` (filled by the app; the View menu does not use it yet).
- `PanelId::TexturePosition` has an empty arm in `App::panel_body`.

## What to do
1. `texture_panel::texture_panel` and `texture_panel::texture_position_section` no longer draw the "Texture Position" heading (the panel header shows it). Everything else is unchanged: in the section the handles button is still above the controls.
2. View menu: a "Texture Position" item after "Shadows", checked while `MenuState::texture_position` is true, sending `Action::TogglePanel(PanelId::TexturePosition)` like the other panel items. The app already turns that into reveal.
3. App wiring (checked by hand in the real window, not by tests):
   - The Materials panel body ends after the Materials widget: no separator and no texture section under it.
   - The `PanelId::TexturePosition` body shows the section exactly as the Materials body did (same `target`, same `texture_on`, same `PanelAction::Handles` and `PanelAction::Texture` handling). When there is no document, show "No model".
   - Selecting a textured face still enables the controls; "Position in Viewport" still works when the panel is docked, and the handles keep showing while the panel is collapsed.

## Acceptance
- `cargo test -p modeler --test texture_panel_dock`; `texture_panel`, `texture_handles_button`, `dock_layout`, `dock_ui`, `dock_stack`, `menus`, `menus_items`, `materials_panel*` pass.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
