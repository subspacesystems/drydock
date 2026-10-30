# m7-text-outline: text to flat outlines

3D text (`m7-text-3d`) needs the shape of letters as flat polygons. This task is the pure
function that turns a string into closed loops: a font's glyph outlines, flattened at a
chord tolerance, with outer and hole loops told apart, and a simple line layout. No kernel,
no UI.

Skeleton on main: `app/crates/modeler-dae/src/text.rs` (`text_outline`, `TextOutline`,
`TextLoop`, `TextError`, re-exported from the crate root); `skrifa` (MIT OR Apache-2.0,
already in the lock through egui's font stack) is a workspace dependency of `modeler-dae`.
Files to change: `app/crates/modeler-dae/src/text.rs` only.
Acceptance tests: `app/crates/modeler-dae/tests/text_outline.rs` (they use the app's bundled
Roboto, `app/crates/modeler/assets/fonts/Roboto-Regular.ttf`, through `include_bytes!`; the
function itself takes the font bytes as an argument).

## `text_outline(font, text, size, tolerance)`

Returns a `TextOutline`; `size` is the em height and `tolerance` the largest distance between
a curve and its polyline, both in the caller's unit. Y is up, the first line's baseline is
`y = 0`, its pen starts at `x = 0`.

1. **Arguments, checked in this order:** `size` not finite or not above 0: `BadSize`.
   `tolerance` likewise: `BadTolerance`. Bytes `skrifa::FontRef::new` rejects, or a font with
   zero units per em: `BadFont`.
2. **Read outlines in font units.** Use `Size::unscaled()` for outlines, advances and
   metrics, and scale in `f64` by `k = size / units_per_em`. Do not ask skrifa for a scaled
   size: it rounds coordinates to 1/64 of the size, which wrecks small text.
3. **Lines.** Split the text at `'\n'`; `lines` is the piece count (empty text is 1 line).
   Line `i` is moved down by `i * line_height`, where `line_height = (ascent - descent +
   leading) * k` from the font's metrics (descent is negative). `width` is the largest line
   advance.
4. **Characters, in order along a line.**
   - `'\r'` and other control characters: skipped, no advance.
   - `'\t'`: advances by 4 times the space glyph's advance, draws nothing.
   - Otherwise map the char with the font's charmap; a char the font lacks uses glyph 0 (the
     missing-glyph box). Draw its outline at the current pen `x`, then advance by the
     glyph's advance width. No kerning.
5. **Flatten** each contour, working in font units with the tolerance divided by `k`. Line
   segments keep their ends. A quadratic with control points `p0 p1 p2` becomes
   `n = max(1, ceil(sqrt(|p0 - 2 p1 + p2| / (4 tol))))` equal steps in `t`; a cubic
   `p0 p1 p2 p3` becomes `n = max(1, ceil(sqrt(3 M / (4 tol))))` steps, `M` the larger of
   `|p0 - 2 p1 + p2|` and `|p1 - 2 p2 + p3|`. Each step adds the curve point at `t = i / n`,
   `i = 1..=n`. These counts bound the chord's distance from the curve by `tol`, and a
   smaller `tol` never gives fewer steps.
6. **Clean each loop:** drop a point equal to the one before it, drop trailing points equal
   to the first (the loop closes by itself), drop a contour left with fewer than 3 points.
7. **Nest the loops of one glyph.** For loop `i`, the containers are the other loops of the
   same glyph whose polygon contains `points[0]` of loop `i` (even-odd crossing test). An odd
   number of containers makes it a hole; its `parent` is the container with the smallest
   absolute area. An even number (0, 2, ...) makes it an outer loop with `parent: None`
   (an island inside a hole is outer).
8. **Orient:** outer loops counter-clockwise (positive signed area), holes clockwise,
   reversing the point order where the font's winding differs.
9. **Output order:** by line, then character, then the glyph's contour order. `parent` is an
   index into the final `loops` vector (add the glyph's starting offset).

A space or tab has no loops but advances; empty text has no loops and `width` 0.

## Where to look

- `show text_outline`, `show TextOutline`, `show TextLoop`, `show TextError` for the skeleton.
- skrifa: `skrifa::FontRef::new`, `MetadataProvider` (`font.charmap()`, `font.metrics(size,
  loc)`, `font.glyph_metrics(size, loc).advance_width(gid)`, `font.outline_glyphs().get(gid)`),
  `skrifa::outline::{DrawSettings::unhinted, OutlinePen}`, `skrifa::instance::{Size, LocationRef}`.
  Read them under `~/.cargo/registry/src/*/skrifa-0.44.0/src`.
- Test helpers in `tests/text_outline.rs`: `check_structure` (what every result must satisfy),
  `area`, `inside`.
- `modeler-dae/src/overhang.rs` for the crate's module style.
