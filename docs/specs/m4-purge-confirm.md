# m4-purge-confirm: Purge Unused asks first

Milestone M4 (Tyrel, round 4: Purge Unused should ask before it removes
anything and say which materials go).

## What is already on main

- `modeler_kernel::paint::unused_materials(model) -> Vec<MaterialId>`: the materials no
  existing face side and no instance uses, in the model's material order (exactly what
  `purge_materials` removes).
- `MaterialsPanel::confirming(&self) -> bool` skeleton (`unimplemented!`).

## What to build (`modeler` app crate)

### `materials_panel` (`src/materials_panel.rs`)

- Clicking **Purge Unused** no longer returns `MaterialAction::Purge`. It opens a modal
  dialog (`egui::Modal`, so the panel and the rest of the window take no clicks while it
  shows) built from `unused_materials(model)` at that moment:
  - Some unused: the text `The following unused materials will be removed from the model:`,
    then each unused material's name as its own label, in model order (put them in a
    scroll area if there are many), then the buttons **Purge** and **Cancel**.
  - None unused: the text `No unused materials to remove.` and one button **OK**.
- **Purge**: `materials_panel` returns `Some(MaterialAction::Purge)` on that frame, once,
  and the dialog closes. **Cancel**, **OK** and Escape close it and return nothing.
- `confirming()` is true while the dialog shows, false otherwise (also for a fresh
  `MaterialsPanel::default()`). Keep the dialog state in `MaterialsPanel` (it derives
  `Clone, Debug, Default, PartialEq`; keep that).
- Everything else in the panel behaves as before.

### App (`src/app.rs`)

`MaterialAction::Purge` is handled as now. Add `self.materials.confirming()` to the
`dialog_open` check in `App::update` so viewport keys (Delete, tool letters, Escape) don't
reach the model while the dialog shows.

## Tests

`modeler/tests/materials_purge_confirm.rs` (egui_kittest). Two tests in
`tests/materials_panel.rs` (`purge_unused_sends_purge`,
`list_and_buttons_work_while_the_form_is_open`) are updated in this task's spec commit to
confirm with **Purge**.
