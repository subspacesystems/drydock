# m10-sky-ground-render: the renderer draws sky and ground

Part of the background feedback. The model will carry a sky and a ground color
(`m10-background-kernel`); the Model Info task wires them to the viewport. This task is the
renderer only: two optional colors in `drydock_render::Style`, drawn as a full-screen pass.

## Interface (skeleton on main)

`Style` has two new fields, both `Option<[f32; 4]>`, sRGB RGBA 0..1 like `background`:

- `sky`: the color where the view looks above the horizon.
- `ground`: the color where it looks below the horizon.

`Style::default()` has both `None`, so every existing caller and render test is unchanged. A
`None` color shows `background` there (the clear color). `drydock::look::viewport_style`
sets both to `None` for now.

## Rule

The horizon is a property of the view direction, not of the eye position. World +Z is up.

For a pixel with NDC `(nx, ny)` (`ny` up, -1..1) let `f` be the camera's forward vector and `u`
its up vector (`Camera::basis`), `t = tan(fov_y / 2)`. Define

    h(ny) = f.z + ny * t * u.z

`h > 0`: sky. `h < 0`: ground. `h == 0`: either (the horizon line).

- **Perspective:** `h` is the z component of the view ray's direction up to a positive factor
  (the camera's right vector is horizontal, so `nx` does not matter). The horizon is a
  straight horizontal line on screen. Using the uniform `h` for both projections is
  fine, or take the sign of the ray's z from `inv_view_proj` for perspective; the results agree.
- **Parallel:** all view rays are equal, so a ray test would color the whole screen one way.
  Use the same `h(ny)`: the horizon line is where a perspective camera with the same eye,
  target and field of view would show it. Switching projection does not move the horizon.
- Level camera: horizon at the middle row. Pitched down by `p`: `ny = tan(p) / t`, above the
  middle (off screen when beyond the top). Straight down (`u.z = 0`, `f.z = -1`): all ground.
  Straight up: all sky.
- The eye's height does not matter: below z = 0 the upper half is still sky.

No texture and no soft blend: a hard edge. (A blend of a few pixels is allowed if all test
pixels, which are at least 8 rows from the horizon, stay exact.) Colors are exact: convert
with `linear_color` like `background`; the sRGB texture path was fixed in `m10-viewport-gamma`.

## Drawing

- When `sky` or `ground` is `Some`, draw one full-screen pass before everything else in the
  main render pass: after the clear, before the shadow `ground_pipeline` pass and the
  geometry. No depth write, depth compare `Always`, same MSAA count and color target as
  `ground_pipeline`. Reuse `vs_ground` (full-screen triangle with `ndc`); add `fs_sky` in
  `scene.wgsl`.
- In `fs_sky`: compute `h`; if `h > 0`, write the sky color, else the ground color; if that
  color is absent, `discard` (the clear color stays). Neither present: skip the pass.
- Pass the data in `CameraUniform` (both the Rust struct and the WGSL struct, same layout):
  `sky: vec4` (linear rgb, `w` = 1 when present), `ground_color: vec4` (same), `horizon: vec4`
  (`x = f.z`, `y = t * u.z`). The shadow-map light camera also builds a `CameraUniform`: give
  it zeros for the new fields.
- The shadows on-ground pass (`on_ground`) still draws on top of the ground color. It shades
  `camera.background`; leave it as is (the model-info task decides whether it should shade
  the ground color).

## Tests (failing on main, must pass)

`drydock-render/tests/sky_ground.rs` (headless, lavapipe): level camera (upper rows sky, lower
ground), pitched up and down, straight down (all ground) and straight up (all sky), eye below
z = 0, one color missing (background shows), neither (plain background, default unchanged),
parallel projection same horizon as perspective at three pitches, parallel level and straight
down, geometry drawn over both. Existing tests must stay green (`cargo test -p drydock-render`,
`-p drydock`).

## Where to look

- `Style`, `Style::default`, `linear_color`, `CameraUniform`, `Renderer::render_with_lines`,
  `ground_pipeline` setup in `Renderer::new` (`show Renderer::new`), `vs_ground`/`fs_ground`
  in `src/scene.wgsl`.
- `drydock_view::Camera::basis`, `Camera::eye`, `Projection`.
- Test helpers: `render_headless`, the `render`/`assert_rgb` helpers in `tests/viewport_colors.rs`
  and `tests/sky_ground.rs`.
