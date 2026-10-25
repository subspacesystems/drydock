# m5-render-sun: shade faces by the sun

Milestone M5 (Sun). Crate `modeler-render`: `src/lib.rs`, `src/scene.wgsl`. `Style::sun` and `SunLight` are on main and not used yet.

## Goal
When `Style::sun` is `Some`, lit materials are shaded by sunlight from a fixed direction in the world instead of by today's light at the viewer. With `None` nothing changes (all existing render tests stay green).

## Shading with a sun
For each fragment of a lit (not `unlit`) material:
- `n` = the interpolated normal, normalized, turned to the side the viewer sees: if `dot(n, v) < 0` use `-n`, where `v` points from the surface to the eye (perspective) or is `-forward` (parallel), exactly as the current shader computes `v`.
- `direct` = `max(dot(n, s), 0)` where `s` = `SunLight::direction`; `direct` = 0 when `s.z <= 0` (the sun is at or below the horizon).
- `shade` = `min(1, dark + light * direct)`.
- The linear color (material color × texel, as today) is multiplied by `shade`. Alpha is unchanged (`opacity × texel alpha`).

Unlit materials, lines, extra lines and the background are not affected. The shading must not depend on where the camera is apart from choosing the side (an oblique view of a face gives the same shade as a view head-on).

## Plumbing
- Pass the sun to the shader in a uniform (for example extend the camera uniform with a direction vector carrying an "enabled" flag and a vector with `light` and `dark`); keep `#[repr(C)]` alignment rules for WGSL uniforms (16-byte vectors).
- `SunLight::shadows` and `on_ground` are for the next task (`m5-render-shadows`); ignore them here.
- No new public API is needed; don't change existing signatures.

## Acceptance
`cargo test -p modeler-render --test sun_light` (headless on lavapipe), all other `modeler-render` tests unchanged, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
