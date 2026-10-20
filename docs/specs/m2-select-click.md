# m2-select-click: modifier clicks and boxes

Module `modeler-kernel/src/click.rs` (skeleton on main). Pure functions; no geometry.

Tyrel's decision: a plain click replaces the selection; Shift adds; Ctrl toggles; Alt removes. (A preferences screen for other mappings comes later; keep the mapping in one place, `ClickMods::op`.)

## `ClickMods::op`
- Alt held (with or without others): `Remove`.
- Else Ctrl held (with or without Shift): `Toggle`.
- Else Shift held: `Add`.
- Else: `Replace`.

## `apply(current, picked, mods)`
Result is sorted by `Entity`'s order and has no duplicates. `current` and `picked` may be unsorted and hold duplicates.
- `Replace`: the picked entities (even when none: clicking or boxing empty space clears the selection).
- `Add`: current plus picked.
- `Toggle`: entities in exactly one of current and picked (after removing duplicates inside each).
- `Remove`: current without the picked entities.

With any op other than `Replace`, empty `picked` leaves the selection as it was (sorted, deduplicated).

## `apply_click(current, hit, mods)`
Same as `apply` with `picked` = the hit entity, or nothing for `None`.
