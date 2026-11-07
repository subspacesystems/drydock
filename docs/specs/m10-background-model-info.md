# m10-background-model-info: Background section in Model Info; viewport colors from the model

Tyrel's feedback on M10: the viewport was dark in Dark mode. The background, sky and
ground colors belong to the model (kernel: `m10-background-kernel`, renderer:
`m10-sky-ground-render`, both merged before this task). This task adds the editing UI and
makes the viewport use the model's settings.

## What to build

1. **`look::model_style(Option<&Background>) -> Style`** (skeletons in `look.rs`):
   - `background` = the model's `color`, alpha 1.
   - `sky` = `Some(sky_color, 1.0)` when `sky` is on, else `None`; `ground` likewise.
   - `default_line`, `default_face`, `default_back` = the **Light** palette's `edge`,
     `face_front`, `face_back` (as `viewport_style(Theme::Light)` gives), whatever the OS
     theme: the default sky is light and dark-theme edges would vanish on it.
   - `sun` = `None`. `None` for the argument (no model yet) = `Background::default()`.
   - `look::apply_model_style(&mut Style, Option<&Background>) -> bool`: copy the colour
     fields (`background`, `sky`, `ground`, `default_line`, `default_face`, `default_back`;
     never `sun`) from `model_style`; true when any changed.
   - Leave `viewport_style` and `apply_viewport_theme` as they are (existing tests).
2. **`Editor::set_background(&mut self, model, Background) -> bool`**: like
   `set_length_format`: one undo step labelled "Set Background" via `Model::set_background`;
   no step when unchanged (returns true); on `Err` returns false with
   `self.message = Some("Can't use those colors")`, model unchanged.
3. **`background_ui::background_section(ui, &Background) -> Option<Background>`** in the style
   of `units_ui::units_section`:
   - `ui.strong("Background")`, then a row `field_label("Background")` + `color_edit_button_rgb`
     for `color`; a row with `ui.checkbox(.., "Sky")` and a color button for `sky_color`; the
     same for "Ground". The color buttons stay enabled when the box is off (the color is kept).
   - Returns `Some(new)` on the frame a checkbox is toggled or a color button changed
     (`response.changed()`), with only that field changed; `None` otherwise.
4. **App wiring** (`app.rs`):
   - In the Model Info window, after the Units section (when `kernel` is present), call
     `background_section` with `model.background()`, and apply a pick with
     `editor.set_background` then `refresh_document` (so Ctrl+Z undoes it and the document
     turns modified). Keep a separator between sections like Units.
   - Replace `look::apply_viewport_theme(&mut self.style, theme)` at the top of `ui()` with
     `look::apply_model_style(&mut self.style, background)` where `background` is the open
     document's kernel `background()` (or `None` with no model / no kernel). The initial
     `Style` (`App::new`, ~line 281) is built with `look::model_style(None)`. Set `self.dirty`
     when it returns true, so a change, an undo, a redo, a New or an Open redraws. Check that
     a color change reaches the screen without re-uploading the scene (style is passed to the
     renderer each frame, as the theme change was).
   - Drag on a color button's picker changes every frame: that is a lot of undo steps. Accept
     it for this task, except merge consecutive changes if `UndoStack` already offers a way;
     do not build a new mechanism.
5. Docs: one line in `docs/guide/` Model Info page if it exists (look for "Units" in the
   guide pages); the guide coverage test stays green.

## Tests (already written, do not edit)

`app/crates/drydock/tests/background_model_info.rs`: style from the model (defaults, off
flags, custom colors, none, light edge colors against the Dark palette), `apply_model_style`,
`Editor::set_background` (one step, undo/redo, unchanged, invalid), and the section with
`egui_kittest` (heading, labels, checkbox state, toggling Sky and Ground). The color pickers'
popups are not driven by tests; the oracle only needs these to pass.

## Where to look

- `show look::viewport_style`, `show look::apply_viewport_theme`, `show look::palette`
- `show Editor::set_length_format` (pattern for the editor step), `Editor::key`, `UndoStack`
- `show units_section`, `show field_label`; `show model_info_window_with`
- `show Model::background`, `show Model::set_background`, `drydock_kernel::background::Background`
- `app.rs`: the Model Info block with `picked_unit` (~3113), `ui()` start (~2939), `App::new` (~281)
- `show Style` in `drydock-render` (fields `sky`, `ground`)
- Test helpers: the `section` harness in `tests/units_ui.rs`
