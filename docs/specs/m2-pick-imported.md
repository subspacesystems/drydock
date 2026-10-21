# m2-pick-imported: select, pick and snap on imported geometry

Tyrel's report: on an imported model (05-house) a box around the whole house selected only a line he had drawn, and clicks and snaps ignored the house. Cause: an imported DAE becomes instances of definitions, while `infer::pick`, `infer::infer` and the selection highlight only look at the root body's own edges and faces. (Box selection does find instances; the window never drew them.)

Rule: an instance is selected as a whole. Any hit inside an instance, at any depth, selects the instance of the root body that holds it (`Entity::Instance`). Geometry inside instances can be snapped to but not edited.

## Kernel (`modeler-kernel/src/infer.rs`, `select.rs`; skeletons on main)

### `infer::pick_model(model, probe) -> Option<Entity>`
Like `pick`, over the root body plus all geometry inside instances, each point of an instance's geometry taken through the composed transforms (instance transforms may scale and rotate).
- Edges first: the visible edge nearest the cursor within `probe.radius` (screen distance, as `pick` does). A root edge gives `Entity::Edge`, an edge inside an instance gives `Entity::Instance` for the root-level instance holding it. Ties: the nearer one along the ray, then a root edge before an instance edge.
- Else the face the ray meets first, root or instance, same rule (`Entity::Face` or `Entity::Instance`).
- An edge is hidden when a face (root or instance) other than the ones it touches lies between it and the viewer, as for root geometry today. A face that is nearer along the ray wins over a farther one regardless of owner.
- With no instances the result equals `pick(root, probe)`. A probe that isn't finite gives `None`.

### `infer::infer_model(model, probe) -> Option<Inference>`
Like `infer`, over the same combined geometry: endpoints, midpoints, points on edges, points on faces (with the face's real height), hidden-point rejection, axis and ground fallbacks, direction lock. The snap kind and `point` are in root coordinates. Ids inside `Snap` for instance geometry are the ids in the definition's context; callers use the kind and the point. With no instances the result equals `infer(root, probe)` exactly (including ids). On equal screen distance and depth, root geometry wins.

### `select::instance_segments(model, body, id) -> Vec<(DVec3, DVec3)>`
Every edge of the instance's definition and, recursively, of the instances inside it, as segments in the coordinates of `body`, composed transforms applied. Each edge once (not once per face that uses it). Any order. Empty when the body or the instance doesn't exist. Cycles in definitions can't occur (the model rejects them) but don't loop forever.

### Performance
These run on every pointer move on models like 05-house (many thousands of edges across instances). Don't rebuild a combined `Context` per call; walk the definitions with their transforms, or cache the flattened geometry keyed on something cheap, as long as the answer is always for the current model. The corpus test below runs a handful of calls in a debug build.

Tests: `modeler-kernel/tests/infer_model.rs` (21), `modeler-dae/tests/pick_corpus.rs` (runs with `MODELER_CORPUS`; the oracle sets it).

## Editor (`modeler/src/editor.rs`)
- `pick_at` uses `pick_model`; `infer_at` and `infer_shape_at` use `infer_model`. Nothing else changes: clicks, double/triple clicks (an instance stays selected), boxes, line and shape tools behave as before, now also on instance geometry. Drawing still adds only to the root body.
- `prune` keeps working for `Entity::Instance`.
Tests: `modeler/tests/editor_instances.rs` (8).

## Window (`modeler/src/app.rs`, not unit tested)
Draw the highlight for `Entity::Instance(id)` in the selection: every segment from `instance_segments(model, BodyId::Root, id)`, projected, in the same accent color and 3 px stroke as selected edges. Skip segments that don't project. (Leave faces unfilled.)
Run `--screenshot` or `harness/bin/gui-session` over 05-house if you can; I will.

Touch only `infer.rs`, `select.rs` (replace the skeleton bodies), `editor.rs`, `app.rs`. Don't change any other kernel behaviour or test.
