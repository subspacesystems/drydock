# m12-cursor-art: tool cursor images and hotspots

Part of M12 (Tyrel). Only the art and its table: showing the cursor is `m12-cursor-set`.

## What exists (on main)
- `design/brand/icons/generate_icons.py` now also writes `design/brand/icons/cursors/<name>.svg` for every tool icon except `select`, plus `eyedropper`: 32x32, same drawing as the icon over a dark outline, so white ink shows on light backgrounds. These 18 SVGs are committed. Do not change the generator or the SVGs.
- `drydock::cursors` (`src/cursors.rs`) is a skeleton with `unimplemented!()` bodies.
- The paint icon already pours to the left, so the bucket cursor is mirrored as required; nothing to flip here.

## To do
1. Run the ignored test `write_cursor_pngs` (see the header of `tests/cursor_art.rs`). It writes 18 PNGs to `app/crates/drydock/assets/cursors/<name>.png`. Commit them.
2. Fill in `src/cursors.rs`:
   - `file_name(tool)`: the icon name (`line`, `rectangle`, `circle`, `arc`, `arc3`, `pushpull`, `offset`, `followme`, `move`, `rotate`, `scale`, `tape`, `erase`, `paint`, `orbit`, `pan`, `zoom`); `None` for `Tool::Select`.
   - `image_png(tool)`: the PNG bytes via `include_bytes!`; `None` for Select.
   - `hotspot(tool)`: the clicking point in 32 px image coordinates, `None` for Select:

     | tool | point | tool | point |
     | --- | --- | --- | --- |
     | line | 25,7 | pushpull | 16,22 |
     | rectangle | 25,25 | orbit | 16,16 |
     | circle | 16,16 | paint | 4,22 |
     | arc | 16,12 | tape | 4,16 |
     | arc3 | 26,22 | erase | 9,24 |
     | offset | 22,22 | pan | 16,18 |
     | followme | 25,7 | zoom | 13,13 |
     | move | 16,16 | scale | 27,5 |
     | rotate | 16,16 | | |

   - `eyedropper_png()` and `eyedropper_hotspot()` = (4, 28): the Eyedropper (Paint with Alt) is not a toolbar tool.
3. Add a bullet for `assets/cursors/` to `assets/README.md` (our own art, from `design/brand/icons/cursors`, written by `write_cursor_pngs`).

Rings: the hotspot for tools with a ring is the ring's center; the tests check no white ink sits on that pixel.

## Tests
`tests/cursor_art.rs`: Select has none; the other 17 tools have a 32x32 PNG and an in-image hotspot; the table values; dark outline pixels and white ink exist and white ink is outlined; committed PNGs equal a fresh render of the SVGs and the embedded bytes.

## Where to look
- `show write_cursor_pngs`, `show render` in `tests/cursor_art.rs`
- `show Tool` (`src/input.rs`), `icons::TOOLS` (`src/icons.rs`)
- `show window_icon::ICON_PNG` for an `include_bytes!` example
