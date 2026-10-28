# m6-eyedropper-deep: sample the material drawn under the cursor, inside components too

Milestone M6 (UI), feedback round 4. Crates `modeler-kernel` (`src/infer.rs`) and `modeler` (`src/editor.rs`).

## Goal
Clicking a component with the eyedropper samples the component's own material (often none), not the material the user sees on the face they clicked. The eyedropper, and Alt+click in the Paint Bucket, must pick up exactly the material drawn at the cursor, wherever that face lives: in the open body, inside a component at any depth, or in the rest of the model shown around an open component.

## Already on main (skeleton)
`modeler_kernel::infer::material_in(model, path, rest, probe) -> Option<Option<MaterialId>>`, with `unimplemented!()`. Its doc comment is the contract.

## Rules
### Kernel: `material_in`
- The probe is in the coordinates of the body open at `path`, as for `pick_in`. Return `None` when the path doesn't resolve (or its transform can't be inverted, like `pick_in`) or the probe is invalid.
- Cast the probe's ray (`origin`, `dir`) through the whole model and find the **nearest face it meets** (smallest positive distance along the ray). Faces count at any depth inside instances, with the instances' transforms. A point on a face's boundary counts as on the face; a point in a hole doesn't. Edges, snapping and the snap radius play no part.
- Which faces take part: the open body and everything inside its instances, always; with `rest`, also every other face of the model (the root and the other instances, as drawn around an open component). Without `rest` the other faces are ignored, so a ray that meets only those gives `None`. On the root path everything is "open".
- Skip an instance whose definition is already on the way down (the same cycle guard as `gather_flat_body`).
- The result for the face it meets: the material on the **side the ray sees**. That is the front when the ray runs against the face's front normal, the back otherwise. Map the normal into the world with the inverse transpose of the instance transforms, so a mirrored instance still shows its front material outward. Equivalent and simpler: compare the ray direction mapped into the face's own body coordinates with the body-space normal (`Context::face_plane(face).normal`).
- When that side is unpainted (`None`), use the material of the **innermost painted instance** that holds the face, counting the open path's own instances (a face in the open body inherits from the open instance, as it is drawn). When nothing on the way is painted, the result is `Some(None)` (the default material). This matches the renderer (`modeler-dae/src/scene.rs`: `self.material(materials.front).or(inherited)`).
- Read-only: no caches need to change. A plain walk over the model per click is fine (the 05-house model has about 71 000 faces). Distances may be compared in world units or any one parameter along the same ray.

### Editor
- Armed eyedropper click: if `pick_at` reports an **edge of the open body** under the cursor, keep waiting (as now). Otherwise call `material_in` with the editor's path, `!hide_rest` and a probe built like `pick_with`'s (no anchor, no lock). `Some(m)`: make `m` current, clear the message and disarm. The tool stays (`m6-eyedropper-2`). `None`: keep waiting (still armed).
- Alt+click in the Paint Bucket samples the same way: an edge of the open body, or `None` from `material_in`, keeps the current material; otherwise `Some(m)` becomes current. Sampling never paints and adds no undo step.
- `paint::sample` stays as it is (other callers and tests use it).

## Notes
A throwaway implementation that passes these tests took about 100 lines in `infer.rs`: a recursive walk carrying the transform, the inherited material and the position along `path.steps`, with each face tested in its body's own coordinates by the ray mapped through the inverse transform. The same per-face ray test as `flat_face_t` works there. The editor needed about 30 lines, a `material_at` helper used by both clicks.

## Acceptance
- `cargo test -p modeler-kernel --test infer_material`: root face sides, default and empty space, a painted face inside a painted instance, inheritance, nested instances, the nearest face along the ray, near an edge, a mirrored instance, open path inheritance, rest shown or hidden, a path that doesn't resolve.
- `cargo test -p modeler --test eyedropper_deep`: eyedropper and Alt+click on faces inside components, inside an open component and on the rest of the model, empty space keeps waiting.
- Existing tests unchanged and green, notably `eyedropper`, `eyedropper_2`, `editor_paint` (`alt_click_on_an_edge_or_nothing_keeps_the_material`, `alt_clicking_an_instance_samples_its_material`), `edit_path`, `infer_model`. Also `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and all workspace tests.
