# m6-eyedropper: Materials panel eyedropper

Milestone M6 (UI), feedback round. Crate `modeler`: `src/editor.rs`, `src/materials_panel.rs`, `src/icons.rs`.

## Goal
A button in the Materials panel that picks a material from the model: press it, click a face (or a group/component instance), and that material becomes current.

## Already on main (skeleton)
`Icon::Eyedropper` (drawn as nothing yet, not in `icons::all()`), `MaterialAction::Eyedropper`, `Editor::arm_eyedropper` / `eyedropper_armed` (bodies `unimplemented!()`), and `app.rs` calling `arm_eyedropper` on that action. The Paint Bucket already samples with Alt+click (`click_paint`); reuse its sampling (`paint::sample`, the side the camera sees) instead of copying it, for example by pulling the "seen side" computation into a helper.

## Rules
- **Editor.** `arm_eyedropper()` sets a flag and drops a paste or placement in progress; the tool stays as it is. While armed, a click (any tool) on a face or an instance makes the sampled material current (`None` = the default material when the face is unpainted), clears the message, switches to the Paint Bucket tool, and disarms. The click changes nothing in the model, selects nothing and adds no undo step (`click` returns false). A click on an edge or on nothing keeps waiting, material and tool unchanged. Esc (`EditKey::Cancel`) disarms with no other effect; `set_tool` disarms. A new editor is not armed.
- **Panel.** On the first row, after Purge: an icon button `icons::icon_button(ui, Icon::Eyedropper, "Eyedropper", "Pick a material from the model", false)`. Clicking it returns `MaterialAction::Eyedropper`. The row must still fit a 160 pt column.
- **Icon.** Draw `Icon::Eyedropper` (a pipette: bulb at the top right, tube running down to a tip at the bottom left) and add it to `icons::all()` after `Purge`. It must pass the existing icon tests (centred, ink minimum, distinct from the others).

## Acceptance
- `cargo test -p modeler --test eyedropper` (editor behaviour and the panel button), `--test icons`, `--test icons_2` (the icon list now ends with the eyedropper).
- Existing tests, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
