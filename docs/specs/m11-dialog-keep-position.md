# m11-dialog-keep-position: dialog windows keep their place

Tyrel (Windows): open Model Info, move it aside, then move the main window: the dialog jumps back to the centre of the main window. It should open centred and then stay where it was put. This happens with every dialog window, modeless and modal.

## Cause

`dialog_window_impl` (dialog.rs) builds the viewport with `.with_position(centered_position(root_outer, window.size))` on every frame, from the main window's current outer rectangle. egui sends a move whenever the builder's position differs from the last frame's, so each main-window move re-centres every open dialog.

## What to build

1. `opening_position(ctx, viewport_id, root_outer, size)` (skeleton on main): returns the stored position for `viewport_id` from `ctx` temp data; otherwise, if `root_outer` is known, computes `centered_position`, stores it and returns it; otherwise `None` (nothing stored).
2. `forget_opening_position(ctx, viewport_id)`: removes the stored position.
3. In the real-window branch of `dialog_window_impl`, use `opening_position(...)` for the builder's position instead of calling `centered_position` each frame (add the position only when it returns `Some`). Call `forget_opening_position` whenever the dialog is not open: in the `!*open` early return, and where `close` is set to false->true / `*open = false`, so reopening centres over wherever the main window is then.
4. Do not change the inline (`embed_viewports`) path, the modal blocker, or the fit-height resize.

Floating panels (floating.rs) already set the position only while the viewport is unknown; checked in the real window (Materials floated, moved, main window moved: it stayed). No change there.

## Tests (already written, do not edit)

- `tests/dialog_keep_position.rs`: unit tests of the two helpers.
- `tests/dialog_keep_position_x11.rs`: real X11 windows (needs DRYDOCK_GUI_TESTS=1, xdotool, xwininfo; skips otherwise). A plain and a modal dialog open centred over the main window, are moved to (40,60), the main window is moved to (300,200), the dialog must still be at (40,60); closed and reopened it must be centred over the main window's new place.

## Where to look

- `show dialog_window_impl`, `show centered_position`, `show dialog_viewport_id` in `app/crates/drydock/src/dialog.rs`
- `show opening_position`, `show forget_opening_position` (the skeletons)
- Temp data pattern already used there: `ctx.data_mut(|d| d.get_temp_mut_or(..))` / `insert_temp` with `viewport_id.0.with("...")`
- Test helpers: `find_window`, `position`, `move_window`, `run_case` in `tests/dialog_keep_position_x11.rs`
