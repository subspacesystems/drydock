# m7-curves-carry: curves carried through groups, components, copies and push/pull caps (kernel)

`m7-curves` made arcs and circles stay curves through edits in one body. Some operations
rebuild geometry under new vertex ids instead of editing it in place: make group/component,
explode, copy/paste, import, and push/pull's new cap. Today they leave plain edges where the
source had a curve, so a circle put in a group, a pasted circle or a pulled cylinder's top
loses its center and radius. The exporter (`m7-print-mesh`) works per group and component
and needs those curves. This task carries them: wherever an operation copies geometry through
a vertex map and a matrix, it now copies the curves too.

## Files

Already on main (don't redesign):
- `curve.rs`: `Curve::transformed` and `carry`, signatures and docs, with
  `unimplemented!()` bodies. Everything from `m7-curves` (`fits`, `fit`, `curve_edges`,
  `reconcile`, ...) is there and works.

You implement, changing only these files:
- `app/crates/modeler-kernel/src/curve.rs`: `Curve::transformed`, `carry` (private helpers
  welcome; reuse the module's `step_edges`, `fits`, `fit`).
- `app/crates/modeler-kernel/src/component.rs`: `make_component`.
- `app/crates/modeler-kernel/src/copy.rs`: `extract` and `copy_body_into` (which `insert` and
  `import_component` use).
- `app/crates/modeler-kernel/src/explode.rs`: `explode_inner`.
- `app/crates/modeler-kernel/src/pushpull.rs`: `push_pull`.

Acceptance tests: `app/crates/modeler-kernel/tests/curves_carry.rs` and
`curves_carry_props.rs`. Every existing test must still pass.

## Moving a curve: `Curve::transformed(&self, m)`

Let `L` be the upper-left 3×3 of `m` (`DMat3::from_mat4`). `m` is a **similarity** when
`G = Lᵀ L` is a multiple of the identity: with `s² = trace(G) / 3`, every entry of
`G - s² I` is within `1e-9 · s²` of zero, and `s²` is finite and `> 0`.

- Not a similarity, or `det L` is zero or not finite: `None`.
- Otherwise, with `sign = -1` when `det L < 0` (a mirror) and `+1` otherwise:
  - `center' = m.transform_point3(center)`
  - `x_axis' = normalize(L · x_axis)`
  - `normal' = sign · normalize(L · normal)`
  - `radius' = radius · sqrt(s²)`
  - `start`, `end` and `vertices` unchanged.
- Any result component not finite: `None`.

Why the sign: a mirror reverses the turning direction, so the normal flips to keep the
chain counter-clockwise about it with the same angles. With these formulas `point_at(a)` of
the result equals `m` applied to the old `point_at(a)` for every `a` (the tests check this
at `1e-9`). A matrix that stretches only along the circle's normal keeps the circle round,
but it isn't a similarity: `transformed` says `None` and `carry` re-fits it (below).

## Carrying curves: `curve::carry(ctx, curves, m, map, log)`

`ctx` holds a copy of the geometry the curves were on, moved by `m`. `map(ctx, v)` gives the
vertex of `ctx` that source vertex `v` became, or `None`. For each curve, in order:

1. Map every chain vertex (a closed chain's last entry maps like its first). If any maps to
   `None`, the result is `None`.
2. Candidate: `transformed(m)` with `vertices` = the mapped chain. Use it if it is `Some`
   and `fits(ctx, ..)`.
3. Otherwise `fit(ctx, mapped chain, curve.start)`; use that if it is `Some`.
4. Otherwise the result is `None`.
5. If a step edge of the chosen curve (as `curve_edges` would list them) already belongs to
   a curve, the result is `None`: one edge never belongs to two curves. "Already" means
   a curve in `ctx` when `carry` was called, or one this call added for an earlier entry.
   Build that set of edges once per call, not per curve.
6. `ctx.insert_curve(chosen, log)`; the result is its id.

`carry` returns one entry per input curve. It adds nothing else and changes nothing when it
returns all `None`. Callers ignore the result unless they need it.

## Where curves are carried

Every operation below already runs inside one `ModelEdit` and pushes one undo step; the curve
records go into that same edit, so undo and redo stay exact (tests compare the whole model).

- **Make group / component** (`make_component`): before editing, collect every curve of the
  source body. In the `edit_geometry` closure that fills the new definition, after the faces,
  call `carry` with those curves, `m = inverse` (the translation by `-origin` used for the
  vertices) and `map` = that closure's `vertex_map`. Curves whose whole chain was selected
  move into the definition. The source body keeps whatever `reconcile` leaves: nothing when
  the curve's edges were removed; the curve itself when its edges stay (they still bound a
  face that wasn't selected), so then both bodies have one. A partly selected curve gives
  the definition plain edges.
- **Copy into the clipboard** (`extract`): in the closure that fills the clipboard's root,
  after the faces, `carry` the source body's curves with `m = IDENTITY` and that closure's
  `vertex_map`.
- **Copy a body** (`copy_body_into`, used by paste for the holder and for definitions it
  doesn't reuse, and by `import_component`): same, after the faces, the source body's
  curves, `IDENTITY`, its `vertex_map`.
- **Explode** (`explode_inner`, also the last step of every paste): collect the
  definition's curves with the other source data. `restick` deletes the copied vertices and
  re-adds their edges, so the ids from the first closure are gone by then. After `restick`,
  run one more `model.edit_geometry(body, ..)` that calls `carry` with `m = data.transform`
  and a `map` that finds each copied vertex by position:
  `ctx.find_vertex(transformed position of v, LINEAR_TOL)` (the positions are already in
  `source_vertices`). Where the copy welded onto existing geometry the carried curve still
  fits through split chords; where it landed on an existing curve's edges, rule 5 skips it.
  A group's definition loses its curves with its geometry (`empty_group`, through
  `reconcile`); a component's definition keeps its own.
- **Push/pull** (`push_pull`, extrude path only): before editing, collect the body's curves
  whose `curve_edges` are non-empty and all in the pushed face's loop edges (outer and
  holes). In the building closure, once the top edges exist, `carry` them with
  `m = translation(t)` (`t = distance · n`, the push vector) and `map` = `new_vertex`. The
  cap's curve is then the base curve moved by `t`: same normal, axes, radius and angles. The
  base curve stays as it was. Side faces get no curves. A pulled circle is a cylinder with a
  curve at each end; a disc pushed into a solid gives a hole with a curve at its rim and one
  at its floor; pushed through, the cap becomes a hole in the far face and its curve stays
  on that hole's edges. The slide path (all neighbours stretch) already keeps the curve
  through `reconcile` and needs no change.

## Notes

- Tolerances: positions `LINEAR_TOL` (as in `fits`); the similarity test `1e-9` relative.
- Don't change `reconcile`, `fits` or `fit`; don't add checks to `Context::apply`.
- `find_vertex` scans every vertex; once per chain vertex in explode is fine.
- Keep the existing behaviour of every operation: the same entities, faces, materials,
  return values and errors as before. Only curves are added.

## Not in scope

Carrying part of a curve (runs of a partly selected or partly pushed curve) is left out:
those copies get plain edges. Also not here: Follow Me, picking a whole curve by clicking one
edge, Entity Info for curves, and the exporter.
