# m6-components-import-ui: import a file as a component

Milestone M6 (UI). Crate `modeler`: `src/components_panel.rs`, `src/editor.rs`, `src/app.rs`.

## Goal
The Components panel gets an Import button. It asks for a `.dae` or `.modeler` file, turns the file's model into a new component of the open model (one undo step), and starts placing it. The kernel half (`modeler_kernel::copy::import_component`) is on main. Three tested items; the dialog and wiring are checked by hand in the real window.

## Rules for the tested items
- `components_panel` shows a button with the accessible label `Import`, role button, on the same row as the List and Tiles buttons (after them). Clicking it makes the panel return `ComponentAction::Import`. It is shown whether or not the model has components, and does not disturb the existing behaviour (Place on a click, search, list/tiles).
- `read_component_source(path)` reads the file with the same readers the app uses to open files (`Document::open`) and returns its kernel model plus the file name without extension as the definition name. Errors (all `Err(String)`, one line, containing the file's name): the file is missing or unreadable, or the file read but produced no kernel model (use the document's `kernel_error` text).
- `Editor::import_component(model, source, name)` calls `import_component` with the editor's own undo stack and the label `Import Component`, then calls `place_definition` for the new definition (so `placing()` is `Some(id)`). On `Err(CopyError)` it returns a one-line message (`Empty` reads "Nothing to import"; others say the import failed), stores it as the editor message, and leaves model, undo stack and placing state as before.

## App wiring (checked by hand)
- `ComponentAction::Import` opens the existing file dialog (see `OpenDialog`) in an "import" mode: same list of folders and `.dae`/`.modeler` files, title and button say Import instead of Open, and choosing a file does not replace the document. A `Cancel` closes it.
- On choosing a file: `read_component_source`, then `Editor::import_component` on the open document's kernel model, then refresh the document (scene, dirty flag) as other edits do. Any error is shown the way other file errors are.
- Replace the TODO for `ComponentAction::Place(id)`: call `Editor::place_definition` on the open model and show the placing preview, as paste does.

## Acceptance
- `cargo test -p modeler --test components_import_ui`
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
