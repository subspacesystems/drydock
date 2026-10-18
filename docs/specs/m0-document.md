# m0-document: opened-file model

Milestone M0. Crate `app/crates/modeler`, file `src/document.rs`. Implement the three `unimplemented!("m0-app")` functions there; don't change public signatures or fields, and leave `input.rs` and `main.rs` alone. Allowed dependencies: those already in the crate's `Cargo.toml`.

## `Document::open(path)`
- Import with `modeler_dae::import_file`, flatten with `modeler_dae::flatten`, keep the `path` as given, and fill `Stats`.
- `Stats`: `triangles` and `lines` from the scene's counts, `materials` = number of materials in the flat scene, `size_m` = extent of `scene.bounds` along X, Y, Z as `f64` (all zero when the scene has no bounds).
- Errors are a single line (no newline) that starts with or contains the file's name (not the whole directory path is required, but the file name must appear), followed by the reason. Use `ImportError`'s `Display` for the reason. Never panic on bad input.
- Must be callable from a background thread (`Document` and the result are `Send`).

## `Document::title()`
`"<file name> - modeler"`, using only the last path component.

## `format_size(size_m)`
Each axis with exactly three decimals, joined by `" x "`, then `" m"`. Example: `4.115 x 6.706 x 2.438 m`.

## Tests (fixed; don't edit)
`tests/document.rs` with `tests/fixtures/slab.dae` (a 2 x 3 quad plus a 4 m line, one material).
