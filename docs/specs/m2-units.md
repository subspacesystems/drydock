# m2-units: typed lengths

Milestone M2. Crate `app/crates/modeler-kernel`, file `src/units.rs` only. Replace the `unimplemented!("m2-units")` bodies; don't change public signatures or types. No new dependencies (std only). Tests: `tests/units.rs`, `tests/units_props.rs` (fixed; don't edit).

The measurement box of the drawing tools hands what the user typed to these functions. All results are meters (f64). Library code never panics, whatever the text.

## `meters_per(unit)`
Millimeter 0.001, Centimeter 0.01, Meter 1, Inch 0.0254, Foot 0.3048.

## `parse_length(input, default)`
Surrounding white space is ignored; blank input is `Err(Empty)`; any other failure is `Err(Invalid)`. Input is ASCII only; anything else is invalid. Letters are case-insensitive.

Grammar, in words:
- Optional sign `+` or `-` (white space may follow it). A minus negates the whole length, including a feet-and-inches pair.
- A **quantity** is one of: a decimal number (`5`, `2.5`, `.5`; a point needs digits after it, so `5.` is invalid; no exponents, no thousands separators); a fraction of two whole numbers (`3/8`, written without spaces; a zero denominator is invalid; an improper fraction such as `7/4` is fine); or a whole number, white space, then a fraction (`4 1/2`, `4  3/4`). A decimal point never mixes with a fraction (`1.5/2`, `1 1.5/2` invalid).
- A **unit word** after a quantity, optionally separated by white space: `mm`, `cm`, `m` (metric); `"` or `in` (inches); `'` or `ft` (feet). Nothing else is a unit word (`5 feet` is invalid). A bare quantity is read in `default`.
- After a feet quantity, an optional second quantity gives inches; its unit word may be omitted, `"` or `in`, but not anything else (`5' 4m`, `5'4'` invalid). Examples: `5'`, `5'4`, `5' 4"`, `5ft 4in`, `5'4 1/2"`, `5' 1/2"`, `5' 14"` (inches are not limited to 11).
- Metric and inch quantities take nothing after the unit word (`5m4`, `5"4`, `5mm 3` invalid). Two quantities without a feet mark between them (`5 5`) are invalid.
- The result must be finite; a number so large that it overflows is `Invalid`.

## `parse_lengths(input, default)`
Split `input` at every comma and parse each piece with `parse_length`; the first failing piece's error is returned (a blank piece is `Empty`, so `5,` is `Err(Empty)`). `"7"` gives a one-element list.

## `format_length(meters, unit, decimals)`
`decimals` above 6 counts as 6. A value that rounds to zero at that precision is written without a sign. A non-finite `meters` returns `"0"` (no unit; nothing else is promised for it).
- Millimeter, Centimeter, Meter: the value in that unit rounded to `decimals` places, trailing zeros and then a trailing point removed, then `mm`, `cm` or `m`. `2.5m`, `100mm`, `0m`, `-1.25m`.
- Inch: the same with the inch mark `"`. `4.5"`, `0"`.
- Foot: the length rounded to `decimals` places of an inch, then split into whole feet and the rest in inches (so rounding that reaches 12 inches carries into the feet). Written `F' I"` with the inches trimmed as above; whole feet only gives `5'`; no feet gives `4.5"`; zero gives `0"`. A minus sign goes in front of the whole thing: `-2' 1"`.

Reading what `format_length` wrote gives back the length within half a unit of the last place written (inches for Inch and Foot), whatever `default` is, and writing it again gives the same text.
