# m11-picker-after-prompt

Fix (Tyrel, Windows): File > Export STL, OK, then Cancel in the Save picker leaves the
main window disabled for good (clicks ding, it cannot even be closed).

## Cause
The export prompt is a modal OS window. On Windows creating it disables its owner (the main
window) and dropping it enables the owner again. `ExportOutcome::Ok` in `App::update` starts
the system picker in the same frame the prompt stops being drawn; the viewport window is
dropped at the end of that frame or later. The picker's thread can therefore show its dialog
while the main window is still disabled, and when the picker closes Windows restores the
owner's state as it was when the picker opened: disabled. The prompt's drop had already
enabled it, so nothing enables it again. Any picker started from a prompt can hit this
(Export OK; unsaved changes > Save on New/Open leading to Save As).

## Required change (both parts)
1. **Defer.** A picker request is never started in a frame where one of our modal prompts is
   open or was drawn. It is queued and started once `QUIET_FRAMES` (2) frames in a row had no
   modal prompt.
   - `picker.rs` (skeleton on main): `ModalGate::frame`, `Picker::queue`,
     `Picker::start_queued`, `Picker::is_busy` (already counts a queued request).
   - `App`: every `self.picker.start(...)` caller (`open_dialog`, `open_import_dialog`,
     `open_models_dialog`, `open_save_dialog`, `open_export_save_dialog`) calls `queue`
     instead and returns. Add an `App` field `modal_gate: ModalGate`. Early in `update`
     (before the prompts are drawn, after `frame` is available) compute `modal_open`: true
     when any of our modal prompt/dialog windows is open, i.e. any of `confirm`,
     `name_prompt`, `text_dialog`, `export_dialog`, `units_prompt`, the purge prompt and
     every other prompt drawn with `dialog::modal_prompt_window` / `modal_dialog_window`
     (find them all; do not miss one). Call `self.modal_gate.frame(modal_open)` and then
     `self.picker.start_queued(&ctx, Some(frame), may_start)`. A prompt that is closed by
     the user's click this frame was still open at the top of the frame, so the request
     queued by that click starts at least `QUIET_FRAMES` frames later. Request a repaint
     while a request is queued so those frames happen without input.
   - When `start_queued` returns `Started::Builtin` with its request, open our own dialog
     for it exactly as the callers did (`OpenDialog::with_mode`/`new`/`import`/`models`,
     `SaveDialog::new`/`with_mode`; the request holds `purpose`, `dir`, `name`). Move that
     match into one helper.
   - While a request is queued the other file actions must not start a second one
     (`is_busy` is already used for that at the two places that read it).
2. **Restore.** Replace `self.picker.poll()` in `update` by `poll_releasing(modal_open,
   || enable_main_window(frame))`, with `modal_open` computed as above for this frame
   (prompts that are open now). `picker::enable_main_window` is empty on other systems; on
   Windows it calls `EnableWindow(hwnd, TRUE)` for the window handle of `frame`
   (`raw_window_handle::HasWindowHandle`, `Win32WindowHandle`). Add `windows-sys` as a
   direct dependency under `[target.'cfg(windows)'.dependencies]` only if needed: match the
   version already in Cargo.lock through winit, feature `Win32_UI_Input_KeyboardAndMouse`
   (that is where `EnableWindow` lives). It is MIT/Apache-2.0; if cargo-deny complains
   about the new direct dependency, fix `app/deny.toml` the way the other crates did.
   It must compile on Linux (`cfg(windows)` code is not built there; keep it small and
   careful, it cannot be run in the gate).

Do not change the winit patch (`app/vendor/winit`) or the behaviour of other modals.

## Tests
`crates/drydock/tests/picker_after_prompt.rs` (already written; do not edit). It covers the
gate, the queue (stub records nothing until allowed, exactly one request on a later frame
with the same options), replacement, waiting for an outstanding request, and the release hook
on path / cancel answers, not on "no answer" or while a prompt is open. `App` itself needs an
`eframe::Frame` so it is not driven by kittest; keep the logic in `Picker` / `ModalGate` and
the `App` wiring thin. The existing tests (`picker_flow.rs`, `picker_requests.rs`,
`file_dialogs_ui.rs`, dialogs tests) must stay green.

After merge the orchestrator asks Tyrel to retry on Windows: Export STL > OK > Cancel and
OK > Save; New with unsaved changes > Save > Cancel.

## Where to look
`show App::open_export_save_dialog`, `show App::open_save_dialog`, `show Picker::start`,
`show Picker::poll`, `show Picker::queue`, `show ModalGate::frame`, `show StubPicker`, the
`self.picker.poll()` block and `ExportOutcome::Ok` in `App::update`, `show
dialog::modal_prompt_window`, `show dialog::modal_dialog_window`, `show
App::finish_save` (the `after_save` path), `app/vendor/winit/src/platform_impl/windows/window.rs`
(`modal_owner`, read only), tests helpers `stub()` / `export_request()` in the new test file.
