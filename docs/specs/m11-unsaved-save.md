# m11-unsaved-save: Save in the unsaved-changes prompt saves, then goes on

## Problem
With unsaved changes, File > New, File > Open or closing the window shows the "Unsaved changes"
prompt. Clicking Save only closes it: nothing is saved, no Save model picker appears for an
untitled model, and the New/Open/close never happens.

## Cause
In `App::ui`, the `UnsavedChoice::Save` arm sets `self.confirm = None` and then calls
`self.save_before(&ctx, frame)`. `save_before` starts with `self.confirm.take()`, finds nothing
and returns.

## Fix
Let `save_before` take the pending action: remove the `self.confirm = None;` line from the Save
arm (or pass the action into `save_before`). Nothing else changes.

## Behaviour afterwards
- Titled .drydock model: Save writes the file; `saved_now` then runs the pending action
  (New / Open / Close) from `after_save`.
- Untitled or DAE model: `open_save_dialog` queues the Save model picker (it starts two quiet
  frames after the prompt closes, see m11-picker-after-prompt). Picker Save: `finish_save`
  writes, then the action runs.
- Picker Cancel: `after_save` is cleared, the document is unchanged and still modified, the main
  window works. (Check that the picker-cancel path clears `after_save`; fix it if it does not.)
- Discard and Cancel behave as before.

## Tests
`tests/unsaved_save_x11.rs` (given, do not edit): real window under Xvfb with xdotool, skipped
unless `DRYDOCK_GUI_TESTS` is set. It draws a rectangle, presses ctrl+n, presses Save in the
prompt (Tab, Enter), expects a "Save model" window, cancels it with Escape and expects the main
title still marked modified. It fails on current main (no picker appears).
Keep the change small: app.rs only.

## Where to look
- `show App::save_before`, `show App::save`, `show App::saved_now`, `show App::request`,
  `show App::perform`, `show App::open_save_dialog`, `show App::finish_save`
- `show prompts::unsaved_prompt`, `show UnsavedChoice`
- the `prompts::unsaved_prompt` match near the end of `App::ui` in
  app/crates/drydock/src/app.rs
- tests/modal_prompts.rs (prompt-level tests), tests/title_fresh_doc.rs (real-window helpers)
