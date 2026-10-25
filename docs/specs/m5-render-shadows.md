# m5-render-shadows: shadows from the sun

Milestone M5 (Sun). Crate `modeler-render`: `src/lib.rs`, `src/scene.wgsl`. Builds on `m5-render-sun` (merged first): `Style::sun` shades lit faces by the sun's direction. `SunLight::shadows` and `on_ground` are ignored so far.

## Goal
When `Style::sun` is `Some` and `shadows` is true, faces that the sun cannot see are in shadow, and the shadow of every face falls on the faces behind it.

## Behavior
- Every face of the scene (all batches, any material, lit or unlit) blocks sunlight, whichever side the sun is on.
- A lit (not `unlit`) fragment that is in shadow gets `direct = 0`, so its shade is `dark`. Fragments in the sun are shaded exactly as without shadows. Unlit materials never receive shadows. A face never shadows itself (no acne stripes on faces facing the sun; use a depth bias or slope-scaled bias and test it).
- Suggested method (any that passes is fine): render the scene's depth from the sun into a texture with an orthographic light camera fitted around the scene's bounding sphere, then compare in the main pass with a small bias and a 3x3 percentage-closer filter. Edges may be soft by about one shadow-map texel; use at least 2048 x 2048. When the sun is at or below the horizon, skip the shadow pass (everything lit is already dark).
- `on_ground` (needs `shadows`): the plane z = 0 is covered where no face is drawn. Where the sun cannot see that plane the background color shows darker: the background (linear) is multiplied by `dark / min(1, dark + light * s.z)`, `s` the sun direction. Where the sun does see it, the background is unchanged. The plane never lightens, hides lines or covers faces, and is not drawn when `on_ground` is false.
- With `shadows` false, or `sun` `None`, nothing changes.
- Shadows do not depend on the camera (perspective or parallel).

## Acceptance
`cargo test -p modeler-render --test sun_shadows` (headless on lavapipe), all other `modeler-render` tests unchanged, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests. Keep the added GPU work off the path when `Style::sun` is `None` or `shadows` is false.
