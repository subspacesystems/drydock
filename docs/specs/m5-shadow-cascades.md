# m5-shadow-cascades: sharp shadows up close

Milestone M5 (Sun), fix after the first try. Crate `modeler-render`: `src/lib.rs`, `src/scene.wgsl`.

## Problem
Today one 2048 x 2048 shadow map covers the whole scene (plus the ground area its shadow can reach). On a site model 100 m or more across, one map texel is 0.1 m or more. Zoomed in, each texel spans many pixels, so shadow edges show stairs. The depth bias is a fixed share of the whole scene's depth range (tens of centimeters on such a model), so shadows also start well away from the foot of a wall.

## Goal
Split the camera's view of the scene into depth slices ("cascades") and give each its own shadow map fitted to that slice: near the eye the maps are fine, far away coarser. The light cameras are computed by a pure function so they can be tested without a GPU.

## Already on main
- `pub const SHADOW_SIZE: u32 = 2048` (texels per side of each cascade's map), `pub const SHADOW_CASCADES: usize = 4`.
- `pub fn shadow_region(bounds, sun, on_ground) -> (DVec3, DVec3)`: implemented, the box shadows are needed in (scene bounds plus the reachable ground area). Don't change its behavior.
- `pub struct ShadowCascade { far, light_view_proj, texel }` and `pub fn shadow_cascades(camera, aspect, bounds, sun, on_ground) -> Vec<ShadowCascade>` (stub). Implement it and use it in the renderer. The old private `shadow_bounds` / `light_view_proj` / `SHADOW_BIAS` may go once unused.

## `shadow_cascades`
Notation: `R` = `shadow_region(bounds, sun, on_ground)`; view depth of a point `p` = `(p - camera.eye()) · forward` (`forward` from `camera.basis()`); "in the frame" = `camera.project(p, aspect)` gives NDC x and y in -1..1 (perspective: in front of the eye).

- `sun` need not be unit length; only its direction matters. When the sun is at or below the horizon (`z <= 0` after normalizing, or not finite/zero), return an empty list. For a perspective camera, when no point of `R` has positive view depth, return an empty list.
- Otherwise return 1 to `SHADOW_CASCADES` cascades, nearest first, with strictly increasing `far`. The last `far` is at least the deepest view depth of any corner of `R`. A fragment at view depth `t` uses the first cascade with `far >= t` (deeper than the last `far`, it uses the last one).
- `light_view_proj` maps world to an orthographic light view looking along `-sun`: NDC x and y in -1..1 across the map, depth 0..1 with 0 nearest the sun. Every corner of `bounds` (every possible caster, also outside the frame) lies in depth 0..1 for every cascade. Moving a point along `sun` changes only its depth.
- Coverage: every point of `R` in the frame lies inside the map of the cascade its depth picks, at least one texel from the map's edge (NDC |x|, |y| <= 1 - 2/SHADOW_SIZE), so the 3x3 filter stays inside.
- `texel` is the world size of one texel; texels are square (moving a point by `texel` across the sun moves it 2/SHADOW_SIZE in NDC x or y).
- Resolution: a close-up must get a fine map. With the eye 3 m from a ground point of a 200 m site, 35 degree perspective, the cascade at the target has `texel <= 5 mm`; a parallel top view 3.8 m across has `texel <= 10 mm` at the ground. Farther cascades are not finer than nearer ones. When the whole region is in view, no cascade is coarser than 1.1 x (diagonal of `R`) / SHADOW_SIZE, i.e. never worse than today's single map.
- Stability: panning the camera must not make edges crawl. When the camera moves without turning (same distance, angles and field of view), every cascade except the last keeps its `texel`, and its map moves over the world by whole texels.

Suggested method (any that passes is fine): split depths at `2·distance·4^i` (i = 0..2) and the deepest point of `R`; skip slices that hold none of `R`. For each slice take the bounding sphere of the camera frustum between its depths (perspective: the first slice starts at the eye; parallel: at the nearest depth of `R`), or the bounding sphere of `R` if that is smaller. Fit the light camera's x/y around the sphere with a few texels of margin, snap its center to whole texels along fixed light axes (x = `Z × sun` normalized, or `Y × sun` when the sun is within ~8 degrees of straight up; y = `sun × x`), and take the depth range from the corners of `R` along `sun`. Keeping x/y extents a function of the slice depths only is what makes panning stable.

## Renderer
- One depth texture array with `SHADOW_CASCADES` layers of `SHADOW_SIZE` squared; one depth pass per cascade into its layer. Create the texture once and keep it in the `Renderer` (lazily on first use is fine): don't allocate 64 MB per frame. Unused layers are not drawn.
- Fragment shader: pick the cascade by the fragment's view depth (the camera uniform has `eye` and `forward`), project with that cascade's matrix, 3x3 percentage-closer filter as today.
- Bias in terms of the cascade's texel, not the whole depth range: e.g. move the point along its normal (turned to the sun's side) by about 1.5 texels and subtract about 1.5 texels' worth of depth; the shadow pipeline's slope-scaled bias may stay. Faces in the sun show no acne; the ground right behind a wall's foot is in shadow.
- The ground plane (`on_ground`) uses the same cascades (normal +Z).
- Everything else as in `m5-render-shadows`: unlit materials never receive shadows, `on_ground` only darkens the background, nothing changes with `shadows` false or `sun` `None`, no shadow pass when the sun is down.

## Acceptance
- `cargo test -p modeler-render --test shadow_cascades` (pure, includes a property test over random cameras, scenes and suns).
- `cargo test -p modeler-render --test shadow_edges` (lavapipe): a 200 m site with a roof 10 m up and a 1 m wall; every ground pixel more than 3 px (6 px in a long low view) from a shadow edge is exactly lit or dark, seen from above 3.8 m across, in perspective 3 m from the edge, in a long low view, and 0.2 m across at a wall's foot.
- All other `modeler-render` tests unchanged (`sun_shadows`, `sun_light`, ...), `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
