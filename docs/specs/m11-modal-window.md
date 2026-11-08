# m11-modal-window: prompts in modal windows

Milestone M11. Prompts become their own OS windows that belong to the main window
and have to be answered first: the main window ignores clicks and keys while one is open.
This task adds a modal mode next to `dialog_window` and moves the shared prompts and the
Make Group/Component name prompt to it. The remaining prompts follow in `m11-modal-dialogs`.

## What is already on main

- `dialog_window` (own viewport; embedded `egui::Window` in tests), `DialogWindow`,
  `centered_position`, `dialog_viewport_id` (task `m11-dialog-window`).
- The windowing patch: every non-root window names the root as parent;
  `egui_winit::mark_modal_title(ctx, title)` makes the window created for a viewport with that
  exact title modal (Wayland, Windows, and X11 `_NET_WM_STATE_MODAL`). Call it before the
  viewport is first shown. Nothing stops the main window's input at the application level:
  that is this task.
- Skeleton `dialog::modal_dialog_window(ctx, id, window, open, content)`
  (`unimplemented!`); `DialogWindow` is unchanged (no new field).
- `examples/modal_probe.rs`: a small eframe app for the real-window test. Do not edit it.

## What to build

1. **`modal_dialog_window`** in `dialog.rs`. Share the viewport code with `dialog_window`
   (a private helper taking a `modal` flag) instead of copying it.
   - Does nothing when `!*open`.
   - **Real window** (`!ctx.embed_viewports()`): call `egui_winit::mark_modal_title(ctx,
     window.title)`, then show the same viewport `dialog_window` shows (same id, size,
     centering, close and Escape handling). While open, the *calling viewport* (the one whose
     pass called the function; the root for the shared prompts) must ignore pointer and
     keyboard input: draw an invisible layer over its whole screen rect (an `egui::Area` at
     `Order::Foreground`, no fill, that senses click and drag) and make it egui's modal
     layer (`Memory::set_modal_layer`, as `egui::Modal` does; see `egui::Modal::show`) so
     widgets below cannot take focus or keys either. No dimming. Clear the modal layer when
     the dialog closes. If the OS focus is on the main window, typed keys still reach egui
     there; they must not reach any main-window widget.
   - **Embedded** (`ctx.embed_viewports()`): `egui::Modal::new(id)` with the strong title line
     the shared prompts draw today (`dialog_title`), a width of `window.size.x`, then
     `content`. The title-bar-less Modal closes on Escape or a click outside, as today.
   - `open` is cleared by Escape, the title bar's close, and `content` through the caller's flag.
2. **Shared prompts** `confirm_dialog` and `name_dialog` (`dialog.rs`): same signatures, same
   answers (`Some(true)`, `Some(false)`, `None`), same buttons and defaults; they call
   `modal_dialog_window` with a size that fits (about 380 x 130 for confirm, 360 x 120 for
   name; check a screenshot). Closing without an answer (Escape, title bar) is `Some(false)`.
   In the real window the title is the window's title, so skip the `dialog_title` line there.
   Callers (`app.rs` `app_confirm`, `toolbars.rs`) stay as they are; the viewport title is the
   `title` argument. `tests/confirm.rs`, `tests/primary_buttons.rs`, `tests/dialogs_enter.rs`
   and `tests/open_dialog_primary.rs` pass unchanged.
3. **Make Group / Make Component** (`chrome::name_prompt`): replace the `egui::Window` with
   `modal_dialog_window` (title "Make Group" or "Make Component", about 340 x 130). Keep
   the signature, `NamePrompt`, `PromptOutcome`, the "Name:" label and the OK/Cancel row;
   `Escape`/close is `Cancel`. `tests/chrome_components.rs` passes unchanged.
4. **Test hook**: `DRYDOCK_OPEN_DIALOG=group` at startup opens the Make Group prompt (set
   `name_prompt = Some(NamePrompt::new(true))` where the app is built, next to the `about`
   hook from `m11-dialog-window`). Undocumented in `--help`.
5. `app.rs` already skips `global_input` while any prompt is open (`dialog_open`); keep
   `name_prompt` in that list.

Only egui APIs and the vendored `egui_winit` function; no new dependency. If egui's modal
layer cannot be used from a viewport other than the one it draws in, say so in the commit
message and block the calling viewport by your own means (an area that eats input plus
clearing keyboard focus each frame); the Xvfb test decides.

## Acceptance tests

- `tests/modal_window.rs` (kittest, embedded; do not edit): title and content, nothing while
  closed, a click on a main-window button does nothing while open and works after close,
  clicks inside work, Escape clears `open`; `confirm_dialog` and `name_dialog` keep their
  answers (Enter/Escape/OK).
- `tests/modal_window_x11.rs` (Xvfb, no window manager, xdotool, xprop,
  `DRYDOCK_GUI_TESTS=1`; skips itself otherwise): the probe with `--modal` has a "Probe Name"
  window that is transient for the main window and has `_NET_WM_STATE_MODAL`; a click and typed
  keys in the main window while it is open change nothing; typing and Enter in the prompt
  answers `ok:box` and closes it; afterwards the main window takes a click and keys again;
  Escape cancels; and the app started with `DRYDOCK_OPEN_DIALOG=group` has a "Make Group"
  window that is transient for "Drydock", modal, and closes on Escape.

Run `harness/bin/run-tests -p drydock` at the end, with Xvfb and `DRYDOCK_GUI_TESTS=1`.
Take a `gui-session` style look if you can: the prompt should open centered over the main window.

## Where to look

Use `show <name>`:
- `modal_dialog_window`, `dialog_window`, `DialogWindow`, `centered_position`,
  `dialog_viewport_id`, `confirm_dialog`, `name_dialog`, `dialog_title`, `button_row`,
  `DialogButton` in `dialog.rs`.
- `name_prompt`, `NamePrompt`, `PromptOutcome` in `chrome.rs`; the `name_prompt` and
  `app_confirm` handling and the `dialog_open` list in `app.rs`; the `about` hook.
- egui 0.36: `egui::Modal::show`, `Memory::set_modal_layer`, `Memory::allows_interaction`,
  `Context::embed_viewports`, `Context::show_viewport_immediate`.
- `egui_winit::mark_modal_title` and `app/vendor/egui-winit/PATCHES.md`.
- Tests: `tests/confirm.rs` (`confirm_harness` style), `tests/dialog_window.rs` (X helpers).
