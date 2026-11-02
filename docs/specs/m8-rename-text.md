# m8-rename-text: shown text says Drydock

The product's public name is **Drydock** (full form **Subspace Drydock**, by Subspace Systems). Spelling: exactly `Drydock`, one word, capital D only. `modeler` stays the codename, the crate prefix, the file extension and the settings folder; those have their own later tasks. This task changes only text the user can see.

## Changes

- `main.rs`: window title `Drydock`; the `eframe::run_native` app name `drydock` (lowercase, it only sets the Linux window class); every `eprintln!` prefix `modeler: ...` becomes `Drydock: ...`. Leave the `Usage: modeler ...` help text (the program name changes in `m8-rename-crates`).
- `document.rs`: `Document::title()` is `<name>[*] - Drydock`; the "not a modeler-model file" read error becomes `not a Drydock model file` (the file's first line stays `modeler-model` until `m8-rename-extension`). Leave the `.modeler` extension handling.
- `chrome.rs`: `about_text()` is `Drydock <version> (<hash>)`; `about_lines()` becomes `Subspace Drydock`, `by Subspace Systems`, `about_text()`, the License line, the Copyright line, `Contributors:`, then the names (unchanged); the About window title is `About Drydock`; the Help item is `About Drydock...`.
- `lib.rs`: `banner()` is `Drydock <version>`; update its unit test.
- `app.rs`: status messages `Drydock: 1 item {verb}` / `Drydock: {count} items {verb}`. Leave `"model.modeler"`, the `.modeler` hint text and extension filters.
- `modeler-mesh/src/stl_write.rs`: STL header prefix `Drydock STL: ` (same 13 bytes, header stays 80 bytes, never starts with `solid`); fix its doc comment.
- `README.md`: title `# Drydock` and an intro naming Subspace Drydock; say that `modeler` is the internal codename and the crate prefix. Keep the rest.
- Not in scope: `MODELER_*` env names, `modeler::` paths, GPU debug labels in `modeler-render`, `dock.rs`/`toolbars.rs` file headers, `recent.rs` config folder, copyright and SPDX lines (stay "Subspace Systems").

## Tests (already written; do not edit)

- `modeler/tests/drydock_text.rs`: About text and window (kittest), and `no_shown_string_says_modeler`, which scans every string literal in `modeler/src` and `modeler-mesh/src` for lowercase `modeler` apart from its short allow-list. It must pass without widening the list.
- Updated existing assertions in `chrome.rs`, `menus.rs`, `menus_fit.rs`, `document.rs`, `document_edit.rs`, `native.rs`, `cli.rs` and `modeler-mesh/tests/stl_write.rs`.

## Where to look

- `show about_text`, `show about_lines`, `show about_window`, and the Help menu item near `chrome.rs:1084`.
- `show Document::title`, and the `NotAModelFile` arm near `document.rs:476`.
- `app.rs` near line 3497 (the `items {verb}` status messages); the rest of `app.rs` is untouched.
- `main.rs`: `with_title`, `run_native`, the `eprintln!` calls (grep `modeler:`).
- `show encode` in `modeler-mesh/src/stl_write.rs`.
- `show banner` in `lib.rs`.
- Test helper to read: `literals` and `ALLOWED` in `modeler/tests/drydock_text.rs`.
