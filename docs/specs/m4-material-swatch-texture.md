# m4-material-swatch-texture: textured materials show their image in the Materials panel

Milestone M4, from Tyrel's round 3: in the Materials panel, materials with a
texture show a plain white or see-through square. The swatch (the small square in the
list, the larger one in tiles) should show a sample of the texture image.

## Behaviour

- A material whose texture image can be read shows the image in its swatch, in both the
  list (14 x 14) and the tiles (64 x 40), instead of the flat color. The image fills the
  swatch without stretching: it is scaled to cover the swatch and the overflow is cropped
  equally on both sides.
- Images are read and shrunk off the UI thread, so opening a model with many textured
  materials doesn't stall the window. Until an image is ready, and when it can't be read
  (missing, not an image), the swatch shows the flat color as today.
- Each image file is read once per panel (per `Swatches` cache), the first time a material
  using it is drawn. A file that failed is not retried. Materials hidden by the search box
  are not drawn, so nothing is read for them.
- When a load finishes, the window repaints so the image appears without moving the mouse.
- Clicking the swatch or the name still chooses the material; the tooltip is unchanged. The
  Default entry is unchanged.

## What to build

### `crates/modeler/src/swatches.rs` (skeleton on main)

- `SWATCH_MAX_SIDE` (64): longest side of a preview in pixels.
- `thumbnail(image, max_side)`: when both sides are `<= max_side`, the image unchanged.
  Else scale by `max_side / longest side`, each new side `round(side * scale)` clamped to
  `1..=max_side`, resampled with a smoothing filter (e.g. `image::imageops::resize` with
  `FilterType::Triangle`), so a solid color stays that color.
- `cover_uv(image_size, swatch_size)`: the UV rectangle (0..1 on both axes) of the image
  part that fills the swatch. With image aspect `ia = w / h` and swatch aspect `sa`: if
  `ia > sa` the full height shows and the width fraction `sa / ia` is centered (`u` from
  `(1 - f) / 2` to `1 - (1 - f) / 2`); else the full width shows and the height fraction
  `ia / sa` is centered. A zero image side or a swatch side that isn't a positive finite
  number gives the whole image `(0,0)-(1,1)`.
- `SwatchState`: `Unknown` (never asked for), `Loading`, `Ready { size }` (the preview's
  size after `thumbnail`), `Failed(message)`.
- `Swatches`: a cache from image path (the material's texture path as stored in the model,
  used as is) to its state. `Clone` shares the cache (keep it behind an `Arc`). `Default` is
  empty. `PartialEq` is always true and `Debug` prints no contents (as on main), so
  `MaterialsPanel` keeps its derives.
  - `get(ctx, path)`: first ask for a path: mark it `Loading`, queue it for loading and
    return `None`. Loading runs on a background thread (one thread for the cache, started
    on first use, working through the queue in order, is enough): `image::open`, then
    `thumbnail(.., SWATCH_MAX_SIDE)` on the RGBA image; on success the state becomes
    `Ready { size }`, on error `Failed` with the error text. When a load finishes, call
    `ctx.request_repaint()`. Once the state is `Ready`, `get` returns
    `Some(texture id)`: the first such call uploads the preview with `ctx.load_texture`
    (keep the `TextureHandle` in the cache, so later calls return the same id and the
    texture stays alive while the cache does). `Loading` and `Failed` return `None`;
    `Failed` never queues the path again.
  - `state(path)`: the current state; never starts a load. `Ready` as soon as the
    background thread has finished, even before the first `get` uploads it.
  - `wait()`: block until every load queued so far has finished (no busy loop: a
    `Condvar` or channel). Returns at once when nothing is pending.

### Panel (`crates/modeler/src/materials_panel.rs`)

`MaterialsPanel::swatches` is on main. For each material entry drawn with a texture, ask
`panel.swatches.get(ui.ctx(), &texture.path)`. With `Some(id)`, paint the swatch rect with
the image (`ui.painter().image(id, rect, cover_uv(size, rect.size()), Color32::WHITE)`,
with `size` from `state`), else fill the flat color as today. The swatch rects, their
sizes and the click handling stay as they are. `material_entries` runs inside a scroll
area closure; passing a clone of `panel.swatches` (it shares the cache) avoids borrowing
`panel` there.

`Cargo.toml` already depends on `image` (the workspace version, PNG and JPEG).

## Tests

- `crates/modeler/tests/swatches.rs` (17): `thumbnail`, `cover_uv`, and the cache with
  PNG/JPEG files written to a temp folder, missing and broken files, many loads, clones,
  panel equality.
- `crates/modeler/tests/materials_swatch.rs` (5, `egui_kittest`): the image is drawn in the
  list swatch (14 x 14, left of the name) and the tile swatch (64 x 40, above the name),
  unreadable images keep the color swatch, materials hidden by the search read nothing,
  clicking a textured swatch chooses the material. The tests find the image by the texture
  id that `get` returns among the frame's shapes.

## Out of scope

Tinting the preview with the material color or opacity, a checkerboard behind transparent
images, reloading an image that changed on disk, previews anywhere else (texture panel,
Paint Bucket cursor), dropping previews of materials that were deleted.
