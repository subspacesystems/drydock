# m6-panel-speed: Entity Info and Components stop redoing their work every frame

Milestone M6 (UI), feedback round. Crate `modeler`: `src/entity_info.rs`, `src/components_panel.rs`.

## Goal
With a large model (a whole house in one group) the app crawls while Entity Info or Components is open. Measured on a real house model in a release build: describing the selected group takes about 30 ms and working out the component thumbnails about 25 ms, and both happen on every frame; the thumbnails also hand egui about 160 000 line segments to draw every frame. Make both panels keep their results until something they depend on changes, and cap how many segments a thumbnail draws.

## Already on main
- `Model::cache_key()` is public: `(model identity, revision)`. The revision changes on every edit that can change what the panels show (geometry, materials, names, instances).
- `entity_info::DescribeCache` (with `get`, `computed`) and a `cache` field on `EntityInfoPanel`.
- `components_panel::ThumbnailCache` (with `get`, `computed`), a `thumbnails` field on `ComponentsPanel`, and `MAX_THUMBNAIL_SEGMENTS` (400).
The bodies are `unimplemented!()`; the field layout of the two cache structs is yours (keep their derives).

## Rules
- `DescribeCache::get(model, body, selection)` returns exactly what `describe(model, body, selection)` returns. It computes again only when `model.cache_key()`, `body` or `selection` (as a list, order included) differ from the last call; otherwise it returns the stored description. `computed()` counts the calls that computed.
- `entity_info_panel` gets its description from `panel.cache` instead of calling `describe` itself. Everything it shows and returns stays as it is.
- `ThumbnailCache::get(model, definition)` returns exactly `thumbnail_segments(model, definition)`. It keeps one result per definition and drops all of them when `model.cache_key()` changes. `computed()` counts the calls that computed.
- `components_panel` gets every thumbnail from `panel.thumbnails` (list and tiles).
- `thumbnail_segments` returns at most `MAX_THUMBNAIL_SEGMENTS` segments. When a definition has more edges, keep the ones longest on screen (after projection). The framing (scale and centring) is still worked out from every edge, so the kept segments sit exactly where they would without the cap. With up to `MAX_THUMBNAIL_SEGMENTS` edges, nothing changes.
- No other behaviour changes.

## Acceptance
- `cargo test -p modeler --test panel_speed`: cache results equal the uncached functions; reuse while inputs are unchanged; recompute on a new selection, body, model edit (rename, geometry) or another model; the Entity Info panel describes once over 20 frames and follows a new selection; thumbnails computed once per definition over many frames in both list and tiles; recomputed after an edit; the cap keeps the longest segments in place.
- The existing `entity_info`, `components_panel` tests, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
