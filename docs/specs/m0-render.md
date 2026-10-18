# m0-render: scene renderer

Milestone M0. Crate `app/crates/modeler-render`. Implement everything marked `unimplemented!()` in `src/lib.rs` behind the existing public API; add private modules and WGSL shaders freely. Allowed dependencies: those already in `Cargo.toml` (`wgpu` 30, `image`, `bytemuck`, `pollster`, `glam`, and the workspace crates). Uses `modeler_dae::FlatScene` as input and `modeler_view::Camera` for view and projection.

Tests run headless on Mesa lavapipe (software Vulkan). The worker container has it; `WGPU_BACKEND=vulkan` is set.

## Color
- All colors in materials and `Style` are sRGB, 0..1. Convert to linear before shading; render into `COLOR_FORMAT` (`Rgba8UnormSrgb`), which encodes back to sRGB. Upload textures as `Rgba8UnormSrgb` so sampling returns linear values.
- A face-on, opaque, lit surface shows exactly its material color (times its texel color) in the output image.

## Shading
- Surface color (linear) = material color x texel color (texel = white when there is no texture).
- Lit materials: multiply by `AMBIENT + (1 - AMBIENT) * |dot(n, v)|`, where `n` is the interpolated vertex normal (normalized) and `v` is the unit direction from the surface point toward the eye (for parallel projection: `-forward`). The absolute value makes every face two-sided; never cull back faces.
- Unlit materials (`Material::unlit`): no shading factor.
- Batches with `material: None` use `Style::default_face` (lit).
- Texture coordinates: (0, 0) is the bottom-left of the image, (1, 1) the top-right (sample at `(u, 1 - v)` in image row order). Repeat wrapping, linear filtering, mipmaps generated on upload.

## Transparency
- Alpha = material opacity x texel alpha.
- Batches whose material opacity < 1, or whose texture has any texel with alpha < 255, are transparent. Draw all opaque batches first (depth write on), then transparent batches (depth test on, depth write off) sorted back to front by the distance from the eye to each batch's bounding-box center, blended `src * alpha + dst * (1 - alpha)` in linear space.

## Lines
- `LineBatch`es draw as 1-pixel lines (`LineList`), color = the material color (unlit) or `Style::default_line`, depth-tested against faces. Lines are drawn after opaque faces and before transparent faces.

## Textures
- Load each material's `texture` file once per `upload` with `image` (PNG and JPEG). Files larger than `MAX_TEXTURE_SIZE` on either side are downscaled (keep aspect) first.
- A texture that is missing or fails to decode: the material renders with its color only, and `GpuScene::warnings` gets one line naming the file and the reason. Never panic on bad input.

## Depth and anti-aliasing
- Depth buffer `Depth32Float`, cleared to 1.0, compare `Less`. Projection from `Camera::projection_matrix(width / height, scene sphere)`, where the sphere comes from `GpuScene::sphere`.
- 4x MSAA when the adapter supports it for these formats, resolved into `Viewport::color`.
- Clear color: `Style::background`.

## Headless
- `render_headless`: create an instance (backends from the environment, falling back to all), request any adapter (Vulkan preferred), create a device, then `Renderer::new`, `upload`, `viewport`, `render`, and copy `Viewport::color` back to an `RgbaImage` of exactly `width x height` (mind the 256-byte row alignment for buffer copies). Return `NoDevice` if no adapter/device is available.

## Tests (fixed; don't edit)
`tests/render.rs`: background, face-on color, oblique shading, two-sided faces, depth ordering independent of batch order, default material, unlit, texture orientation and sampling, missing texture warning, texture alpha, transparency blending, line drawing and color, non-square viewports, perspective rendering. Tests build scenes in code and write any texture files to a temporary directory.

## Out of scope
Shadows, edges of faces (the importer gives only loose lines), picking, selection highlight. The app wires the renderer into its window in `m0-app`.
