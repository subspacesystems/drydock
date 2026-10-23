# m4-edit-path: editing inside an instance (kernel)

Implement the skeletons on main:

- `EditPath` in `app/crates/modeler-kernel/src/editpath.rs`;
- `pick_in` and `infer_in` at the end of `app/crates/modeler-kernel/src/infer.rs`.

You may add private helpers in `infer.rs` and refactor its private code (for example, share
the body of `infer_model` with `infer_in`). Don't change the behaviour of existing public
functions; their tests must keep passing. Tests: `tests/edit_path.rs` and
`tests/edit_path_props.rs` in the kernel crate.

Read first: `docs/specs/m2-inference.md` (snaps, priorities, hidden points, ties) and
`docs/specs/m2-pick-imported.md` (`pick_model` and `infer_model`, geometry inside instances).

## Why

The user opens an instance to edit its definition in place. Tools then work in the open
body (the definition), in its coordinates, while the rest of the model stays on screen.
Every instance of that definition shows the edit. The app keeps an `EditPath` and builds
its probes in the open body's coordinates (it transforms the cursor ray by the inverse of
`EditPath::transform` and composes its projection with that transform). This task gives it
the path and the pick and inference that work there.

## `EditPath`

`steps[0]` is an instance of the root; each later step is an instance inside the definition
of the step before it. Instance ids are unique in the whole model, but a step only counts
when it is an instance of the body the steps before it open.

- `root()`: empty path. `is_root()`: no steps.
- `body(model)`: `BodyId::Root` for the empty path, else `BodyId::Definition` of the last
  step's definition. `None` when a step doesn't resolve (not an instance of the body the
  path has opened so far, or that body is gone).
- `transform(model)`: `T0 * T1 * … * Tn` of the steps' instance transforms (identity for the
  root): maps the open body's coordinates to the world. `None` exactly when `body` is `None`.
- `open(model, instance)`: the path plus `instance` when the path resolves and `instance`
  is an instance of the open body; else `None`.
- `close()`: drops the last step; the root stays the root.
- `valid_prefix(model)`: the longest leading part of the steps that resolves (the empty path
  at worst).

## `pick_in(model, path, rest, probe)`

The probe is in the open body's coordinates. The candidates are:

- the open body's own edges and faces, reported as `Entity::Edge` / `Entity::Face` with
  their ids in the open body;
- the geometry inside the open body's instances (at any depth, with the composed
  transforms), reported as `Entity::Instance` of the instance *in the open body* that holds
  it;
- with `rest`: every other piece of geometry of the model (the root's and other bodies' own
  geometry along the path, other instances, other instances of the same definition), placed
  where it shows (mapped into the open body's coordinates with the inverse of
  `path.transform` times its world transform). A pick that lands on it returns `None`.

Without `rest`, the rest of the model is ignored: it neither picks nor hides anything (the
app hides it on screen).

The rule is `pick_model`'s: the nearest visible edge within the snap radius, else the first
face the ray hits; ties as in `pick_model`, with the open body's own geometry before its
instances' and those before the rest. For the root path the result equals `pick_model`'s,
whatever `rest` is.

`None` when the path doesn't resolve, when `path.transform` has a non-finite entry or
`|det| < 1e-12`, or when nothing is hit.

## `infer_in(model, path, rest, probe)`

The probe (cursor ray, `project`, `anchor`, `lock`, `centers`, `axes`) and the returned point
are in the open body's coordinates. The ground plane is the open body's `z = 0` plane (what
`infer` already does with the probe's coordinates).

The snapping geometry is the open body's own geometry, the geometry inside its instances
and, with `rest`, every other piece of geometry of the model, all placed as in `pick_in`.
Everything works as in `infer_model` over that set: the same categories and priorities, the
same hidden-point test (faces of the set hide points; without `rest` the rest hides
nothing), the direction lock. Ties: the open body's own geometry, then its instances', then
the rest.

Ids in `Snap`: for the open body's own geometry, its ids; elsewhere the ids in the context
the geometry lives in (callers use the kind and the point, as with `infer_model`).

For the root path the result equals `infer_model`'s, whatever `rest` is.

`None` in the same cases as `pick_in`'s path and transform checks, and when `infer_model`
would give `None` (invalid probe, nothing to snap and no ground point).

### Cost

The app calls `infer_in` and `pick_in` every frame while the pointer moves. Cache the
flattened geometry like `infer_model` does (thread-local, keyed by the model's cache key),
with the path and `rest` added to the key.

## Box selection

Nothing to add: `select::box_select(model, body, project, …)` already works on any body; the
app passes `path.body(model)` and its composed projection. One test checks that.

## Examples

Definition `P` holds a 2 m square `(0,0,0)-(2,2,0)`; instance `A` of `P` in the root at
`x + 10`. Path `[A]`: `body` is `Definition(P)`, `transform` the translation by `(10,0,0)`.
A top view probe over world `(11, 1)` (local `(1, 1)`) picks `Face(f)` of `P`; with a root
square at `z = 5` covering the same spot and `rest`, the pick gives `None`; without `rest`,
`Face(f)` again. A root vertex at world `(5, 5, 0)` snaps as an endpoint at local
`(-5, 5, 0)` with `rest`; without it the cursor lands on the ground at about the same local
point with `Snap::Ground`.

## Not in scope

- Editor state, double click to open, Esc to close, drawing the rest faded or hidden
  (`m4-app-edit-context`, `m4-app-components`).
- Tools: they already take a body and points in its coordinates.
