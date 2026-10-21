# m2-infer-cache: pick and snap fast on big imported models

Tyrel's report: on 05-house the line-tool indicator lags the cursor by about half a second. Cause: `infer::infer_model` and `infer::pick_model` call `flatten(model)` on every pointer move, rebuilding every vertex, edge and face of every instance (cloned points, sets of touching faces) before looking at any of it. `docs/specs/m2-pick-imported.md` said not to do this.

## Change (`modeler-kernel/src/model.rs`, `modeler-kernel/src/infer.rs`)

1. `Model::revision() -> u64` (skeleton on main returns 0). It must differ after every change to the root, a definition, or an instance: `add_definition`, `set_definition`, `remove_definition`, `add_instance`, `set_instance`, `remove_instance`, `edit_geometry` (bump even if the closure did nothing), and `apply` / `apply_edit` / undo / redo when they succeed. Reads and `clone()` leave it unchanged. Material-only changes may bump it or not.
2. A clone must not be mistaken for its original. Two models can have the same revision and different contents (clone, then edit each once). Give every `Model` a process-unique id, new on `Model::new()`/`default()` and on `clone()` (implement `Clone` by hand, or wrap the id in a small type whose `Clone` takes a fresh id). `PartialEq` and `Debug` output stay as they are in meaning (equality must not look at the id or the revision).
3. `infer_model` and `pick_model` flatten once per `(model id, revision)` and reuse the result. Keep one cached flattening per thread (`thread_local!`), held as `Rc`/`Arc` so the lookup borrows nothing across the query. Results must be identical to today's. No other public API changes.
4. Cheap rejection in the scans over `Flat` (only if still needed after 3; do it when it is a few lines): store a bounding box per flat face and per edge, and skip a face when the ray misses its box, and skip an edge when both ends project farther than `radius` from the cursor on the same side of the cursor's box. Don't change any answer.

Do not touch `editor.rs`, `app.rs` or any existing test. Unused-code warnings must not appear (clippy `-D warnings`).

Tests: `modeler-kernel/tests/infer_cache.rs` (7; five already pass on main and guard against a stale or shared cache), plus the existing `infer_model.rs`, `infer_props.rs`, `editor_instances.rs` and the corpus pick test, which must stay green.

I will measure on 05-house with `harness/bin/gui-session` after it merges.
