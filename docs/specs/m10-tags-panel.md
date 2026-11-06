# m10-tags-panel: the Tags panel (show/hide, add, rename, delete)

Milestone M10. Needs `m10-tags-kernel` (tags in the model; `Model::add_tag`, `rename_tag`,
`set_tag_visible`, `remove_tag`, `tag`, `tags()`). Not here: assigning the selection to a
tag (`m10-tags-assign`), leaving hidden instances out of drawing (`m10-tags-hidden`), and
dropping them from the selection (`m10-tags-hidden-selection`).

## What is on main already (don't redo it)
- `PanelId::Tags` (title "Tags", layout key "tags"), between Components and Shadows in
  `PANELS`; closed by default; the `tags` flag in `MenuState`; the "Tags" check item in the
  View menu beside the other panels (`Action::TogglePanel(PanelId::Tags)`).
- `app.rs`: the `App::tags` state, the `PanelId::Tags` arm that calls `tags_panel`, and the
  `PanelAction::Tag(TagAction)` arm that applies each action through the `Editor` and then
  refreshes the document (like materials). You may leave `app.rs` alone.
- Skeletons with `unimplemented!("m10-tags-panel")` that you fill in: `tags_panel.rs`
  (`default_tag_name`, `listed_tags`, `tags_panel`) and four `Editor` methods in `editor.rs`.

## Editor methods
Each pushes exactly one undo step on `Editor`'s own undo stack (`self.undo.push(label,
log)`), sets `self.message = None` when it works, and sets a one-line message when the
kernel refuses (then no step and the model is untouched).
- `add_tag(model, name) -> Option<TagId>`: label "New Tag". Message "Can't make that tag".
- `rename_tag(model, id, name) -> bool`: label "Rename Tag". The same name is accepted with
  no step (the kernel records nothing). Message "Can't rename that tag" for a blank name or a
  missing tag.
- `set_tag_visible(model, id, visible) -> bool`: label "Show Tag" or "Hide Tag"; unchanged is
  accepted with no step. Message "Can't change that tag" for a missing tag.
- `remove_tag(model, id) -> bool`: label "Delete Tag"; the kernel untags the instances in the
  same record list, so undo brings the tag and its assignments back. Message "Can't delete
  that tag".
Push only when the kernel's log is not empty (`UndoStack::push` already ignores an empty
edit).

## The panel (`tags_panel.rs`)
`tags_panel(ui, panel, model) -> Option<TagAction>` returns at most one action per frame.
- **Add**: a button labelled "Add" at the top. Click returns `TagAction::Add`; the app names
  the tag with `default_tag_name`.
- **Empty**: with no tags, the label "No tags" (the Add button stays).
- **Rows** in `listed_tags` order. Each row, left to right:
  - **Eye toggle**: a square click target about 20 points wide, drawn with the painter in the
    text colour (`ui.visuals().text_color()`): an outline ellipse or circle for the eye, with
    a diagonal strike across it when the tag is hidden. No new colours and no new icon file.
    Its accessible widget is `WidgetInfo::selected(Button, enabled, tag.visible, label)` with
    the label `Visible: NAME`, so tests read the state as `toggled()`. Click returns
    `SetVisible(id, !tag.visible)`.
  - **Name**: a label with click sense. A double-click starts a rename: `panel.editing =
    Some((id, name))`.
  - **Delete**: a button whose accessible label is `Delete NAME` (a visible "x" or similar is
    fine). Click returns `Delete(id)`.
- **Renaming** (`panel.editing` is `Some((id, text))`): that row's name is a single-line
  `TextEdit` on `text`. It takes focus when it appears and has the whole text selected, so
  typing replaces the name. (egui note: the focus request lands a frame later and `show`
  may overwrite the cursor, so call `ctx.request_repaint()` while starting, and set the
  selection through `TextEditOutput::state` after `show` while the box is new. The tests
  start from an empty text and don't check the selection; sign-off does, in the window.)
  Enter, or focus leaving the box, ends the rename: if the text is
  not blank (`tag_name_is_good`) and differs from the tag's name, return `Rename(id, text)`;
  either way `editing` becomes `None`. Escape ends it with no action. If the tag being
  edited no longer exists (deleted, or undone away), drop `editing` at the start of the frame.
- The scroll area, if you use one, needs its own `id_salt` (sibling panels share a ui id).
- Only the Add and Rename text come from the user; use egui's default fonts and visuals (the
  brand look is applied globally). No colours of your own.

`default_tag_name`: `Tag N` with the smallest N >= 1 such that no tag has exactly that name.
`listed_tags`: ids sorted by lowercase name, then id.

## Tests
`tests/tags_panel.rs` (egui_kittest; actions go through a real `Editor`, the way the app
applies them) and `tests/editor_tags.rs`. kittest cannot send a double-click, so the rename
tests set `panel.editing` directly (with an empty text); the double-click itself is checked in the real window by
sign-off. Run the whole `drydock` crate's tests before finishing: the dock, menu and layout
tests must still pass.

## Where to look
`show` these: `components_panel` and `ComponentAction` (components_panel.rs, the pattern),
`materials_panel`, `Editor::create_material`, `Editor::purge_materials` (undo step + message
pattern), `UndoStack::push`, `Model::add_tag`, `Model::tag`, `Model::tags`, `tag_name_is_good`,
`PanelAction`, `App::apply_material_action`, `icons::icon_button` (for how the painter and
`widget_info` give an accessible label). Test helpers: `harness` and `State` in
`tests/components_panel.rs`; `EditKey::Undo`/`Redo` via `Editor::key`.
