# m6-panel-collapse: collapsing a docked panel keeps the others where they are

Milestone M6 (UI), feedback round 3. Crate `modeler`: `src/dock.rs` (`stack_heights`, `drag_splitter`).

## Goal
Today, collapsing a docked panel hands its space to the other panels in the column. With Entity Info above Materials, collapsing Materials makes Entity Info grow and Materials' title bar drops to the bottom of the column. Instead, every other panel keeps its height: the title bars above the collapsed panel don't move, the panels below it move up by the body it lost, and the free space opens at the bottom of the column. Expanding it again gives it back the height it had.

## Rules
- `stack_heights(available, rows)`:
  - First size every row as if none were collapsed, with the existing rules: share `available` by `weight` (a weight that is not positive counts as 1), pin a row whose share would be under `HEADER_HEIGHT + MIN_BODY` at that minimum and share the rest among the others (repeat), and give every row the minimum when even the minimums don't fit.
  - Then a collapsed row is `HEADER_HEIGHT`; an expanded row keeps the height from the first step.
  - So the result's sum is at most `available` (when the minimums fit), and the difference is free space below the last panel.
  - All rows collapsed gives `HEADER_HEIGHT` each; no rows gives an empty list (as now).
- `drag_splitter(rows, heights, index, delta)`:
  - Same clamping and no-op cases as now (both rows expanded, each kept at or above the minimum).
  - It changes only the weights of rows `index` and `index + 1`. Their sum stays the same, split in proportion to the two new heights, so `stack_heights` with the same `available` gives row `index` its height plus `delta` and row `index + 1` its height minus `delta`, and every other row (collapsed or not) is unchanged.
  - A collapsed row's weight is never touched, so expanding it later restores its old height.
- `app.rs` needs no change: it already draws each row at the height `stack_heights` gives inside a column that doesn't shrink, so the free space shows at the bottom. Check it still draws that way.

## Acceptance
- `cargo test -p modeler --test dock_collapse`.
- `dock_stack`: one test changed in this spec's commit (`a_collapsed_panel_is_just_its_header_and_the_others_keep_their_height`); the rest unchanged and passing.
- All other existing tests unchanged, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
