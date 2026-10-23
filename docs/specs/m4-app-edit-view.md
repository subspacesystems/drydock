# m4-app-edit-view: window wiring for the faded or hidden rest

While the editor has an instance open, the window draws the rest of the model faded, or not
at all when the user chose to hide it. The scene function
(`modeler_dae::kernel_scene_editing`, `RestView`) is on main
(`docs/specs/m4-edit-view-scene.md`), and so is the editor's `hide_rest` option
(`Editor::{hide_rest, set_hide_rest}`) that already controls what snaps and picks. Skeletons on
main: `Document::refresh_editing`, `Action::ToggleHideRest`, `Effect::ToggleHideRest`,
`MenuState::hide_rest`, and an `unimplemented!` arm in `App::apply`.

Files: `app/crates/modeler/src/{document.rs, chrome.rs, app.rs}`. Tests:
`tests/document_edit_view.rs`, `tests/chrome_edit_view.rs`. Every other test must keep passing.

## `Document::refresh_editing(path, rest)`

Like `refresh`, but `scene` is `kernel_scene_editing(kernel, path, rest)`. `stats` describe
the whole model, whatever is drawn: build them as `refresh` does (from the normal
`kernel_scene`), so faces, edges, triangles, lines and size do not change while the rest is
hidden. `modified` becomes true, as in `refresh`. Does nothing without a kernel model.
`refresh` itself must stay as it is.

## View menu

In `menu_bar`, after the standard views, a separator and one item, always enabled:

| `state.hide_rest` | Label | Returns |
| --- | --- | --- |
| false | `Hide Rest of Model` | `Action::ToggleHideRest` |
| true | `Show Rest of Model` | `Action::ToggleHideRest` |

Clicking it closes the menu, like the other items. No shortcut.

## App

- `Effect::ToggleHideRest`: flip the editor's option (`set_hide_rest(!hide_rest())`), then
  refresh the document and the uploaded scene like any edit (`refresh_document`). No undo step.
- `refresh_document` draws with `Document::refresh_editing(editor.path(), rest)`, where `rest`
  is `Hidden` when the editor hides the rest, else `Faded`. With the root open that is the
  normal scene, so nothing changes outside an instance.
- The scene must also be rebuilt whenever the open path changes without a model edit: after a
  double click opens or closes an instance and after Esc closes it (watch `editor.path()` each
  frame, or compare it before and after the editor handles input), including when undo or redo
  repairs the path.
- Bounds for Zoom Extents come from the scene as drawn (`scene_bounds(&document.scene)`, as
  `refresh_document` does now).
- A new empty document or opening a file starts with the root open and the rest shown.
