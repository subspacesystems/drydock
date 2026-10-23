# m4-app-materials-ui: Materials panel

A panel on the right of the window lists the model's materials with a color swatch, lets
the user choose the Paint Bucket's material, create and edit materials (name, color,
opacity, texture image and its size) and purge the unused ones.

The editor side exists (`docs/specs/m4-app-materials.md`): `Editor::{create_material,
edit_material, purge_materials, material, set_material}`. This task adds the panel and wires
it into the app.

Code: `app/crates/modeler/src/materials_panel.rs` (skeleton on main: fill in every
`unimplemented!`), and the wiring in `app/crates/modeler/src/app.rs`.
Tests: `app/crates/modeler/tests/materials_panel.rs` (`egui_kittest`).

## `MaterialForm`

The fields of the new / edit form. All fields are public; the panel edits them in place.

### `MaterialForm::new_material()`

- `editing: None`, `name`: empty (the kernel names a blank material "Material", or
  "Material 2" and so on when that is taken; the field shows that name as a hint)
- `color: [0.8, 0.8, 0.8]`, `opacity: 1.0`
- `texture`: empty, `size: [1.0, 1.0]`

### `MaterialForm::edit(id, material)`

- `editing: Some(id)`, `name`: the material's name
- `color`: its first three color channels, `opacity`: its fourth
- `texture`: the texture's path, or empty without a texture
- `size`: the texture's size (`[x, y]`), or `[1.0, 1.0]` without a texture

### `MaterialForm::material()`

The material the fields describe, or `None` while a field is out of range:

- `name`: `name` trimmed (blank is fine: the kernel names it)
- `color`: `[color[0], color[1], color[2], opacity]`; `None` when any of these four is not
  finite or lies outside `0..=1`
- `texture`: `None` when `texture` is blank after trimming (then `size` doesn't matter);
  else `Some(Texture::new(trimmed texture, DVec2::new(size[0], size[1])))`, and `None` for
  the whole result when either size is not finite or not greater than 0.

So `MaterialForm::edit(id, &m).material() == Some(m)` for any valid material `m` whose name
and texture path have no surrounding spaces.

## `materials_panel(ui, panel, model, current)`

Draws into `ui`, top to bottom:

1. A heading `Materials`.
2. A row `Default` (the default material), then one row per material of `model`, in
   `model.materials()` order. A material's row is a small square swatch in its color
   (opacity ignored) followed by a selectable label whose text is exactly the material's
   name; hovering the row shows the texture path when it has one. The row of `current`
   (`Default` when `current` is `None`) is drawn selected; the others are not.
   Clicking a row returns `MaterialAction::Choose(Some(id))` (`Choose(None)` for
   `Default`), also when it is already current.
3. Buttons `New...`, `Edit...` and `Purge Unused`:
   - `New...` opens the form with `MaterialForm::new_material()` (replacing a form
     that was open). No action.
   - `Edit...` is enabled only when `current` is `Some` material of `model`; it opens the
     form with `MaterialForm::edit` of that material. No action.
   - `Purge Unused` returns `MaterialAction::Purge`.
4. While `panel.form` is `Some`: a separator, a heading `New Material` (when `editing` is
   `None`) or `Edit Material`, and the fields, each with a label in front:
   - `Name`: single-line text field. While empty it shows the hint
     `paint::unique_material_name(model, "Material")`. It takes keyboard focus on the frame
     the form opens (from `New...` or `Edit...`), so the user can type the name straight
     away.
   - `Color`: an RGB color button (egui's color picker). egui's picker converts the color
     back and forth each frame and loses a little precision, so draw it on a copy and write
     the copy into `color` only when the button reports a change: an untouched form gives
     back exactly the color it was filled with.
   - `Opacity`: a slider from 0 to 1.
   - `Texture image`: single-line text field (the image file; blank for none).
   - `Width (m)` and `Height (m)`: number fields for `size`.
   - Buttons `OK` and `Cancel`. `OK` is enabled only while `material()` is `Some`; it
     returns `MaterialAction::Create(material)` for a new material or
     `MaterialAction::Edit(id, material)` when editing `id`, and closes the form
     (`panel.form = None`). `Cancel` closes the form and returns nothing.

When the form edits a material that `model` no longer has (an undo took it away), the panel
closes the form (no action) the next time it is drawn.

The panel returns at most one action per frame. The list and the buttons keep working while
the form is open.

## App wiring (`app.rs`)

- The app keeps one `MaterialsPanel`. While a document with a kernel model is open, the
  panel is drawn in a right side panel (`egui::Panel::right("materials")`, resizable, about
  180 points wide by default) for that model with `current = editor.material()`. Add the
  right panel before the central viewport panel, like the toolbar. Nothing is drawn there
  without a kernel model.
- `Choose(m)`: `editor.set_material(m)` and switch to the Paint Bucket tool (the same path
  as choosing it from the toolbar, so the editor's tool changes too).
- `Create(m)` / `Edit(id, m)`: when `m` has a texture whose path is relative, resolve it
  first with `document::resolve_texture_path(path, folder)`, `folder` being the folder of
  the document's file (an empty path for an untitled document). Then
  `editor.create_material` / `editor.edit_material` on the document's kernel model, and
  when that changed the model (`Some` / `true`), refresh the document as the other edits
  do (`refresh_document`), so the viewport and the window title update.
- `Purge`: `editor.purge_materials`; refresh the document when it removed any.
- Keys typed into the panel's text fields must not reach the tools (`global_input` already
  returns early while egui wants keyboard input; keep it that way).

## Out of scope

A file chooser for the texture image (the path is typed), a material library, drag and drop
of materials, sampling from the panel, the texture positioning tool
(`m4-app-texture-position`).
