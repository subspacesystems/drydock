# m7-overhang: tint faces that need support

A printer lays each layer on the one below. A surface that faces down and leans further from
vertical than the overhang angle has too little under it and needs support. This task tints
those faces in the viewport so the user sees them while modelling. The Print Setup panel
(`m7-print-setup`) will offer the angle; this task keeps it in `ViewState` at 45°.

Skeletons on main: `app/crates/modeler-dae/src/overhang.rs` (`is_overhang`,
`tint_overhangs`, `DEFAULT_OVERHANG_DEGREES`, `OVERHANG_COLOR`),
`Action::ToggleOverhangs`, `Effect::ReuploadScene`, `ViewState::overhangs` /
`overhang_degrees`, `MenuState::overhangs`.

Files to change: `modeler-dae/src/overhang.rs`, `modeler/src/viewstate.rs`,
`modeler/src/chrome.rs`, `modeler/src/app.rs`.

Acceptance tests: `app/crates/modeler-dae/tests/overhang.rs`,
`app/crates/modeler/tests/overhang_ui.rs`, `app/crates/modeler-render/tests/overhang_render.rs`.

## `is_overhang(normal, max_degrees)`

- `max_degrees` is clamped to `0..=90` (NaN counts as 90).
- `normal` may have any length. Zero, or not finite: `false`.
- With `n` the unit normal, the surface is an overhang when it faces down and its angle from
  vertical, `asin(-n.z)`, exceeds the limit. Compute it as
  `-n.z > sin(max_degrees) + 1e-9`.
- So a vertical wall is never an overhang, a ceiling is one for any limit under 90°, and a
  surface exactly at the limit is not.

## `tint_overhangs(scene, max_degrees)`

Returns a new scene; `scene` is not changed.

1. **Bed.** The bed height is `scene.bounds.min[2]` (the lowest point of the scene). With no
   bounds there is no bed.
2. **Which triangles.** Walk every batch in order and each of its triangles (`indices` in
   threes) in order. A triangle's normal is `(b - a) × (c - a)` in its vertex order,
   computed in `f64` from the positions. A triangle is tinted when `is_overhang(normal,
   max_degrees)` and it is not **on the bed**: on the bed means all three vertices have
   `z <= bed + 1e-4` (0.1 mm). Degenerate triangles (zero normal) are never tinted.
3. **Nothing tinted:** the result is an exact copy of `scene` (same materials, batches,
   lines and bounds).
4. **Otherwise:**
   - `materials` is `scene.materials` followed by one more:
     `Material { id: "overhang", name: "Overhang", color: OVERHANG_COLOR, texture: None,
     opacity: 1.0, unlit: false }`. Call its index `k` (= `scene.materials.len()`).
   - Each source batch keeps its `material`, `positions`, `normals` and `texcoords` as they
     were; its `indices` keep only the triangles that weren't tinted, in order. A batch left
     with no triangles is dropped. Batches keep their order.
   - One new batch goes last: `material: Some(k)`, and for each tinted triangle in walk
     order, its three vertices in order: positions and normals copied from the source
     batch, `texcoords` empty, `indices` `0, 1, 2, ...` (one new vertex per corner).
   - `lines` and `bounds` are copied unchanged.

## App

- **View menu:** a check item "Overhangs" right after "Axes", checked when
  `MenuState::overhangs`; choosing it returns `Action::ToggleOverhangs` and closes the menu.
  The app fills `MenuState::overhangs` from `ViewState::overhangs`.
- **ViewState:** starts with `overhangs: false` and `overhang_degrees:
  DEFAULT_OVERHANG_DEGREES`. `apply(Action::ToggleOverhangs, _)` flips `overhangs`, leaves
  the camera alone and returns `Effect::ReuploadScene`.
- **Drawing:** whenever the app uploads the document's scene to the renderer (open, after an
  edit, entering or leaving a group, `Effect::ReuploadScene`), it uploads
  `tint_overhangs(&scene, view.overhang_degrees)` instead when `view.overhangs` is on. The
  document's own `scene` stays untinted (raycasts and zoom use it). `Effect::ReuploadScene`
  with no document open does nothing.
- Editing with overhangs on keeps the tint: each edit's new scene is tinted again.

## Not in scope

The angle field (`m7-print-setup`), a bed other than the scene's lowest point, support
generation, tinting edges.
