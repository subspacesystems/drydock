# m0-render-twosided: coincident faces and thread-safe headless rendering

Milestone M0. Crate `app/crates/modeler-render`, file `src/lib.rs` (and `src/scene.wgsl` if needed). Tests: `tests/coincident.rs` (fixed). All existing tests in `tests/render.rs` must keep passing. No new dependencies.

## 1. Coincident front and back faces

Some exporters write every face twice: a front copy wound one way and a back copy at exactly the same place wound the other way, usually with a different material, and often triangulated along the other diagonal. The renderer currently draws all opaque triangles without culling, so the two copies fight in the depth buffer: streaks and the wrong material show through.

Rule: where two opaque surfaces coincide, the one whose front faces the viewer wins. A surface that has no coincident partner stays visible from both sides, exactly as today (`face_on_surfaces_show_their_exact_color_from_either_side` in `tests/render.rs`).

Front-facing means counter-clockwise on screen (wgpu `FrontFace::Ccw`, the current default).

Suggested approach (any approach that passes the tests is fine): draw the opaque batches twice in the same render pass, first with back-face culling (front-facing triangles only), then with front-face culling (back-facing triangles only) using a pipeline with a positive depth bias (`wgpu::DepthBiasState`, constant plus a small slope scale) so that a back-facing fragment loses against a front-facing one at the same depth. Keep the bias small: a separate surface 1 cm in front of another, in a scene about 1 m across viewed in a 96 px viewport, must still hide what is behind it, even when its back faces the viewer and the view is oblique (`a_nearby_surface_seen_from_behind_still_hides_what_is_beyond_it`). Depth compare stays `Less`.

Transparent batches and lines are unchanged.

## 2. `render_headless` from many threads

On Mesa lavapipe, creating instances and devices on several threads at once crashes the process now and then (SIGSEGV). The test harness runs tests in parallel, so this makes the render tests flaky. Serialize `render_headless`: hold a process-wide lock (`static` `std::sync::Mutex<()>`) for the whole call. A poisoned lock must not cause a panic; recover the guard. Don't change its signature.

## Tests (`tests/coincident.rs`)
- `coincident_faces_show_the_side_facing_the_viewer`: a red square facing +Z and a blue square facing -Z at the same height, in both batch orders and with either triangulation. From above (parallel and perspective, several yaws and pitches) every pixel well inside the square is pure red; from below, pure blue.
- `lone_faces_are_seen_from_both_sides_at_any_angle`: a single square is visible from either side at oblique angles.
- `a_nearby_surface_seen_from_behind_still_hides_what_is_beyond_it`: see above.
- `headless_rendering_is_safe_from_many_threads`: 8 threads each render 4 small images.
