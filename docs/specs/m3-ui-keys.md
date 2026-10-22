# m3-ui-keys: keys for the M3 tools

Only `modeler/src/input.rs` changes (the `key` function). The skeleton already has `Action::Flip(usize)` and `Effect::Flip(usize)`, and `ViewState::apply` passes a flip straight through. Flip has no key: it is reached from a menu in a later task. Never panic.

## Key table additions (plain key, no modifier)
- `P` selects Push/Pull, `M` Move, `Q` Rotate, `S` Scale, `T` Tape Measure.
- The projection toggle moves from `P` to Shift+`P`. The View menu will also offer it.
- Every other key and modifier combination stays as it was: Ctrl+S is Save, Ctrl+Shift+S is Save As, plain `S` is no longer unbound, and Shift/Ctrl/Alt with `P`, `M`, `Q`, `S`, `T` (other than Shift+`P` and the existing Ctrl+S forms) do nothing. Lower-case names do nothing.

## Tests
`tests/input.rs` (key table, unlisted keys, a check that older letters keep their tools) and `tests/viewstate.rs` (flip passes through untouched).
